import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { createMemoryRouter, Outlet, RouterProvider } from 'react-router-dom'
import { VenueShellFeatures } from '../../../WorldShell'
import { ParadiseCityMapPanel } from '../../../venue-entry/ParadiseCityMapPanel'
import type { VenueShellSources } from '../../../WorldShell'
import type { CommunityBootstrapBinding } from '../../../community-adoption/bootstrap'
import type { MeetingClockPort } from '../../../meeting-entry/ports'
import type { HttpRequest, HttpResult } from '../../../../shared/api/http'
import type { LocationSnapshot } from '../../../../shared/api/paradise-city-v1/location'
import type { LocationPeople } from '../../../../features/paradise-city-map/model/location-adapter'
import type { ReadPort } from '../../../../features/paradise-city-map/ports'
import { createSessionController } from '../../../../features/auth/model/session'
import bootstrap from '../../../../../../contracts/fixtures/core/bootstrap-open.json'

const eventId = '00000000-0000-4000-8000-000000000201'
const userId = '00000000-0000-4000-8000-000000000101'
const friendId = '00000000-0000-4000-8000-000000000102'
const at = 1_800_000_000_000
const iso = (offset = 0) => new Date(at + offset).toISOString()
const identity = { venueId: 'paradise-city', siteplanId: 'paradise-city-siteplan', revision: 1, variantId: 'photo-5-provisional' }
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
const mode = new URLSearchParams(location.search).get('mode') === 'removed' ? 'removed' : 'real'

const auth = createSessionController({ eventId, fetch: async () => { throw Error('No live transport') },
  cookiePolicy: { available: true, onInvalidate: () => () => {}, run: (_kind, work) => work(), dispose() {} } })
auth.session.replace(auth.session.read().generation, { userId, eventId, accessToken: 'TEST_ONLY_MEMORY' })
const scope = auth.privacy.capture(), lifetime = new AbortController()
const clock: MeetingClockPort = { read: () => ({ scope, nowMs: at }), subscribe: () => () => {} }
const community = { source: bootstrap, scope, signal: lifetime.signal, isCurrent: () => true,
  read: () => ({ clock }), dispose() {} } as unknown as CommunityBootstrapBinding
const http = { async request<T>(input: HttpRequest<T>): Promise<HttpResult<T>> {
  const value = input.path.endsWith('/venue/location') ? snapshot()
    : input.path.endsWith('/venue/landmarks') ? landmarkList()
      : { user_id: input.path.split('/').at(-1), display_name: 'Fixture friend', bio: null, avatar: bootstrap.participant.profile.avatar }
  return { data: input.decode(value, { scope, status: 200, path: input.path }), scope }
} }
const people: ReadPort<LocationPeople | null> = { read: () => ({ scope, venue: identity, accessUntilMs: at + 60000,
  self: { userId, displayName: 'Fixture fan', avatar: null, canMessage: true, canMeet: true },
  friends: [{ userId: friendId, displayName: 'Fixture friend', avatar: null, canMessage: true, canMeet: true }] }), subscribe: () => () => {} }
const sources: VenueShellSources = { http, ...(mode === 'real' ? { people } : {}) }

function Layout() {
  return <VenueShellFeatures auth={auth} scope={scope} community={community}
    participant={mode === 'real' ? bootstrap.participant : null} sources={sources}><Outlet /></VenueShellFeatures>
}
const router = createMemoryRouter([{ element: <Layout />, children: [{ path: '*', element: <ParadiseCityMapPanel /> }] }], { initialEntries: ['/world/map'] })
declare global { interface Window { __VENUE_SHELL_PORTS__: { mode: string } } }
window.__VENUE_SHELL_PORTS__ = { mode }
createRoot(document.getElementById('root')!).render(<StrictMode><RouterProvider router={router} /></StrictMode>)
