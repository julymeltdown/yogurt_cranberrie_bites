import type { SocialProductionPorts } from '../world-panels/social'
import { sameScope } from '../social-composition/owner'
import { services, type SocialAvailability, type SocialAvailabilityPort, type SocialContext, type SocialOptions } from '../social-composition/ports'

/** A supplied production source owns readiness. This leaf defines no new HTTP authority. */
export interface FeedEntryOptions {
  readonly readiness?: (context: SocialContext) => SocialAvailabilityPort | null
  /** Borrow the existing reviewed media producer; absent means no upload producer. */
  readonly media?: SocialOptions['media']
}

/** Root wires this leaf into its existing production ports. No Auth, renderer or socket is created. */
export function createFeedSocialPorts(options: FeedEntryOptions = {}): SocialProductionPorts {
  return {
    availability(context) {
      let source: SocialAvailabilityPort | null
      try { source = options.readiness?.(context) ?? null } catch { return null }
      if (!source) return null
      const producer = source
      const scope = Object.freeze({ ...context.scope })
      let cached: SocialAvailability | null = null
      let failed = false
      const read = (): SocialAvailability | null => {
        try {
          const lease = producer.read()
          if (failed || !context.auth.privacy.isCurrent(scope) || !lease || lease.bootstrap !== context.bootstrap
            || !sameScope(lease.scope, scope) || Object.keys(lease.services).some(key =>
              !(services as readonly string[]).includes(key) || typeof lease.services[key as keyof typeof lease.services] !== 'boolean')) {
            cached = null; return null
          }
          const ready = lease.services.feed === true && context.bootstrap.capabilities.feed.available
          if (cached?.services.feed === ready) return cached
          cached = Object.freeze({ scope, bootstrap: context.bootstrap, services: Object.freeze({
            feed: ready, people: false, intro: false, age: false, inbox: false, requests: false,
          }) })
          return cached
        } catch { cached = null; return null }
      }
      return { read, subscribe(listener) {
        const stops: (() => void)[] = []
        try {
          stops.push(producer.subscribe(listener))
          stops.push(context.auth.privacy.subscribe(() => { cached = null; listener() }))
        } catch {
          failed = true; cached = null
          for (const stop of stops.splice(0).reverse()) { try { stop() } catch { /* Release independent observers. */ } }
          return () => {}
        }
        return () => { for (const stop of stops.splice(0).reverse()) { try { stop() } catch { /* Release every borrowed observer. */ } } }
      } }
    },
    ...(options.media ? { media: options.media } : {}),
  }
}
