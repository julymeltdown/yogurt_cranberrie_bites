import type { Decoder, HttpRequest } from '../../../shared/api/http'
import type { PrivacyScope } from '../../../shared/query/privacy'
import type { WorldEntryRequest } from '../../world-entry/authority'
import { validCapabilities } from '../../capabilities'
import { sameScope } from '../../social-composition/owner'
import { services, type SocialAvailability, type SocialContext, type SocialService } from '../../social-composition/ports'
import type { OwnedSocialAvailability } from '../availability-lifetime'
import { decodePage } from '../../../features/people/contract'
import { decodeOwn } from '../../../features/intro/contract'
import { decodeAge } from '../../../features/age-status/controller'
import { page as notificationPage } from '../../../features/inbox/model/contract'
import { decode, handleList, requestList } from '../../../features/social-sharing/model/contract'

/** Read-only subset of the incumbent World port; creates no transport or authority. */
export interface IncumbentSocialWorld {
  read(): { readonly release: WorldEntryRequest; readonly socialPolicy?: {
    read(): { readonly scope: PrivacyScope; readonly eventId: string; readonly streamId: string;
      readonly roomEpoch: string; readonly policyRevision: number } | null
    subscribe(listener: () => void): () => void
  } } | null
  subscribe(listener: () => void): () => void
}
type Binding = NonNullable<ReturnType<IncumbentSocialWorld['read']>>
type Policy = NonNullable<ReturnType<NonNullable<Binding['socialPolicy']>['read']>>
interface Round { readonly binding: Binding; readonly policy: Policy; readonly controller: AbortController }
export interface ProductionSocialReadiness extends OwnedSocialAvailability {
  prepare(signal: AbortSignal): Promise<void>
  dispose(): void
}
const unavailable = (): Record<SocialService, boolean> => ({ feed: false, people: false, intro: false, age: false, inbox: false, requests: false })

/** Pure construction. Root's committed availability lifetime calls prepare/dispose. */
export function createProductionSocialReadiness(context: SocialContext, world?: IncumbentSocialWorld): ProductionSocialReadiness {
  const { auth, bootstrap } = context, scope = Object.freeze({ ...context.scope })
  const base = `/api/concert/events/${scope.eventId}`
  const listeners = new Set<() => void>()
  let started = false, disposed = false, round: Round | null = null, lease: SocialAvailability | null = null
  let observed: Binding | null = null
  let stopWorld = () => {}, stopPolicy = () => {}, stopPrivacy = () => {}, stopCaller = () => {}, stopRelease = () => {}
  const current = () => !disposed && auth.privacy.isCurrent(scope) && sameScope(auth.privacy.capture(), scope)
    && auth.session.isCurrent(scope) && bootstrap.participant.userId === scope.userId
    && bootstrap.participant.eventId === scope.eventId && bootstrap.event.eventId === scope.eventId
    && ['OPEN', 'LIVE', 'READ_ONLY'].includes(bootstrap.event.phase) && validCapabilities(bootstrap.capabilities)
  function authority(): { binding: Binding; policy: Policy } | null {
    try {
      if (!current()) return null
      const binding = world?.read(), policy = binding?.socialPolicy?.read()
      if (!binding || !policy || binding.release.signal.aborted || binding.release.bootstrap !== bootstrap
        || !sameScope(binding.release.privacyScope, scope) || !auth.session.isCurrent(binding.release.scope)
        || binding.release.scope.userId !== scope.userId || binding.release.scope.eventId !== scope.eventId
        || binding.release.scope.generation !== scope.generation || !sameScope(policy.scope, scope)
        || policy.eventId !== scope.eventId || !Number.isSafeInteger(policy.policyRevision) || policy.policyRevision < 1
        || !policy.streamId || policy.streamId.length > 128 || !policy.roomEpoch || policy.roomEpoch.length > 128) return null
      return { binding, policy }
    } catch { return null }
  }
  function matches(candidate: Round) {
    const active = authority()
    return started && round === candidate && !candidate.controller.signal.aborted
      && active?.binding === candidate.binding && active.policy === candidate.policy
  }
  const notify = () => { for (const listener of Array.from(listeners)) { try { listener() } catch { /* Complete independent withdrawal. */ } } }
  function withdraw() {
    const old = round; round = null; lease = null
    old?.controller.abort()
    notify()
  }
  function dispose() {
    if (disposed) return
    disposed = true; withdraw()
    for (const stop of [stopWorld, stopPolicy, stopRelease, stopPrivacy, stopCaller]) { try { stop() } catch { /* Complete remaining cleanup. */ } }
    observed = null; listeners.clear()
  }
  async function read<T>(candidate: Round, path: string, operation: NonNullable<HttpRequest<T>['domainOperation']>, parse: Decoder<T>, query?: HttpRequest<T>['query']) {
    if (!matches(candidate)) throw Error('Service readiness withdrawn')
    await auth.privacy.run(scope, async ({ signal }) => {
      if (!matches(candidate)) throw Error('Service readiness withdrawn')
      await auth.request({ method: 'GET', access: 'protected', errorFamily: 'domain', domainOperation: operation,
        path: base + path, ...(query ? { query } : {}), signal, timeoutMs: 12_000,
        decode(value, response) {
          if (!matches(candidate) || response.status !== 200 || response.path !== base + path
            || response.scope.userId !== scope.userId || response.scope.eventId !== scope.eventId
            || response.scope.generation !== scope.generation) throw Error('Service readiness response unavailable')
          parse(value, response)
          if (!matches(candidate)) throw Error('Service readiness withdrawn')
          // Discard bodies, private handles and claims; retain only a qualified service bit.
          return undefined
        } })
    }, candidate.controller.signal)
    if (!matches(candidate)) throw Error('Service readiness withdrawn')
  }
  async function probe(service: Exclude<SocialService, 'feed'>, candidate: Round) {
    switch (service) {
      case 'people': await read(candidate, '/intros', 'listConcertIntros', value => decodePage(value, scope), { limit: 20 }); break
      case 'intro': await read(candidate, '/me/intro', 'getOwnConcertIntro', value => decodeOwn(value, scope)); break
      case 'age': await read(candidate, '/me/age-status', 'getConcertAgeStatus', value => decodeAge(value, scope)); break
      case 'inbox': await read(candidate, '/me/notifications', 'listConcertNotifications', value => notificationPage(value, scope, false), { limit: 20, unread_only: false }); break
      case 'requests':
        if (!bootstrap.capabilities.social_sharing.available) return false
        await read(candidate, '/me/social-handles', 'listOwnConcertHandles', decode(handleList, value => value.items.every(row => row.user_id === scope.userId)))
        await read(candidate, '/me/social-requests', 'listConcertSocialRequests', decode(requestList, value => value.items.every(row => row.event_id === scope.eventId && row.recipient_id === scope.userId)), { direction: 'INCOMING', limit: 20 }); break
    }
    return true
  }
  function reconcile() {
    if (!started || disposed) return
    if (!current()) { dispose(); return }
    let binding: Binding | null
    try { binding = world?.read() ?? null } catch { withdraw(); return }
    if (binding !== observed) {
      withdraw(); stopPolicy(); stopRelease(); stopPolicy = () => {}; stopRelease = () => {}; observed = binding
      if (binding) {
        binding.release.signal.addEventListener('abort', reconcile, { once: true })
        stopRelease = () => binding.release.signal.removeEventListener('abort', reconcile)
        stopPolicy = binding.socialPolicy?.subscribe(reconcile) ?? (() => {})
      }
    }
    const active = authority()
    if (active?.binding === round?.binding && active?.policy === round?.policy) return
    withdraw()
    if (!active || !started || disposed) return
    const candidate: Round = { ...active, controller: new AbortController() }; round = candidate
    lease = Object.freeze({ bootstrap, scope, services: Object.freeze({ ...unavailable(), feed: bootstrap.capabilities.feed.available }) })
    notify()
    for (const service of services) {
      if (service === 'feed' || !matches(candidate)) continue
      void probe(service, candidate).then(ready => {
        if (!ready || !matches(candidate) || !lease) return
        lease = Object.freeze({ bootstrap, scope, services: Object.freeze({ ...lease.services, [service]: true }) }); notify()
      }).catch(() => { /* This producer remains unavailable; no retries or synthetic success. */ })
    }
  }
  return {
    read() { return started && round && matches(round) ? lease : null },
    subscribe(listener) { if (disposed) return () => {}; listeners.add(listener); return () => { listeners.delete(listener) } },
    async prepare(signal) {
      if (disposed || signal.aborted) { dispose(); return }
      if (started) return
      started = true
      try {
        if (!current()) { dispose(); return }
        signal.addEventListener('abort', dispose, { once: true }); stopCaller = () => signal.removeEventListener('abort', dispose)
        stopPrivacy = auth.privacy.registerCleanup(dispose)
        stopWorld = world?.subscribe(reconcile) ?? (() => {})
        reconcile()
      } catch { dispose() }
    }, dispose,
  }
}
