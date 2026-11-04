import type { Decoder, HttpRequest } from '../../shared/api/http'
import { sameScope } from '../social-composition/owner'
import { services, type SocialAvailability, type SocialAvailabilityPort, type SocialContext, type SocialService } from '../social-composition/ports'
import { validCapabilities } from '../capabilities'
import { decodePage } from '../../features/people/contract'
import { decodeOwn } from '../../features/intro/contract'
import { decodeAge } from '../../features/age-status/controller'
import { decoder, type Page, type Post } from '../../features/event-feed/contract'
import { page as notificationPage } from '../../features/inbox/model/contract'
import { decode, handleList, requestList } from '../../features/social-sharing/model/contract'

/** The serving owner pins this to the actual adapter source commit. No default revision. */
export const sourceRevision: string = import.meta.env.VITE_MCR_IN03_AVAILABILITY_REVISION ?? ''

export interface IN03AvailabilityPort extends SocialAvailabilityPort {
  /** Explicit bounded reads, never a render-time side effect or an authorization grant. */
  prepare(signal?: AbortSignal): Promise<void>
  dispose(): void
}
const unavailable = () => Object.freeze(Object.fromEntries(services.map(service => [service, false])) as Record<SocialService, boolean>)

/** No Auth/cache/World/socket is created. Successful bodies are validated and immediately discarded. */
export function createIN03Availability(context: SocialContext): IN03AvailabilityPort | null {
  const { auth, bootstrap } = context, scope = Object.freeze({ ...context.scope })
  const eligible = () => auth.privacy.isCurrent(scope) && sameScope(auth.privacy.capture(), scope)
    && bootstrap.participant.userId === scope.userId && bootstrap.participant.eventId === scope.eventId
    && bootstrap.event.eventId === scope.eventId && ['OPEN', 'LIVE', 'READ_ONLY'].includes(bootstrap.event.phase)
    && validCapabilities(bootstrap.capabilities)
  if (!eligible()) return null
  let disposed = false, generation = 0, flight: AbortController | null = null
  let stopPrivacy = () => {}, stopCaller = () => {}, observing = false
  let lease: SocialAvailability = Object.freeze({ bootstrap, scope, services: unavailable() })
  const listeners = new Set<() => void>()
  const live = () => !disposed && eligible()
  const notify = () => {
    const pending = Array.from(listeners)
    for (const listener of pending) { try { listener() } catch { /* Complete other consumers' withdrawal. */ } }
  }
  function replace(flags: Readonly<Record<SocialService, boolean>>) {
    lease = Object.freeze({ bootstrap, scope, services: Object.freeze({ ...flags }) }); notify()
  }
  function dispose() {
    if (disposed) return
    disposed = true; generation++; flight?.abort(); flight = null; stopCaller(); stopPrivacy()
    replace(unavailable()); listeners.clear()
  }
  const base = `/api/concert/events/${scope.eventId}`
  async function read<T>(controller: AbortController, path: string, operation: HttpRequest<T>['domainOperation'], parse: Decoder<T>, query?: HttpRequest<T>['query']) {
    await auth.privacy.run(scope, async ({ signal }) => {
      await auth.request({ method: 'GET', access: 'protected', errorFamily: 'domain',
        ...(operation ? { domainOperation: operation } : {}), path: base + path,
        ...(query ? { query } : {}), signal, timeoutMs: 12_000,
        decode(value, response) {
          if (response.status !== 200 || response.path !== base + path || response.scope.userId !== scope.userId
            || response.scope.eventId !== scope.eventId || response.scope.generation !== scope.generation) throw Error('Readiness response scope unavailable')
          parse(value, response)
          // No private response data is stored in the readiness port or its result.
          return undefined
        } })
    }, controller.signal)
  }
  async function probe(service: SocialService, controller: AbortController) {
    switch (service) {
      case 'feed':
        if (!bootstrap.capabilities.feed.available) return false
        await read(controller, '/feed', 'listConcertFeed', decoder<Page<Post>>('Domain_feed_FeedPage', value => value.items.every(row => row.event_id === scope.eventId)), { limit: 20, filter: 'ALL' }); break
      case 'people':
        await read(controller, '/intros', 'listConcertIntros', value => decodePage(value, scope), { limit: 20 }); break
      case 'intro':
        await read(controller, '/me/intro', 'getOwnConcertIntro', value => decodeOwn(value, scope)); break
      case 'age':
        await read(controller, '/me/age-status', 'getConcertAgeStatus', value => decodeAge(value, scope)); break
      case 'inbox':
        await read(controller, '/me/notifications', 'listConcertNotifications', value => notificationPage(value, scope, false), { limit: 20, unread_only: false }); break
      case 'requests':
        if (!bootstrap.capabilities.social_sharing.available) return false
        await read(controller, '/me/social-handles', 'listOwnConcertHandles', decode(handleList, value => value.items.every(row => row.user_id === scope.userId)))
        await read(controller, '/me/social-requests', 'listConcertSocialRequests', decode(requestList, value => value.items.every(row => row.event_id === scope.eventId && row.recipient_id === scope.userId)), { direction: 'INCOMING', limit: 20 }); break
    }
    return true
  }
  return {
    read: () => live() ? lease : null,
    subscribe(listener) { if (!live()) return () => {}; listeners.add(listener); return () => { listeners.delete(listener) } },
    async prepare(signal) {
      if (!live() || signal?.aborted) { dispose(); return }
      if (!observing) {
        observing = true
        try { stopPrivacy = auth.privacy.registerCleanup(dispose) } catch { dispose(); return }
      }
      const ticket = ++generation
      flight?.abort()
      const controller = new AbortController(); flight = controller
      stopCaller()
      if (signal) {
        signal.addEventListener('abort', dispose, { once: true })
        stopCaller = () => { signal.removeEventListener('abort', dispose) }
      } else stopCaller = () => {}
      replace(unavailable())
      try {
        const results = await Promise.allSettled(services.map(service => probe(service, controller)))
        if (!live() || controller.signal.aborted || generation !== ticket) return
        replace(Object.fromEntries(services.map((service, index) => {
          const result = results[index]
          return [service, result?.status === 'fulfilled' && result.value === true]
        })) as Record<SocialService, boolean>)
      } finally {
        if (flight === controller) flight = null
      }
    }, dispose,
  }
}

/** Synchronous construction stays unavailable until the caller explicitly awaits prepare(). */
export function availability(context: SocialContext): IN03AvailabilityPort | null {
  return /^[0-9a-f]{40}$/.test(sourceRevision) ? createIN03Availability(context) : null
}
