import type { MeetingConsumerPort, VenueCapabilities } from '../../features/paradise-city-map/ports'
import type { CommunityBootstrapBinding } from '../community-adoption/bootstrap'
import type { PrivacyScope } from '../../shared/query/privacy'

const reject = async () => ({ state: 'rejected' as const })
/** No guessed transport or endpoint. Root injects frozen wire-backed producers later. */
export const unavailableVenueCapabilities: VenueCapabilities = Object.freeze({
  read: () => null, subscribe: () => () => {}, beginPhoneSharing: reject, publishPhoneFix: reject,
  pause: reject, requestLocation: reject, checkIn: reject, postNote: reject, reply: reject, resolve: reject,
})
export const unavailableMeetingConsumer: MeetingConsumerPort = Object.freeze({
  read: () => null, subscribe: () => () => {}, open: async () => ({ kind: 'unavailable' as const }), close: () => true,
})

/** Uses the SAME existing qualified Community clock. No bootstrap request or timeline. */
export function bindCapabilitiesToCommunity(source: VenueCapabilities, binding: CommunityBootstrapBinding | null, scope: PrivacyScope, isCurrent: () => boolean): VenueCapabilities {
  function clock() {
    try {
      if (!isCurrent() || !binding?.isCurrent()) return null
      const value = binding.read()?.clock.read(), s = value?.scope
      return value && s && s.userId === scope.userId && s.eventId === scope.eventId && s.generation === scope.generation
        && s.policyGeneration === scope.policyGeneration && s.gate === 'open' ? value : null
    } catch { return null }
  }
  const guarded = (context: Parameters<VenueCapabilities['beginPhoneSharing']>[0]) => !context.signal.aborted && clock() !== null
  return {
    read() { const now = clock(), value = now ? source.read() : null; return value && now ? { ...value, serverNowMs: now.nowMs } : null },
    subscribe(listener) { const stops = [source.subscribe(listener), binding?.read()?.clock.subscribe(listener)]; return () => stops.forEach(stop => stop?.()) },
    beginPhoneSharing: ctx => guarded(ctx) ? source.beginPhoneSharing(ctx) : reject(),
    publishPhoneFix: (fix, ctx) => guarded(ctx) ? source.publishPhoneFix(fix, ctx) : reject(),
    pause: (reason, ctx) => guarded(ctx) ? source.pause(reason, ctx) : reject(),
    requestLocation: (id, ctx) => guarded(ctx) ? source.requestLocation(id, ctx) : reject(),
    checkIn: (id, ctx) => guarded(ctx) ? source.checkIn(id, ctx) : reject(),
    postNote: (input, ctx) => guarded(ctx) ? source.postNote(input, ctx) : reject(),
    reply: (input, ctx) => guarded(ctx) ? source.reply(input, ctx) : reject(),
    resolve: (key, ctx) => guarded(ctx) ? source.resolve(key, ctx) : reject(),
  }
}
