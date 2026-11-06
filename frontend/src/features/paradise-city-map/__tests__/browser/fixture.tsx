import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { createMemoryRouter, Outlet, RouterProvider } from 'react-router-dom'
import { VenueShellFeatures, type VenueShellSources } from '../../../../app/WorldShell'
import { ParadiseCityMapPanel } from '../../../../app/venue-entry/ParadiseCityMapPanel'
import type { CommunityBootstrapBinding } from '../../../../app/community-adoption/bootstrap'
import type { MeetingClockPort } from '../../../../app/meeting-entry/ports'
import type { HttpRequest, HttpResult } from '../../../../shared/api/http'
import type { LocationSnapshot, LocationCommandReceipt, SharingInput } from '../../../../shared/api/paradise-city-v1/location'
import { LocationUnavailableError } from '../../../../shared/api/paradise-city-v1/location'
import type { LocationPeople, LocationSharingAudience } from '../../model/location-adapter'
import type { ReadPort } from '../../ports'
import { createSessionController } from '../../../../features/auth/model/session'
import bootstrap from '../../../../../../contracts/fixtures/core/bootstrap-open.json'

type Mode = 'cluster' | 'denied' | 'paused' | 'revoked' | 'stale' | 'unavailable'
const eventId = '00000000-0000-4000-8000-000000000201'
const userId = '00000000-0000-4000-8000-000000000101'
const friendId = '00000000-0000-4000-8000-000000000102'
const friend2Id = '00000000-0000-4000-8000-000000000103'
const friend3Id = '00000000-0000-4000-8000-000000000104'
const at = 1_800_000_000_000
const iso = (offset = 0) => new Date(at + offset).toISOString()
const identity = { venueId: 'paradise-city', siteplanId: 'paradise-city-siteplan', revision: 1, variantId: 'photo-5-provisional' }
const grant = { event_id: eventId, selection: { landmark_id: 'real-main-gate', catalog_revision: 3, landmark_revision: 4 },
  provider_id: 'reviewed-provider', source_revision: 'source-1', venue_key: 'venue-key', map_id: 'map-1', map_revision: 2,
  policy_revision: 9, label: 'Reviewed Main Gate', review_reference: 'review-1', reviewed_at: iso(-120000), server_time: iso(-1000),
  valid_until: iso(60000), physical_binding: null, presentation_id: 'main-gate', can_check_in: true, can_meet: true, can_note: true }
const query = new URLSearchParams(location.search)
const mode = (query.get('mode') ?? 'cluster') as Mode

function visible(user: string, source: 'PHONE' | 'SELF_REPORTED_CHECK_IN' | 'LAST_KNOWN' | 'UNAVAILABLE', checkIn = false) {
  return { user_id: user, source,
    position: source === 'PHONE' ? { latitude: 37, longitude: 126, accuracy_m: 10, captured_at: iso(-1000), received_at: iso(-500) } : null,
    landmark: checkIn ? { ...grant } : null, observed_at: iso(-1000), fresh_until: source === 'PHONE' ? iso(10000) : null,
    expires_at: iso(30000), access_until: iso(60000), visibility_revision: 8, physical_arrival_verified: false as const, can_request_location: true }
}
let sharing: 'OFF' | 'ACTIVE' | 'PAUSED' | 'REVOKED' = mode === 'paused' || mode === 'revoked' ? 'ACTIVE' : 'OFF'
let viewerIds: string[] = sharing === 'ACTIVE' ? [friendId] : []
let selfLocation: ReturnType<typeof visible> | null = sharing === 'ACTIVE' ? visible(userId, 'SELF_REPORTED_CHECK_IN', true) : null
let revision = 12
const friends = mode === 'cluster' || mode === 'stale'
  ? [visible(friendId, 'SELF_REPORTED_CHECK_IN', true), visible(friend2Id, 'SELF_REPORTED_CHECK_IN', true), visible(friend3Id, 'SELF_REPORTED_CHECK_IN', true)]
  : [visible(friendId, 'PHONE')]
function snapshot(): LocationSnapshot {
  return {
    event_id: eventId, actor_id: userId, server_time: iso(-1000), access_until: iso(60000),
    policy: { event_id: eventId, venue_key: 'venue-key', provider_id: grant.provider_id, source_revision: grant.source_revision, revision: 9,
      timezone: 'Asia/Seoul', valid_until: iso(60000), fresh_ms: 10000, last_known_ms: 30000, fix_max_age_ms: 5000, future_skew_ms: 100,
      sharing_lease_ms: 60000, request_ttl_ms: 10000, check_in_ttl_ms: 10000, coordinate_retention_ms: 60000, command_retention_ms: 60000,
      max_accuracy_m: 50, max_viewers: 5 },
    clock: { event_id: eventId, venue_key: 'venue-key', policy_revision: 9, timezone: 'Asia/Seoul', server_time: iso(-1000), end_of_day: iso(100000), valid_until: iso(60000) },
    sharing: { state: sharing, revision, viewer_ids: [...viewerIds], expires_at: sharing === 'ACTIVE' ? iso(60000) : null, updated_at: iso(-1000) },
    self_location: selfLocation, friends: friends.filter(friend => friend.user_id !== userId),
    capabilities: { can_share_phone: true, can_pause: true, can_revoke: true, check_in_landmark_ids: ['real-main-gate'], geographic_projection: 'UNAVAILABLE' },
    incoming_requests: [], outgoing_requests: [],
  }
}
const receipt = (operation: LocationCommandReceipt['operation'], key: string): LocationCommandReceipt => ({
  command_key: key, operation, state: 'SUCCEEDED', resource_kind: 'SHARING', resource_id: '00000000-0000-4000-8000-000000000201', revision: ++revision, created_at: iso(), expires_at: iso(60000) })
const http = { async request<T>(input: HttpRequest<T>): Promise<HttpResult<T>> {
  if (input.path.endsWith('/venue/location') && mode === 'unavailable') throw new LocationUnavailableError()
  let value: unknown
  if (input.path.endsWith('/venue/location')) value = snapshot()
  else if (input.path.endsWith('/venue/landmarks')) value = { event_id: eventId, server_time: iso(-1000), access_until: iso(60000), items: [grant] }
  else if (input.path.endsWith('/venue/location/sharing')) {
    const body = (input.body as unknown as { json: SharingInput }).json
    sharing = body.state; viewerIds = [...body.viewer_ids]
    selfLocation = sharing === 'ACTIVE' ? visible(userId, 'SELF_REPORTED_CHECK_IN', true) : null
    value = receipt('setVenueLocationSharing', String(input.idempotencyKey))
  } else if (input.path.includes('/people/')) {
    const id = input.path.split('/').at(-1)
    value = { user_id: id, display_name: 'Fixture friend', bio: null, avatar: bootstrap.participant.profile.avatar }
  } else throw Error(`Unexpected request ${input.path}`)
  return { data: input.decode(value, { scope, status: 200, path: input.path }), scope }
} }

const auth = createSessionController({ eventId, fetch: async () => { throw Error('No live transport') },
  cookiePolicy: { available: true, onInvalidate: () => () => {}, run: (_kind, work) => work(), dispose() {} } })
auth.session.replace(auth.session.read().generation, { userId, eventId, accessToken: 'TEST_ONLY_MEMORY' })
const scope = auth.privacy.capture(), lifetime = new AbortController()
const clockListeners = new Set<() => void>()
let nowMs = at
const clock: MeetingClockPort = { read: () => ({ scope, nowMs }), subscribe(listener: () => void) { clockListeners.add(listener); return () => { clockListeners.delete(listener) } } }
window.__VENUE_CLOCK__ = { advance(ms: number) { nowMs += ms; for (const listener of clockListeners) listener() } }
const community = { source: bootstrap, scope, signal: lifetime.signal, isCurrent: () => true,
  read: () => ({ clock }), dispose() {} } as unknown as CommunityBootstrapBinding
const identities = [
  { userId: friendId, displayName: 'Fixture friend' },
  { userId: friend2Id, displayName: 'Mina Park' },
  { userId: friend3Id, displayName: 'Jae Kim' },
]
const people: ReadPort<LocationPeople | null> = { read: () => ({ scope, venue: identity, accessUntilMs: nowMs + 600000,
  self: { userId, displayName: 'Fixture fan', avatar: null, canMessage: true, canMeet: true },
  friends: identities.map(person => ({ ...person, avatar: null, canMessage: true, canMeet: true })) }), subscribe: () => () => {} }
const sharingAudience: ReadPort<LocationSharingAudience | null> = { read: () => ({ scope, venue: identity, accessUntilMs: nowMs + 600000, viewerIds: [friendId] }), subscribe: () => () => {} }
const sources: VenueShellSources = { http, people, sharingAudience }

const portrait = (person: { readonly displayName: string }, size: 64 | 128) =>
  <span aria-label={`${person.displayName} fixture portrait`} style={{ display: 'flex', alignItems: 'center', justifyContent: 'center',
    width: '100%', height: '100%', borderRadius: '50%', background: '#d8d1c4', color: '#171717', fontWeight: 700,
    fontSize: size === 128 ? 40 : 20, lineHeight: 1 }}>{person.displayName.slice(0, 1)}</span>

function Layout() {
  return <VenueShellFeatures auth={auth} scope={scope} community={community} participant={bootstrap.participant} sources={sources}><Outlet /></VenueShellFeatures>
}
const router = createMemoryRouter([{ element: <Layout />, children: [{ path: '*', element: <ParadiseCityMapPanel renderPortrait={portrait} /> }] }], { initialEntries: ['/world/map'] })
declare global {
  interface Window {
    __VENUE_CLOCK__: { advance(ms: number): void }
    __VENUE_MODE__: Mode
  }
}
window.__VENUE_MODE__ = mode
createRoot(document.getElementById('root')!).render(<StrictMode><RouterProvider router={router} /></StrictMode>)
