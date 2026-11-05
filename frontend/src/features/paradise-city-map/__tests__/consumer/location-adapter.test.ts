import { afterEach, expect, it, vi } from 'vitest'
import type { createLocationClient, LandmarkGrant, LocationCommandReceipt, LocationSnapshot, LocationWriteOperation, SharingInput, PhoneFixInput, LocationRequestInput, LocationRequestDecisionInput, CheckInInput } from '../../../../shared/api/paradise-city-v1/location'
import type { CommandContext, ReadPort, VenueAuthority } from '../../ports'
import type { LocationAssociations, LocationPeople, LocationSharingAudience } from '../../model/location-adapter'
import { createVenueLocationAdapter } from '../../model/location-adapter'
import { TransportError } from '../../../../shared/api/errors'
import { aggregateVenueCapabilities, type VenueNotesSource } from '../../../../app/venue-entry/capability-adapters'
import { mainGate, initialSnapshot, testScope, testVenue } from './harness'

const at = 1800000000000, iso = (offset = 0) => new Date(at + offset).toISOString()
function port<T>(initial: T) {
  let value = initial
  const listeners = new Set<() => void>()
  return { read: () => value, subscribe(fn: () => void) { listeners.add(fn); return () => { listeners.delete(fn) } },
    set(next: T) { value = next; for (const listener of listeners) listener() } } satisfies ReadPort<T> & { set(next: T): void }
}
const grant: LandmarkGrant = { event_id: testScope.eventId!, selection: { landmark_id: 'physical-entry', catalog_revision: 7, landmark_revision: 3 },
  provider_id: 'reviewed-provider', source_revision: 'original-source', venue_key: 'physical-venue', map_id: 'physical-map', map_revision: 4,
  policy_revision: 9, label: 'Reviewed entry', review_reference: 'original-review', reviewed_at: iso(-100000), server_time: iso(-1000), valid_until: iso(60000),
  physical_binding: { physical_place_id: 'physical-entry', current_place_revision: 3 }, presentation_id: 'explicit-test-presentation', can_check_in: true, can_meet: true, can_note: true }
function snapshot(): LocationSnapshot {
  return { event_id: testScope.eventId!, actor_id: testScope.userId!, server_time: iso(), access_until: iso(60000),
    policy: { event_id: testScope.eventId!, venue_key: 'physical-venue', provider_id: grant.provider_id, source_revision: grant.source_revision, revision: 9,
      timezone: 'Asia/Seoul', valid_until: iso(60000), fresh_ms: 10000, last_known_ms: 30000, fix_max_age_ms: 5000, future_skew_ms: 100,
      sharing_lease_ms: 60000, request_ttl_ms: 10000, check_in_ttl_ms: 10000, coordinate_retention_ms: 60000, command_retention_ms: 60000, max_accuracy_m: 50, max_viewers: 5 },
    clock: { event_id: testScope.eventId!, venue_key: 'physical-venue', policy_revision: 9, timezone: 'Asia/Seoul', server_time: iso(), end_of_day: iso(100000), valid_until: iso(60000) },
    sharing: { state: 'OFF', revision: 12, viewer_ids: [], expires_at: null, updated_at: iso() }, self_location: null,
    friends: [{ user_id: 'friend-test', source: 'PHONE', position: { latitude: 37, longitude: 126, accuracy_m: 10, captured_at: iso(-1000), received_at: iso(-500) },
      landmark: null, observed_at: iso(-1000), fresh_until: iso(10000), expires_at: iso(30000), access_until: iso(60000), visibility_revision: 8, physical_arrival_verified: false, can_request_location: true }],
    capabilities: { can_share_phone: true, can_pause: true, can_revoke: true, check_in_landmark_ids: ['physical-entry'], geographic_projection: 'UNAVAILABLE' }, incoming_requests: [], outgoing_requests: [] }
}
const owners: ReturnType<typeof createVenueLocationAdapter>[] = []
afterEach(() => { for (const owner of owners.splice(0)) owner.dispose() })
function setup() {
  const scope = testScope, abort = new AbortController(), lease = { scope, venue: testVenue.identity, accessUntilMs: at + 60000 }
  const people = port<LocationPeople | null>({ ...lease, self: { userId: scope.userId!, displayName: 'Self', avatar: null, canMessage: false, canMeet: false }, friends: [{ userId: 'friend-test', displayName: 'Current friend', avatar: null, canMessage: true, canMeet: true }] })
  const associations = port<LocationAssociations | null>({ ...lease, items: [{ landmarkId: mainGate, grant }] })
  const audience = port<LocationSharingAudience | null>({ ...lease, viewerIds: ['friend-test'] })
  const clock = port<{ scope: typeof scope; nowMs: number } | null>({ scope, nowMs: at }), authorityChanges = port(true)
  const authority: VenueAuthority = { scope, signal: abort.signal, isCurrent: authorityChanges.read, subscribe: authorityChanges.subscribe }
  let value = snapshot(), failWrites = false, deferred: Promise<{ data: LocationSnapshot; scope: typeof scope }> | null = null
  const receipt = (operation: LocationWriteOperation, key: string): LocationCommandReceipt => ({ command_key: key, operation, state: 'SUCCEEDED', resource_kind: operation === 'setVenueLocationCheckIn' ? 'CHECK_IN' : operation === 'publishVenueLocationFix' ? 'PHONE_FIX' : operation === 'setVenueLocationSharing' ? 'SHARING' : 'LOCATION_REQUEST', resource_id: 'resource-test', revision: 13, created_at: iso(), expires_at: iso(60000) })
  const write = (operation: LocationWriteOperation, key: string) => failWrites ? Promise.reject(new TransportError('http', { status: 503 })) : Promise.resolve({ data: receipt(operation, key), scope })
  const client = {
    getSnapshot: vi.fn(() => deferred ?? Promise.resolve({ data: value, scope })), listLandmarks: vi.fn(async () => ({ data: { event_id: scope.eventId!, server_time: iso(), access_until: iso(60000), items: [grant] }, scope })),
    getLandmark: vi.fn(async () => ({ data: grant, scope })),
    setSharing: vi.fn((body: SharingInput, key: string) => { if (!failWrites) value = { ...value, sharing: { ...value.sharing, state: body.state, viewer_ids: body.viewer_ids, expires_at: iso(60000) } }; return write('setVenueLocationSharing', key) }),
    publishFix: vi.fn((_body: PhoneFixInput, key: string) => write('publishVenueLocationFix', key)), requestLocation: vi.fn((_body: LocationRequestInput, key: string) => write('requestVenueLocation', key)),
    decideRequest: vi.fn((_id: string, _body: LocationRequestDecisionInput, key: string) => write('decideVenueLocationRequest', key)), checkIn: vi.fn((_body: CheckInInput, key: string) => write('setVenueLocationCheckIn', key)),
    getCommand: vi.fn((operation: LocationWriteOperation, key: string) => Promise.resolve({ data: receipt(operation, key), scope })),
  } satisfies ReturnType<typeof createLocationClient>
  const owner = createVenueLocationAdapter({ authority, venue: testVenue, clock, client, people, associations, sharingAudience: audience, classification: 'CONTROLLED_FIXTURE' }); owners.push(owner)
  const context = (key = 'original-key'): CommandContext => ({ key, scope, venue: testVenue.identity, signal: abort.signal })
  return { owner, client, authority, clock, people, associations, audience, abort, context, authorityChanges,
    setRaw(next: LocationSnapshot) { value = next }, get raw() { return value }, fail() { failWrites = true },
    defer(promise: NonNullable<typeof deferred>) { deferred = promise } }
}
it('constructs and reads without fetching, watching, mutating or losing current MESSAGE access', () => {
  const h = setup(); expect(h.owner.read()?.friends[0]).toMatchObject({ displayName: 'Current friend', canMessage: true, point: null, locationSource: 'unavailable' }); expect(h.client.getSnapshot).not.toHaveBeenCalled(); expect(h.client.setSharing).not.toHaveBeenCalled()
})
it('keeps WGS84 phone positions off the illustrated map', async () => {
  const h = setup(); await h.owner.load(h.abort.signal); expect(h.owner.read()?.friends[0]).toMatchObject({ point: null, landmarkId: null, locationSource: 'phone' })
})
it('projects only the exact reviewed self-reported selection onto its explicit ART anchor', async () => {
  const h = setup(); h.setRaw({ ...h.raw, friends: [{ ...h.raw.friends[0]!, source: 'SELF_REPORTED_CHECK_IN', position: null, landmark: grant }] }); await h.owner.load(h.abort.signal)
  expect(h.owner.read()?.friends[0]?.point).toEqual(testVenue.landmarks[0]!.point); expect(h.owner.read()?.rights.checkIn).toEqual([mainGate])
  h.associations.set({ ...h.associations.read()!, items: [{ landmarkId: mainGate, grant: { ...grant, review_reference: 'different-review' } }] })
  expect(h.owner.read()?.friends[0]?.point).toBeNull()
})
it('drops foreign, expired and ambiguous reviewed mappings', async () => {
  const h = setup(); await h.owner.load(h.abort.signal)
  h.associations.set({ ...h.associations.read()!, items: [{ landmarkId: mainGate, grant }, { landmarkId: mainGate, grant }] }); expect(h.owner.read()?.rights.checkIn).toEqual([])
  h.associations.set({ ...h.associations.read()!, scope: { ...testScope, policyGeneration: 99 }, items: [{ landmarkId: mainGate, grant }] }); expect(h.owner.read()?.rights.checkIn).toEqual([])
})
it('requires explicit current viewer consent and never defaults to all friends', async () => {
  const h = setup(); await h.owner.load(h.abort.signal); h.audience.set(null)
  expect(await h.owner.beginPhoneSharing(h.context())).toEqual({ state: 'rejected' }); expect(h.client.setSharing).not.toHaveBeenCalled()
  h.audience.set({ scope: testScope, venue: testVenue.identity, accessUntilMs: at + 60000, viewerIds: ['foreign-person'] })
  await h.owner.beginPhoneSharing(h.context()); expect(h.client.setSharing).not.toHaveBeenCalled()
})
it('forwards exact selected viewers, expected sharing revision and original key once', async () => {
  const h = setup(); await h.owner.load(h.abort.signal); expect(await h.owner.beginPhoneSharing(h.context())).toEqual({ state: 'applied' })
  expect(h.client.setSharing).toHaveBeenCalledExactlyOnceWith({ expected_revision: 12, state: 'ACTIVE', viewer_ids: ['friend-test'] }, 'original-key')
  await h.owner.beginPhoneSharing(h.context()); expect(h.client.setSharing).toHaveBeenCalledTimes(1)
})
it('recovers an unknown original operation by status only without replay', async () => {
  const h = setup(); await h.owner.load(h.abort.signal); h.fail(); expect(await h.owner.checkIn(mainGate, h.context())).toEqual({ state: 'unknown' })
  expect(h.owner.ownsCommand('original-key')).toBe(true); expect(await h.owner.resolve('original-key', h.context())).toEqual({ state: 'applied' })
  expect(h.client.getCommand).toHaveBeenCalledExactlyOnceWith('setVenueLocationCheckIn', 'original-key'); expect(h.client.checkIn).toHaveBeenCalledTimes(1)
  expect(await h.owner.resolve('foreign-key', h.context('foreign-key'))).toEqual({ state: 'rejected' }); expect(h.client.getCommand).toHaveBeenCalledTimes(1)
})
it('uses numeric provider selection, never an ART ID or coerced meeting string', async () => {
  const h = setup(); await h.owner.load(h.abort.signal); await h.owner.checkIn(mainGate, h.context())
  expect(h.client.checkIn).toHaveBeenCalledExactlyOnceWith({ expected_sharing_revision: 12, landmark: grant.selection }, 'original-key')
})
it('withdraws raw positions across roster changes and drops late reads after abort', async () => {
  const h = setup(); await h.owner.load(h.abort.signal); h.people.set({ ...h.people.read()!, friends: [] }); expect(h.owner.read()?.friends).toEqual([])
  let finish!: (value: { data: LocationSnapshot; scope: typeof testScope }) => void
  h.defer(new Promise(resolve => { finish = resolve })); const loading = h.owner.load(h.abort.signal); h.abort.abort(); finish({ data: snapshot(), scope: testScope })
  expect(await loading).toBe(false); expect(h.owner.read()).toBeNull()
})
it('enforces sharing state, server freshness and accuracy before forwarding phone fixes', async () => {
  const h = setup(); await h.owner.load(h.abort.signal); const fix = { latitude: 37, longitude: 126, accuracy: 10, capturedAtMs: at }
  await h.owner.publishPhoneFix(fix, h.context()); expect(h.client.publishFix).not.toHaveBeenCalled()
  await h.owner.beginPhoneSharing(h.context('share')); await h.owner.publishPhoneFix({ ...fix, accuracy: 100 }, h.context('bad')); await h.owner.publishPhoneFix({ ...fix, capturedAtMs: at - 10000 }, h.context('stale'))
  expect(h.client.publishFix).not.toHaveBeenCalled(); await h.owner.publishPhoneFix(fix, h.context('fix')); expect(h.client.publishFix).toHaveBeenCalledExactlyOnceWith({ expected_sharing_revision: 12, source: 'DEVICE_GEOLOCATION_REPORTED', latitude: 37, longitude: 126, accuracy_m: 10, captured_at: iso() }, 'fix')
})
it('decides only current pending incoming/outgoing requests with the authoritative revision', async () => {
  const h = setup(), incoming = { request_id: 'request-test', event_id: testScope.eventId!, requester_id: 'friend-test', subject_id: testScope.userId!, revision: 4, state: 'PENDING' as const, created_at: iso(-100), expires_at: iso(10000) }
  h.setRaw({ ...h.raw, incoming_requests: [incoming] }); await h.owner.load(h.abort.signal)
  expect(h.owner.requests().incoming).toHaveLength(1); await h.owner.decideRequest(incoming.request_id, 'CANCEL', h.context('bad')); expect(h.client.decideRequest).not.toHaveBeenCalled()
  await h.owner.decideRequest(incoming.request_id, 'APPROVE', h.context()); expect(h.client.decideRequest).toHaveBeenCalledExactlyOnceWith('request-test', { expected_revision: 4, decision: 'APPROVE' }, 'original-key')
  h.clock.set({ scope: testScope, nowMs: at + 10001 }); expect(h.owner.requests().incoming).toEqual([])
})
function notesSource() {
  const base = initialSnapshot(), state = port<VenueNotesSource extends ReadPort<infer T> ? T : never>({ ...base, rights: base.rights })
  const load = vi.fn(async () => true), loadThread = vi.fn(async () => true)
  const source: VenueNotesSource = { ...state, load, loadThread, postNote: vi.fn(async () => ({ state: 'applied' as const })), reply: vi.fn(async () => ({ state: 'applied' as const })), resolve: vi.fn(async () => ({ state: 'applied' as const })), ownsCommand: vi.fn((key: string) => key === 'note-key') }
  return { source, state, load, loadThread }
}
it('aggregates Notes independently when Location is absent and vice versa', () => {
  const h = setup(), n = notesSource(), onlyN = aggregateVenueCapabilities({ authority: h.authority, venue: testVenue, clock: h.clock, location: null, notes: n.source })
  expect(onlyN.read()?.notes).toHaveLength(1); expect(onlyN.read()?.rights.sharePhone).toBe(false)
  const onlyL = aggregateVenueCapabilities({ authority: h.authority, venue: testVenue, clock: h.clock, location: h.owner, notes: null })
  expect(onlyL.read()?.friends).toHaveLength(1); expect(onlyL.read()?.rights.postNote).toEqual([])
})
it('routes status to exactly the captured L/N owner and rejects ambiguous ownership', async () => {
  const h = setup(), n = notesSource(), aggregate = aggregateVenueCapabilities({ authority: h.authority, venue: testVenue, clock: h.clock, location: h.owner, notes: n.source })
  expect(await aggregate.resolve('unknown-key', h.context('unknown-key'))).toEqual({ state: 'rejected' }); expect(n.source.resolve).not.toHaveBeenCalled()
  await aggregate.resolve('note-key', h.context('note-key')); expect(n.source.resolve).toHaveBeenCalledTimes(1)
  await h.owner.load(h.abort.signal); h.fail(); await h.owner.checkIn(mainGate, h.context('note-key'))
  expect(await aggregate.resolve('note-key', h.context('note-key'))).toEqual({ state: 'rejected' }); expect(h.client.getCommand).not.toHaveBeenCalled(); expect(n.source.resolve).toHaveBeenCalledTimes(1)
})
it('rejects foreign Notes leases, expired clocks and wrong command scope without dispatch', async () => {
  const h = setup(), n = notesSource(), aggregate = aggregateVenueCapabilities({ authority: h.authority, venue: testVenue, clock: h.clock, location: null, notes: n.source })
  n.state.set({ ...n.state.read()!, scope: { ...testScope, policyGeneration: 2 } }); expect(aggregate.read()).toBeNull()
  await aggregate.postNote({ landmarkId: mainGate, body: 'Synthetic', audience: 'FRIENDS', expiry: 'ONE_HOUR' }, { ...h.context(), scope: { ...testScope, generation: 99 } }); expect(n.source.postNote).not.toHaveBeenCalled()
  h.clock.set(null); expect(aggregate.read()).toBeNull()
})
it('forwards explicit Notes reads with the original signal and no command dispatch or read grant inferred from write rights', async () => {
  const h = setup(), n = notesSource(), aggregate = aggregateVenueCapabilities({ authority: h.authority, venue: testVenue, clock: h.clock, location: null, notes: n.source })
  expect(n.load).not.toHaveBeenCalled(); await aggregate.loadNotes!(mainGate, h.context()); expect(n.load).toHaveBeenCalledExactlyOnceWith(mainGate, h.abort.signal)
  await aggregate.loadNoteThread!('note-test', h.context()); expect(n.loadThread).toHaveBeenCalledExactlyOnceWith('note-test', h.abort.signal)
  await aggregate.loadNotes!(mainGate, { ...h.context(), scope: { ...testScope, policyGeneration: 99 } }); expect(n.load).toHaveBeenCalledTimes(1); expect(n.source.postNote).not.toHaveBeenCalled()
})
it('cannot reuse a sharing command key for Pause after a different sharing intent', async () => {
  const h = setup(); await h.owner.load(h.abort.signal); await h.owner.beginPhoneSharing(h.context())
  expect(await h.owner.pause('user', h.context())).toEqual({ state: 'rejected' }); expect(h.client.setSharing).toHaveBeenCalledTimes(1)
})
