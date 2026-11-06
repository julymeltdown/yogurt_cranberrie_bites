import type { CapabilitySnapshot, MeetingConsumerPort, ReadPort, VenueAuthority, VenueAvailability, VenueCapabilities, VenueView, VenueReadContext } from '../../features/paradise-city-map/ports'
import type { MeetingClockPort } from '../meeting-entry/ports'
import type { createVenueLocationAdapter } from '../../features/paradise-city-map/model/location-adapter'
import type { CommunityBootstrapBinding } from '../community-adoption/bootstrap'
import type { PrivacyScope } from '../../shared/query/privacy'

const reject = async () => ({ state: 'rejected' as const })
const unavailableAvailability = (reason: VenueAvailability['reason'] = 'producer-refused'): VenueAvailability =>
  ({ state: 'unavailable', reason, readAtMs: null, ageMs: null, staleAtMs: null, refreshing: false, canRefresh: false })
/** No guessed transport or endpoint. Root injects frozen wire-backed producers later. */
export const unavailableVenueCapabilities: VenueCapabilities = Object.freeze({
  read: () => null, subscribe: () => () => {}, availability: () => unavailableAvailability(), refresh: async () => false,
  beginPhoneSharing: reject, publishPhoneFix: reject,
  pause: reject, revoke: reject, requestLocation: reject, checkIn: reject, postNote: reject, reply: reject, withdraw: reject, resolve: reject,
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
  const guarded = (context: Parameters<VenueCapabilities['beginPhoneSharing']>[0]) => !context.signal.aborted && sameScope(context.scope, scope) && clock() !== null
  return {
    read() { const now = clock(), value = now ? source.read() : null; return value && now ? { ...value, serverNowMs: now.nowMs } : null },
    subscribe(listener) { const stops = [source.subscribe(listener), binding?.read()?.clock.subscribe(listener)]; return () => stops.forEach(stop => stop?.()) },
    availability() { return clock() === null ? unavailableAvailability('access-unavailable') : source.availability?.() ?? unavailableAvailability() },
    refresh: () => clock() === null ? Promise.resolve(false) : source.refresh ? source.refresh() : Promise.resolve(false),
    loadNotes: (id, ctx) => !ctx.signal.aborted && sameScope(ctx.scope, scope) && clock() !== null && source.loadNotes ? source.loadNotes(id, ctx) : Promise.resolve(false),
    loadNoteThread: (id, ctx) => !ctx.signal.aborted && sameScope(ctx.scope, scope) && clock() !== null && source.loadNoteThread ? source.loadNoteThread(id, ctx) : Promise.resolve(false),
    loadNoteSummary: ctx => !ctx.signal.aborted && sameScope(ctx.scope, scope) && clock() !== null && source.loadNoteSummary ? source.loadNoteSummary(ctx) : Promise.resolve(false),
    beginPhoneSharing: ctx => guarded(ctx) ? source.beginPhoneSharing(ctx) : reject(),
    publishPhoneFix: (fix, ctx) => guarded(ctx) ? source.publishPhoneFix(fix, ctx) : reject(),
    pause: (reason, ctx) => guarded(ctx) ? source.pause(reason, ctx) : reject(),
    revoke: ctx => guarded(ctx) && source.revoke ? source.revoke(ctx) : reject(),
    requestLocation: (id, ctx) => guarded(ctx) ? source.requestLocation(id, ctx) : reject(),
    decideLocationRequest: (id, decision, ctx) => guarded(ctx) && source.decideLocationRequest ? source.decideLocationRequest(id, decision, ctx) : reject(),
    checkIn: (id, ctx) => guarded(ctx) ? source.checkIn(id, ctx) : reject(),
    postNote: (input, ctx) => guarded(ctx) ? source.postNote(input, ctx) : reject(),
    reply: (input, ctx) => guarded(ctx) ? source.reply(input, ctx) : reject(),
    withdraw: (input, ctx) => guarded(ctx) ? source.withdraw(input, ctx) : reject(),
    resolve: (key, ctx) => guarded(ctx) ? source.resolve(key, ctx) : reject(),
  }
}

type NotesProjection = Pick<CapabilitySnapshot, 'scope' | 'venue' | 'classification' | 'serverNowMs' | 'accessUntilMs' | 'notes' | 'noteClusters' | 'notesVisibleCount'> & {
  readonly rights: Pick<CapabilitySnapshot['rights'], 'postNote' | 'noteAudiences' | 'noteExpiries'>
}
export type VenueNotesSource = ReadPort<NotesProjection | null> & Pick<VenueCapabilities, 'postNote' | 'reply' | 'withdraw' | 'resolve'> & {
  ownsCommand(key: string): boolean
  load?(id: VenueView['landmarks'][number]['id'], signal: AbortSignal): Promise<boolean>
  loadThread?(id: string, signal: AbortSignal): Promise<boolean>
  loadSummary?(signal: AbortSignal): Promise<boolean>
}
type LocationSource = Pick<ReturnType<typeof createVenueLocationAdapter>, 'read' | 'subscribe' | 'availability' | 'refresh' | 'beginPhoneSharing' | 'publishPhoneFix' | 'pause' | 'revoke' | 'requestLocation' | 'decideRequest' | 'checkIn' | 'resolve' | 'ownsCommand'>
const sameScope = (a: PrivacyScope, b: PrivacyScope) => a.userId === b.userId && a.eventId === b.eventId && a.generation === b.generation && a.policyGeneration === b.policyGeneration && a.gate === b.gate
const sameVenue = (a: VenueView['identity'], b: VenueView['identity']) => a.venueId === b.venueId && a.siteplanId === b.siteplanId && a.revision === b.revision && a.variantId === b.variantId

/** Independent L/N leases; neither missing producer suppresses the other domain. */
export function aggregateVenueCapabilities({ authority, venue, clock, location, notes }: {
  readonly authority: VenueAuthority; readonly venue: VenueView; readonly clock: MeetingClockPort
  readonly location: LocationSource | null; readonly notes: VenueNotesSource | null
}): VenueCapabilities {
  function now() {
    if (authority.signal.aborted || !authority.isCurrent()) return null
    const value = clock.read()
    return value && sameScope(value.scope, authority.scope) && value.scope.gate === 'open' && Number.isFinite(value.nowMs) ? value.nowMs : null
  }
  function checked<T extends Pick<CapabilitySnapshot, 'scope' | 'venue' | 'serverNowMs' | 'accessUntilMs'>>(value: T | null, time: number): T | null {
    return value && sameScope(value.scope, authority.scope) && sameVenue(value.venue, venue.identity)
      && value.serverNowMs !== null && Number.isFinite(value.serverNowMs) && value.accessUntilMs !== null && value.accessUntilMs > time ? value : null
  }
  function valid(context: VenueReadContext) { return !context.signal.aborted && sameScope(context.scope, authority.scope) && sameVenue(context.venue, venue.identity) && now() !== null }
  return {
    read() {
      const time = now(); if (time === null) return null
      const l = checked(location?.read() ?? null, time), n = checked(notes?.read() ?? null, time)
      if ((!l && !n) || now() === null) return null
      return { scope: authority.scope, venue: venue.identity, classification: l?.classification === 'CONTROLLED_FIXTURE' || n?.classification === 'CONTROLLED_FIXTURE' ? 'CONTROLLED_FIXTURE' : 'AUTHORITATIVE_ADAPTER',
        serverNowMs: time, accessUntilMs: Math.min(l?.accessUntilMs ?? Infinity, n?.accessUntilMs ?? Infinity),
        self: l?.self ?? null, friends: l?.friends ?? [], notes: n?.notes ?? [], noteClusters: n?.noteClusters ?? [],
        notesVisibleCount: n?.notesVisibleCount ?? null, sharing: l?.sharing ?? 'off',
        locationRequests: l?.locationRequests ?? { incoming: [], outgoing: [] },
        rights: { sharePhone: l?.rights.sharePhone ?? false, pause: l?.rights.pause ?? false, revoke: l?.rights.revoke ?? false, checkIn: l?.rights.checkIn ?? [],
          postNote: n?.rights.postNote ?? [], noteAudiences: n?.rights.noteAudiences ?? [], noteExpiries: n?.rights.noteExpiries ?? [] } }
    },
    subscribe(listener) {
      const stops = [authority.subscribe(listener), clock.subscribe(listener), location?.subscribe(listener), notes?.subscribe(listener)]
      authority.signal.addEventListener('abort', listener, { once: true })
      return () => { for (const stop of stops) stop?.(); authority.signal.removeEventListener('abort', listener) }
    },
    availability() {
      if (!authority.isCurrent() || now() === null) return unavailableAvailability('access-unavailable')
      return location?.availability?.() ?? unavailableAvailability()
    },
    refresh() { return now() === null || !location?.refresh ? Promise.resolve(false) : location.refresh() },
    loadNotes: (id, context) => valid(context) && venue.landmarks.some(p => p.id === id) && notes?.load ? notes.load(id, context.signal) : Promise.resolve(false),
    loadNoteThread: (id, context) => valid(context) && notes?.loadThread ? notes.loadThread(id, context.signal) : Promise.resolve(false),
    loadNoteSummary: context => valid(context) && notes?.loadSummary ? notes.loadSummary(context.signal) : Promise.resolve(false),
    beginPhoneSharing: context => valid(context) && location ? location.beginPhoneSharing(context) : reject(),
    publishPhoneFix: (fix, context) => valid(context) && location ? location.publishPhoneFix(fix, context) : reject(),
    pause: (reason, context) => valid(context) && location ? location.pause(reason, context) : reject(),
    revoke: context => valid(context) && location?.revoke ? location.revoke(context) : reject(),
    requestLocation: (id, context) => valid(context) && location ? location.requestLocation(id, context) : reject(),
    decideLocationRequest: (id, decision, context) => valid(context) && location ? location.decideRequest(id, decision, context) : reject(),
    checkIn: (id, context) => valid(context) && location ? location.checkIn(id, context) : reject(),
    postNote: (input, context) => valid(context) && notes ? notes.postNote(input, context) : reject(),
    reply: (input, context) => valid(context) && notes ? notes.reply(input, context) : reject(),
    withdraw: (input, context) => valid(context) && notes ? notes.withdraw(input, context) : reject(),
    resolve(key, context) {
      if (key !== context.key || !valid(context)) return reject()
      const l = location?.ownsCommand(key) ?? false, n = notes?.ownsCommand(key) ?? false
      // Exact originating owner only. No key-prefix inference, probing or replay.
      return l !== n ? l ? location!.resolve(key, context) : notes!.resolve(key, context) : reject()
    },
  }
}
