import type { PrivacyScope } from '../../../../shared/query/privacy'
import type { CapabilitySnapshot, CommandContext, CommandResult, GeoFix, MeetingConsumerPort, MeetingLeafSnapshot, MeetingOpenIntent,
  PersonView, PhoneLocationPort, PhoneStatus, VenueAvailability, VenueCapabilities, VenueView } from '../../ports'
import { createVenueMapController } from '../../model/controller'
import type { LandmarkId } from '../../spatial/contract'

// Synthetic-only IDs, not a real producer registry or physical-place authorization.
export const mainGate = 'test-main-gate' as LandmarkId, subGate = 'test-sub-gate' as LandmarkId, station = 'test-station' as LandmarkId

/** Explicit synthetic adapter composition. No real location, friend policy or place approval. */
export const testScope: PrivacyScope = { generation: 1, policyGeneration: 0, gate: 'open', userId: 'actor-test', eventId: 'event-test' }
export const testVenue: VenueView = {
  identity: { venueId: 'paradise-city', siteplanId: 'paradise-city-siteplan', revision: 1, variantId: 'photo-5-provisional' },
  title: 'Paradise City', classification: 'ILLUSTRATED_UNCALIBRATED',
  image: { src: '/venue/paradise-city/v1/siteplan-photo-5.jpg', width: 1024, height: 1280, description: 'Paradise City illustrated siteplan, uncalibrated' },
  landmarks: [
    { id: mainGate, label: 'Main Gate', point: { x: 290, y: 615 }, meetingContext: { landmarkId: 'synthetic-real-main-gate', catalogRevision: 'synthetic-catalog-1', landmarkRevision: 'synthetic-policy-1' } },
    { id: subGate, label: 'Sub Gate', point: { x: 634, y: 552 }, meetingContext: { landmarkId: 'synthetic-real-sub-gate', catalogRevision: 'synthetic-catalog-1', landmarkRevision: 'synthetic-policy-1' } },
    { id: station, label: 'Station', point: { x: 180, y: 770 }, meetingContext: null },
  ],
}
const at = 1800000000000
export function person(userId = 'friend-test', displayName = 'inkparade'): PersonView {
  return { userId, displayName, avatar: null, point: { x: 310, y: 630 }, landmarkId: mainGate, locationSource: 'phone', updatedAtMs: at - 40000, freshUntilMs: at + 90000, visible: true, canRequest: true, canMessage: true, canMeet: true }
}
export function initialSnapshot(): CapabilitySnapshot {
  const friend = person()
  return { scope: testScope, venue: testVenue.identity, classification: 'CONTROLLED_FIXTURE', serverNowMs: at, accessUntilMs: at + 7200000,
    self: person('actor-test', 'You'), friends: [friend], sharing: 'off',
    rights: { sharePhone: true, pause: true, revoke: true, checkIn: [mainGate], postNote: [mainGate, subGate], noteAudiences: ['FRIENDS', 'EVERYONE'], noteExpiries: ['ONE_HOUR', 'END_OF_DAY'] },
    noteClusters: [{ landmarkId: mainGate, count: 1 }], notesVisibleCount: 1,
    notes: [{ id: 'note-test', landmarkId: mainGate, author: friend, body: 'Synthetic place note for a controlled fixture.', createdAtMs: at - 300000, expiresAtMs: at + 3600000, audience: 'FRIENDS', revision: 4, replyCount: 0, canReply: true, canMeet: true, canWithdraw: false, replies: [] }] }
}
export function harness() {
  let snapshot: CapabilitySnapshot | null = initialSnapshot(), current = true, phoneStatus: PhoneStatus = 'idle'
  let leaf: MeetingLeafSnapshot | null = null, fix: ((fix: GeoFix) => void) | null = null, withdraw: ((why: 'background' | 'denied' | 'unavailable') => void) | null = null
  const listeners = new Set<() => void>(), scopeListeners = new Set<() => void>(), phoneListeners = new Set<() => void>(), meetingListeners = new Set<() => void>()
  const abort = new AbortController(), calls: { name: string; input: unknown; context: CommandContext }[] = [], opens: MeetingOpenIntent[] = []
  let next: ((name: string, context: CommandContext) => Promise<CommandResult>) | null = null, watches = 0, stops = 0, refreshes = 0, refreshResult = true
  let availability: VenueAvailability = { state: 'ready', reason: null, readAtMs: at, ageMs: 0, staleAtMs: at + 10000, refreshing: false, canRefresh: true }
  let notesRead = true
  const loads: { name: string; input: unknown }[] = []
  function publish(value: CapabilitySnapshot | null) { snapshot = value; for (const listener of listeners) listener() }
  const result = async (name: string, input: unknown, context: CommandContext): Promise<CommandResult> => {
    calls.push({ name, input, context }); if (next) return next(name, context)
    if (snapshot && name === 'beginPhoneSharing') publish({ ...snapshot, sharing: 'active' })
    if (snapshot && name === 'pause') publish({ ...snapshot, sharing: 'paused' })
    if (snapshot && name === 'revoke') publish({ ...snapshot, sharing: 'revoked' })
    return { state: 'applied' }
  }
  const capabilities: VenueCapabilities = { read: () => snapshot, subscribe(listener) { listeners.add(listener); return () => { listeners.delete(listener) } },
    availability: () => availability, async refresh() { refreshes++; if (refreshResult) availability = { ...availability, state: 'ready', reason: null, ageMs: 0 }; for (const listener of listeners) listener(); return refreshResult },
    beginPhoneSharing: ctx => result('beginPhoneSharing', null, ctx), publishPhoneFix: (value, ctx) => result('publishPhoneFix', value, ctx),
    pause: (value, ctx) => result('pause', value, ctx), revoke: ctx => result('revoke', null, ctx), requestLocation: (value, ctx) => result('requestLocation', value, ctx), checkIn: (value, ctx) => result('checkIn', value, ctx),
    postNote: (value, ctx) => result('postNote', value, ctx), reply: (value, ctx) => result('reply', value, ctx), withdraw: (value, ctx) => result('withdraw', value, ctx), resolve: (value, ctx) => result('resolve', value, ctx),
    loadNotes: async id => { loads.push({ name: 'loadNotes', input: id }); return notesRead }, loadNoteThread: async id => { loads.push({ name: 'loadNoteThread', input: id }); return notesRead },
    loadNoteSummary: async () => { loads.push({ name: 'loadNoteSummary', input: null }); return notesRead } }
  const phone: PhoneLocationPort = { read: () => phoneStatus, subscribe(listener) { phoneListeners.add(listener); return () => { phoneListeners.delete(listener) } },
    start(onFix, onWithdraw) { watches++; fix = onFix; withdraw = onWithdraw; phoneStatus = 'watching'; for (const listener of phoneListeners) listener() },
    stop() { stops++; if (phoneStatus === 'watching') { phoneStatus = 'paused'; for (const listener of phoneListeners) listener() } }, dispose() { phone.stop() } }
  const meeting: MeetingConsumerPort = { read: () => leaf, subscribe(listener) { meetingListeners.add(listener); return () => { meetingListeners.delete(listener) } },
    async open(intent) { opens.push(intent); return { kind: 'opened' } }, close() { if (leaf?.locked) return false; leaf = null; for (const listener of meetingListeners) listener(); return true } }
  const authority = { scope: testScope, signal: abort.signal, isCurrent: () => current, subscribe(listener: () => void) { scopeListeners.add(listener); return () => { scopeListeners.delete(listener) } } }
  const controller = createVenueMapController({ venue: testVenue, authority, capabilities, meeting, phone })
  return { controller, capabilities, phone, authority, meeting, calls, opens, loads, publish, get snapshot() { return snapshot! }, get watches() { return watches }, get stops() { return stops }, get refreshes() { return refreshes },
    setAvailability(value: VenueAvailability) { availability = value; for (const listener of listeners) listener() }, setRefreshResult(value: boolean) { refreshResult = value },
    setResult(handler: typeof next) { next = handler }, setNotesRead(value: boolean) { notesRead = value }, setCurrent(value: boolean) { current = value; for (const listener of scopeListeners) listener() },
    emitFix(value: GeoFix) { fix?.(value) }, background() { withdraw?.('background') }, deny() { withdraw?.('denied') }, noDevice() { withdraw?.('unavailable') },
    publishLeaf(value: MeetingLeafSnapshot | null) { leaf = value; for (const listener of meetingListeners) listener() },
    dispose() { controller.dispose(); abort.abort() } }
}
