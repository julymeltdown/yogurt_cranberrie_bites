import { createEventFeedController, type ResourceLink } from '../../features/event-feed'
import { createFeedThreadController } from '../../features/feed-thread'
import { createInboxController } from '../../features/inbox'
import { targetRequest } from '../../features/inbox/model/targets'
import { createPeopleController } from '../../features/people'
import { createIntroController } from '../../features/intro'
import { createAgeStatusController } from '../../features/age-status'
import { canonicalId, safeReturnTarget } from '../routes'
import { validCapabilities } from '../capabilities'
import { socialRoute } from './routes'
import { services, type SocialOptions, type SocialService, type RequestsModule, type SocialPolicyNotification } from './ports'
import type { PrivacyScope } from '../../shared/query/privacy'
export interface SocialView { readonly phase: 'ready' | 'withdrawn'; readonly path: string | null; readonly notice: 'command' | 'draft' | 'unavailable' | null; readonly version: number }
export const sameScope = (a: PrivacyScope, b: PrivacyScope) => a.userId === b.userId && a.eventId === b.eventId && a.generation === b.generation && a.policyGeneration === b.policyGeneration && a.gate === b.gate
/** One shell lease, canonical controllers only; never constructs Auth/cache/world/socket. */
export function createSocialOwner(options: SocialOptions) {
  const { auth, bootstrap, scope, availability } = options
  if (!auth.privacy.isCurrent(scope) || !canonicalId(scope.userId ?? '') || !canonicalId(scope.eventId ?? '') || !validCapabilities(bootstrap.capabilities)
    || bootstrap.participant.userId !== scope.userId || bootstrap.participant.eventId !== scope.eventId || bootstrap.event.eventId !== scope.eventId) throw new Error('Social lease unavailable')
  let disposed = false, navigating = 0, targetFlight: AbortController | null = null, requests: RequestsModule | null = null
  let view: SocialView = Object.freeze({ phase: 'ready', path: null, notice: null, version: 0 })
  const listeners = new Set<() => void>(), stops: (() => void)[] = [], started = new Set<string>()
  const notify = (patch: Partial<SocialView> = {}) => { view = Object.freeze({ ...view, ...patch, version: view.version + 1 }); for (const fn of Array.from(listeners)) { try { fn() } catch { /* Isolate consumer observers. */ } } }
  const live = () => !disposed && auth.privacy.isCurrent(scope)
  function available(service: SocialService) {
    if (!live() || !['OPEN', 'LIVE', 'READ_ONLY'].includes(bootstrap.event.phase)) return false
    try {
      const lease = availability.read()
      return !!lease && lease.bootstrap === bootstrap && sameScope(lease.scope, scope) && Object.keys(lease.services).every(k => (services as readonly string[]).includes(k) && typeof lease.services[k as SocialService] === 'boolean')
        && lease.services[service] === true && (service !== 'feed' || bootstrap.capabilities.feed.available)
        && (service !== 'requests' || bootstrap.capabilities.social_sharing.available)
    } catch { return false }
  }
  const access = (service: SocialService) => ({ read: () => ({ available: available(service), readOnly: bootstrap.event.phase === 'READ_ONLY' || !bootstrap.capabilities.mutations.available }),
    subscribe: (fn: () => void) => { const stop = availability.subscribe(() => { if (live()) fn() }); return stop } })
  const media = options.media?.create({ auth, bootstrap, scope })
  const feed = createEventFeedController({ auth, access: access('feed') })
  const thread = createFeedThreadController({ auth, access: access('feed'), ...(media ? { media: media.upload } : {}) })
  const people = createPeopleController({ auth, access: access('people') })
  const intro = createIntroController({ auth, access: access('intro') })
  const age = createAgeStatusController({ auth, access: access('age'), ...(options.verification ? { allowVerificationOrigin: options.verification.allowOrigin } : {}) })
  const inbox = createInboxController({ auth, access: { read: () => available('inbox'), subscribe: fn => availability.subscribe(fn) }, navigate: d => { if (sameScope(d.scope, scope) && live()) navigate(d.path) } })
  // The thread UI begins edits before invoking its navigation callback. Protect a
  // retained composer draft before that child action can replace it.
  const threadPanel = { ...thread, beginPostEdit() {
    const draft = thread.read().draft
    if (!draft.edit && (draft.text !== '' || draft.media_ids.length > 0 || draft.links.length > 0)) { notify({ notice: 'draft' }); return false }
    return thread.beginPostEdit()
  } }
  const children = [feed, thread, people, intro, age, inbox]
  function locked() { return ['sending', 'pending', 'uncertain'].includes(thread.read().mutation.phase) || ['sending', 'uncertain', 'checking', 'retired'].includes(inbox.read().command.phase)
    || intro.read().command.locked || age.read().command.locked || age.read().pending !== null || !!requests?.read().locked || !!media && ['initializing', 'uploading', 'finalizing', 'uncertain'].includes(media.upload.read().phase) }
  function navigationReason(path: string): SocialView['notice'] {
    if (!live() || safeReturnTarget(path, '') !== path || !path) return 'unavailable'
    if (path === view.path) return null
    if (locked()) return 'command'
    const next = socialRoute(path), state = thread.read()
    const changesPost = next && (next.kind === 'thread' || next.kind === 'edit') && next.id !== state.selectedId
    if (changesPost && (state.reply.text !== '' || state.draft.edit || next.kind === 'edit' && state.draft.text !== '') || next?.kind === 'compose' && state.draft.edit) return 'draft'
    return null
  }
  function canNavigate(path: string) { return navigationReason(path) === null }
  function navigate(path: string) {
    const reason = navigationReason(path)
    if (reason) { if (live()) notify({ notice: reason }); return false }
    if (path !== view.path) cancelTargets()
    options.navigate({ path, scope }); return true
  }
  function cancelTargets() { navigating++; targetFlight?.abort(); targetFlight = null; inbox.cancelTarget() }
  function dispose() {
    if (disposed) return
    disposed = true; navigating++; targetFlight?.abort(); targetFlight = null
    for (const stop of stops.splice(0).reverse()) { try { stop() } catch { /* Finish all teardown. */ } }
    for (const child of [...children, ...(requests ? [requests] : [])].reverse()) { try { child.dispose() } catch { /* Never retain another child after a faulty cleanup. */ } }
    try { media?.dispose() } catch { /* Parent still withdraws. */ }
    notify({ phase: 'withdrawn', path: null, notice: null }); listeners.clear()
  }
  function register(make: () => () => void) { const stop = make(); if (disposed) stop(); else stops.push(stop) }
  try {
    // Register parent first, before starting child lifetimes. Auth belongs to the shell.
    register(() => auth.privacy.registerCleanup(dispose))
    for (const child of children) register(() => child.subscribe(() => { if (live()) notify() }))
    register(() => availability.subscribe(() => {
      if (!live()) return
      cancelTargets(); notify()
      // A producer may qualify after the route opened. Start the same lazy
      // module when that exact lease becomes ready, without replacing owners.
      if (!requests && (view.path === '/me/social' || view.path === '/me/requests') && available('requests')) enter(view.path)
    }))
    if (!live()) throw new Error('Social lease unavailable')
  } catch { dispose(); throw new Error('Social lease unavailable') }
  const avatars = options.avatars ? { auth, scope, resolve: (expected: PrivacyScope, signal: AbortSignal) => {
    if (!live() || !sameScope(expected, scope)) return Promise.reject(new Error('Avatar unavailable'))
    return options.avatars!.resolve({ auth, bootstrap, scope, signal })
  } } : undefined
  function start(key: string, child: { start(): void }) { if (!started.has(key)) { started.add(key); child.start() } }
  function enter(path: string) {
    const reason = navigationReason(path), route = socialRoute(path)
    if (reason || !route) { if (live()) notify({ notice: reason ?? 'unavailable' }); return false }
    const previous = view.path
    if (previous !== path) cancelTargets()
    notify({ path, notice: null })
    switch (route.kind) {
      case 'feed': start('feed', feed); if (previous && previous !== path) void feed.loadFeed(); break
      case 'notice': start('feed', feed); void feed.selectNotice(route.id); break
      case 'compose': start('thread', thread); if (thread.read().selectedId !== null && !thread.read().draft.edit) thread.select(null); break
      case 'thread': case 'edit':
        start('thread', thread)
        if (thread.read().selectedId !== route.id) thread.select(route.id)
        else if (previous !== path) void thread.loadPost()
        break
      case 'people': case 'profile': { const wasStarted = started.has('people'); start('people', people); if (wasStarted && route.kind === 'people' && previous !== path) void people.load(); break }
      case 'intro': { const wasStarted = started.has('intro'); start('intro', intro); if (wasStarted && previous !== path) void intro.load(); break }
      case 'age': { const wasStarted = started.has('age'); start('age', age); if (wasStarted && previous !== path) void age.load(); break }
      case 'inbox': start('inbox', inbox); if (previous && previous !== path) void inbox.load(); break
      case 'handles': case 'requests':
        if (available('requests') && options.requests && !requests) {
          try {
            const module = options.requests.create({ auth, bootstrap, scope, access: access('requests') })
            if (!live()) { module.dispose(); return false }
            requests = module; register(() => module.subscribe(() => { if (live()) notify() })); if (live()) module.start()
          } catch { requests?.dispose(); requests = null }
        }
        requests?.enter?.(route.kind === 'handles' ? '/me/social' : '/me/requests')
        break
    }
    return live()
  }
  return { auth, bootstrap, scope, options, feed, thread, threadPanel, people, intro, age, inbox, media, avatars, available, read: () => view,
    subscribe(fn: () => void) { if (disposed) return () => {}; listeners.add(fn); return () => { listeners.delete(fn) } }, isDisposed: () => disposed,
    enter, navigate, canNavigate, requests: () => requests, dispose,
    async openResource(link: ResourceLink) {
      if (!available('feed') || !canonicalId(link.target_id)) return false
      const input = ['PARTY', 'GIVEAWAY', 'POST', 'NOTICE'].includes(link.target_type)
        ? targetRequest(scope, { target_type: link.target_type as 'PARTY' | 'GIVEAWAY' | 'POST' | 'NOTICE', target_id: link.target_id }) : null
      const version = ++navigating; targetFlight?.abort(); const c = new AbortController(); targetFlight = c
      try {
        if (!input) throw new Error('Destination unavailable')
        const result = await auth.privacy.run(scope, ({ signal }) => auth.request({ ...input, signal }), c.signal)
        if (!live() || version !== navigating) return false
        return navigate(result.data)
      } catch {
        if (live() && version === navigating) { feed.removeLink(link); thread.removeLink(link); notify({ notice: 'unavailable' }) }
        return false
      } finally { if (targetFlight === c) targetFlight = null }
    },
    notifyPolicy(change: SocialPolicyNotification) {
      if (!live() || !change || Object.keys(change).sort().join(',') !== 'reason,scope' || !change.scope
        || Object.keys(change.scope).sort().join(',') !== 'eventId,gate,generation,policyGeneration,userId' || !sameScope(change.scope, scope)
        || !['block', 'consent-revoked', 'consent-expired', 'handle-replaced', 'suspended', 'event-closed'].includes(change.reason)) return false
      return auth.privacy.changePolicy(scope, change.reason)
    },
    completeVerification(expected: PrivacyScope, proof: Parameters<typeof age.completeVerification>[0]) {
      return live() && sameScope(expected, scope) && available('age') ? age.completeVerification(proof) : Promise.resolve(false)
    },
  }
}
export type SocialOwner = ReturnType<typeof createSocialOwner>
