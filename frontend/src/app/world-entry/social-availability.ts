import type { SocialAvailability, SocialAvailabilityPort, SocialContext } from '../social-composition/ports'
import type { WorldAccessBindingPort } from '../ports'
import { samePrivacy } from '../../world/composition/release'

/** Read the incumbent stream and configured C03 Feed producer, without probing Feed. */
export function createWorldSocialAvailability(context: SocialContext, world?: WorldAccessBindingPort): SocialAvailabilityPort {
  let lastPolicy: unknown = null, snapshot: SocialAvailability | null = null
  const read = () => {
    const binding = world?.read(), policy = binding?.socialPolicy?.read()
    if (!context.auth.privacy.isCurrent(context.scope) || !binding || !policy
      || binding.release.bootstrap !== context.bootstrap || binding.release.signal.aborted
      || !samePrivacy(binding.release.privacyScope, context.scope) || !samePrivacy(policy.scope, context.scope)
      || policy.eventId !== context.scope.eventId || !context.bootstrap.capabilities.feed.available) {
      lastPolicy = null; snapshot = null; return null
    }
    if (lastPolicy !== policy) {
      lastPolicy = policy
      snapshot = Object.freeze({ scope: context.scope, bootstrap: context.bootstrap,
        services: Object.freeze({ feed: true, people: false, intro: false, age: false, inbox: false, requests: false }) })
    }
    return snapshot
  }
  return { read,
    subscribe(listener) {
      if (!world) return () => {}
      let closed = false, stopPolicy = () => {}
      const notify = () => { if (!closed) { try { listener() } catch { /* Independent observer. */ } } }
      const bind = () => {
        stopPolicy(); stopPolicy = () => {}
        if (closed) return
        stopPolicy = world.read()?.socialPolicy?.subscribe(notify) ?? (() => {})
        notify()
      }
      const stopWorld = world.subscribe(bind)
      bind()
      return () => { closed = true; try { stopWorld() } finally { stopPolicy() } }
    },
  }
}
