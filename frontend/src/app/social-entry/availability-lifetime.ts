import { sameScope } from '../social-composition/owner'
import { services, type SocialAvailability, type SocialAvailabilityPort, type SocialContext } from '../social-composition/ports'
import { validCapabilities } from '../capabilities'

export interface OwnedSocialAvailability extends SocialAvailabilityPort {
  prepare?(signal: AbortSignal): Promise<void>
  dispose?(): void
}
export type SocialAvailabilityFactory = (context: SocialContext) => OwnedSocialAvailability | null

function current({ auth, bootstrap, scope }: SocialContext) {
  return auth.privacy.isCurrent(scope) && sameScope(auth.privacy.capture(), scope)
    && bootstrap.participant.userId === scope.userId && bootstrap.participant.eventId === scope.eventId
    && bootstrap.event.eventId === scope.eventId && ['OPEN', 'LIVE', 'READ_ONLY'].includes(bootstrap.event.phase)
    && validCapabilities(bootstrap.capabilities)
}

/** Root's reader must already qualify its incumbent SOCIAL policy AND configured service readiness.
 * This adapter grants neither from C03 alone, and never creates HTTP/Auth/World resources.
 * The reader is borrowed: its owner retains its lifetime and socket.
 */
export function qualifiedSocialAvailability(context: SocialContext, reader?: SocialAvailabilityPort | null): SocialAvailabilityPort {
  let previous: SocialAvailability | null = null, filtered: SocialAvailability | null = null
  return {
    read() {
      try {
        if (!reader || !current(context)) return null
        const value = reader.read()
        if (!value || value.bootstrap !== context.bootstrap || !sameScope(value.scope, context.scope)
          || Object.keys(value.services).some(key => !(services as readonly string[]).includes(key)
            || typeof value.services[key as keyof typeof value.services] !== 'boolean')) return null
        if (value !== previous) {
          previous = value
          filtered = Object.freeze({ bootstrap: context.bootstrap, scope: context.scope, services: Object.freeze({ ...value.services,
            feed: value.services.feed === true && context.bootstrap.capabilities.feed.available,
            requests: value.services.requests === true && context.bootstrap.capabilities.social_sharing.available }) })
        }
        return filtered
      } catch { return null }
    },
    subscribe(listener) { return reader?.subscribe(listener) ?? (() => {}) },
  }
}

/** Pure construction. start() belongs to a committed effect; dispose() withdraws before cleanup. */
export function createSocialAvailabilityLifetime(context: SocialContext, factory: SocialAvailabilityFactory) {
  const lifetime = new AbortController(), listeners = new Set<() => void>()
  let started = false, disposed = false, prepared = false
  let owned: OwnedSocialAvailability | null = null, qualified: SocialAvailabilityPort | null = null
  let stopReader = () => {}, stopPrivacy = () => {}
  const notify = () => { for (const listener of Array.from(listeners)) { try { listener() } catch { /* Finish withdrawal for all consumers. */ } } }
  function dispose() {
    if (disposed) return
    disposed = true; prepared = false
    const candidate = owned; owned = null; qualified = null
    lifetime.abort()
    for (const stop of [stopReader, stopPrivacy, () => candidate?.dispose?.()]) { try { stop() } catch { /* Complete independent cleanup. */ } }
    notify(); listeners.clear()
  }
  return {
    read: () => !disposed && prepared && current(context) ? qualified?.read() ?? null : null,
    subscribe(listener: () => void) { if (disposed) return () => {}; listeners.add(listener); return () => { listeners.delete(listener) } },
    start() {
      if (started || disposed) return
      started = true
      try {
        if (!current(context)) { dispose(); return }
        stopPrivacy = context.auth.privacy.registerCleanup(dispose)
        const candidate = factory(context)
        if (disposed || !current(context)) { candidate?.dispose?.(); dispose(); return }
        owned = candidate
        if (!candidate) { notify(); return }
        qualified = qualifiedSocialAvailability(context, candidate)
        const stop = candidate.subscribe(notify)
        if (disposed) { stop(); return }
        stopReader = stop
        if (candidate.prepare) {
          void Promise.resolve().then(async () => {
            if (!disposed && !lifetime.signal.aborted) await candidate.prepare?.(lifetime.signal)
          }).then(() => { if (!disposed && current(context)) { prepared = true; notify() } }).catch(dispose)
        } else { prepared = true; notify() }
      } catch { dispose() }
    },
    dispose,
  }
}
