// @vitest-environment jsdom
import { webcrypto } from 'node:crypto'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import type { CommunityBootstrapBinding } from '../../community-adoption/bootstrap'
import type { MeetingClockPort } from '../../meeting-entry/ports'
import type { HttpRequest, HttpResult } from '../../../shared/api/http'
import type { LocationSnapshot } from '../../../shared/api/paradise-city-v1/location'
import type { LocationPeople } from '../../../features/paradise-city-map/model/location-adapter'
import type { LandmarkId } from '../../../features/paradise-city-map/spatial/contract'
import type { ReadPort, VenueView } from '../../../features/paradise-city-map/ports'
import { createSessionController } from '../../../features/auth/model/session'
import { checkedBootstrap, eventId, userId } from '../../community-authority/__tests__/fixtures'
import { createVenueProduction, createVenueSources, type VenueSources } from '../../venue-root/production'

const friendId = '00000000-0000-4000-8000-000000000102'
const at = 1_800_000_000_000
const iso = (offset = 0) => new Date(at + offset).toISOString()
const identity = { venueId: 'paradise-city', siteplanId: 'paradise-city-siteplan', revision: 1, variantId: 'photo-5-provisional' }
const venue: VenueView = {
  identity, title: 'Paradise City', classification: 'ILLUSTRATED_UNCALIBRATED',
  image: { src: 'blob:fixture-siteplan', width: 1024, height: 1280, description: 'Fixture siteplan' },
  landmarks: [{ id: 'main-gate' as LandmarkId, label: 'Main Gate', point: { x: 100, y: 200 }, meetingContext: null }],
}
const grant = { event_id: eventId, selection: { landmark_id: 'real-main-gate', catalog_revision: 3, landmark_revision: 4 },
  provider_id: 'reviewed-provider', source_revision: 'source-1', venue_key: 'venue-key', map_id: 'map-1', map_revision: 2,
  policy_revision: 9, label: 'Reviewed Main Gate', review_reference: 'review-1', reviewed_at: iso(-120000), server_time: iso(-1000),
  valid_until: iso(60000), physical_binding: null, presentation_id: 'main-gate', can_check_in: true, can_meet: true, can_note: true }
const snapshot = (): LocationSnapshot => ({
  event_id: eventId, actor_id: userId, server_time: iso(-1000), access_until: iso(60000),
  policy: { event_id: eventId, venue_key: 'venue-key', provider_id: grant.provider_id, source_revision: grant.source_revision, revision: 9,
    timezone: 'Asia/Seoul', valid_until: iso(60000), fresh_ms: 10000, last_known_ms: 30000, fix_max_age_ms: 5000, future_skew_ms: 100,
    sharing_lease_ms: 60000, request_ttl_ms: 10000, check_in_ttl_ms: 10000, coordinate_retention_ms: 60000, command_retention_ms: 60000,
    max_accuracy_m: 50, max_viewers: 5 },
  clock: { event_id: eventId, venue_key: 'venue-key', policy_revision: 9, timezone: 'Asia/Seoul', server_time: iso(-1000), end_of_day: iso(100000), valid_until: iso(60000) },
  sharing: { state: 'OFF', revision: 12, viewer_ids: [], expires_at: null, updated_at: iso(-1000) }, self_location: null,
  friends: [{ user_id: friendId, source: 'PHONE', position: { latitude: 37, longitude: 126, accuracy_m: 10, captured_at: iso(-1000), received_at: iso(-500) },
    landmark: null, observed_at: iso(-1000), fresh_until: iso(10000), expires_at: iso(30000), access_until: iso(60000), visibility_revision: 8,
    physical_arrival_verified: false, can_request_location: true }],
  capabilities: { can_share_phone: true, can_pause: true, can_revoke: true, check_in_landmark_ids: ['real-main-gate'], geographic_projection: 'UNAVAILABLE' },
  incoming_requests: [], outgoing_requests: [],
})
const landmarkList = () => ({ event_id: eventId, server_time: iso(-1000), access_until: iso(60000), items: [grant] })

const stops: (() => void)[] = []
beforeEach(() => { vi.stubGlobal('crypto', webcrypto) })
afterEach(() => { for (const stop of stops.splice(0).reverse()) stop(); vi.restoreAllMocks(); vi.unstubAllGlobals() })

async function fixture(options: { omitPeople?: boolean } = {}) {
  const bootstrap = await checkedBootstrap()
  const participant = (bootstrap.document as { participant?: { profile?: { avatar?: unknown } } } | undefined)?.participant
  const auth = createSessionController({ eventId, fetch: async () => { throw Error('No live transport') },
    cookiePolicy: { available: true, onInvalidate: () => () => {}, run: (_kind, work) => work(), dispose() {} } })
  auth.session.replace(auth.session.read().generation, { userId, eventId, accessToken: 'TEST_ONLY_MEMORY' })
  const scope = auth.privacy.capture(), lifetime = new AbortController()
  const clock: MeetingClockPort = { read: () => ({ scope, nowMs: at }), subscribe: () => () => {} }
  const community = { source: bootstrap, scope, signal: lifetime.signal, isCurrent: () => true,
    read: () => ({ clock }), dispose() {} } as unknown as CommunityBootstrapBinding
  const calls: string[] = []
  const http = { async request<T>(input: HttpRequest<T>): Promise<HttpResult<T>> {
    calls.push(input.path)
    const value = input.path.endsWith('/venue/location') ? snapshot()
      : input.path.endsWith('/venue/landmarks') ? landmarkList()
        : input.path.includes('/people/') ? { user_id: input.path.split('/').at(-1), display_name: 'Fixture friend', bio: null, avatar: participant?.profile?.avatar }
          : (() => { throw Error(`Unexpected request ${input.path}`) })()
    return { data: input.decode(value, { scope, status: 200, path: input.path }), scope }
  } }
  const directory: ReadPort<LocationPeople | null> = { read: () => ({ scope, venue: identity, accessUntilMs: at + 60000,
    self: { userId, displayName: 'Fixture fan', avatar: null, canMessage: true, canMeet: true },
    friends: [{ userId: friendId, displayName: 'Fixture friend', avatar: null, canMessage: true, canMeet: true }] }), subscribe: () => () => {} }
  const sources = createVenueSources({ auth, scope, signal: lifetime.signal, identity, community, http, participant,
    ...(options.omitPeople ? {} : { people: directory }) })
  stops.push(() => { lifetime.abort(); sources.dispose(); auth.dispose() })
  return { bootstrap, auth, scope, lifetime, community, sources, calls }
}

function production(sources: VenueSources) {
  return createVenueProduction({ venue: { ...venue, landmarks: venue.landmarks.map(landmark =>
    ({ ...landmark, meetingContext: sources.meetingPlaces.get(landmark.id) ?? null })) }, sources })
}

it('projects real Location and reviewed-landmark responses through the production ports', async () => {
  const f = await fixture()
  expect(await f.sources.loadLandmarks(f.lifetime.signal)).toBe(true)
  expect(f.sources.meetingPlaces.get('main-gate' as LandmarkId)).toEqual({ landmarkId: 'real-main-gate', catalogRevision: '3', landmarkRevision: '4' })
  const ports = production(f.sources)
  stops.push(() => ports.dispose())
  expect(await ports.start(f.lifetime.signal)).toBe(true)
  const state = ports.capabilities.read()
  expect(state?.friends.map(person => person.userId)).toEqual([friendId])
  expect(state?.friends[0]).toMatchObject({ displayName: 'Fixture friend', locationSource: 'phone', visible: true })
  expect(state?.rights).toMatchObject({ sharePhone: true, checkIn: ['main-gate'], postNote: [] })
  expect(ports.phone.read()).toBe('idle')
  expect(ports.meeting).not.toBeNull()
})

it('names friends only from the actor snapshot and an authorized public profile', async () => {
  const f = await fixture({ omitPeople: true })
  const ports = production(f.sources)
  stops.push(() => ports.dispose())
  expect(await ports.start(f.lifetime.signal)).toBe(true)
  await vi.waitFor(() => expect(ports.capabilities.read()?.friends[0]?.displayName).toBe('Fixture friend'))
  expect(ports.capabilities.read()?.friends[0]?.userId).toBe(friendId)
})

it('fails closed when no reviewed friend identity source is available', async () => {
  const f = await fixture()
  const empty: ReadPort<LocationPeople | null> = { read: () => null, subscribe: () => () => {} }
  const sources = createVenueSources({ auth: f.auth, scope: f.scope, signal: f.lifetime.signal, identity, community: f.community, http: f.sources.http, people: empty, participant: null })
  stops.push(() => sources.dispose())
  expect(await sources.loadLandmarks(f.lifetime.signal)).toBe(true)
  const ports = createVenueProduction({ venue, sources })
  stops.push(() => ports.dispose())
  expect(await ports.start(f.lifetime.signal)).toBe(true)
  expect(ports.capabilities.read()).toBeNull()
  expect(ports.phone.read()).toBe('idle')
})

it('fails closed before any request when the qualified clock is unavailable', async () => {
  const f = await fixture()
  const sources = createVenueSources({ auth: f.auth, scope: f.scope, signal: f.lifetime.signal, identity, community: null, http: f.sources.http, participant: null })
  stops.push(() => sources.dispose())
  expect(await sources.loadLandmarks(f.lifetime.signal)).toBe(false)
  const ports = createVenueProduction({ venue, sources })
  stops.push(() => ports.dispose())
  expect(await ports.start(f.lifetime.signal)).toBe(false)
  expect(ports.capabilities.read()).toBeNull()
})
