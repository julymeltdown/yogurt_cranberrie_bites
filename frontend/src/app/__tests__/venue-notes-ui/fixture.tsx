import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { createMemoryRouter, Outlet, RouterProvider } from 'react-router-dom'
import { VenueShellFeatures } from '../../WorldShell'
import { ParadiseCityMapPanel } from '../../venue-entry/ParadiseCityMapPanel'
import type { VenueShellSources } from '../../WorldShell'
import type { CommunityBootstrapBinding } from '../../community-adoption/bootstrap'
import type { MeetingClockPort } from '../../meeting-entry/ports'
import type { HttpRequest, HttpResult } from '../../../shared/api/http'
import type { LocationSnapshot } from '../../../shared/api/paradise-city-v1/location'
import type { LocationPeople } from '../../../features/paradise-city-map/model/location-adapter'
import type { ReadPort } from '../../../features/paradise-city-map/ports'
import { createSessionController } from '../../../features/auth/model/session'
import bootstrap from '../../../../../contracts/fixtures/core/bootstrap-open.json'

const eventId = '00000000-0000-4000-8000-000000000201'
const userId = '00000000-0000-4000-8000-000000000101'
const friendId = '00000000-0000-4000-8000-000000000102'
const at = 1_800_000_000_000
const iso = (offset = 0) => new Date(at + offset).toISOString()
const identity = { venueId: 'paradise-city', siteplanId: 'paradise-city-siteplan', revision: 1, variantId: 'photo-5-provisional' }
const mainGrant = { event_id: eventId, selection: { landmark_id: 'real-main-gate', catalog_revision: 3, landmark_revision: 4 },
  provider_id: 'reviewed-provider', source_revision: 'source-1', venue_key: 'venue-key', map_id: 'map-1', map_revision: 2,
  policy_revision: 9, label: 'Main Gate', review_reference: 'review-1', reviewed_at: iso(-120000), server_time: iso(-1000),
  valid_until: iso(600000), physical_binding: null, presentation_id: 'main-gate', can_check_in: true, can_meet: true, can_note: true }
const subGrant = { ...mainGrant, selection: { landmark_id: 'real-sub-gate', catalog_revision: 3, landmark_revision: 5 },
  label: 'Sub Gate', presentation_id: 'sub-gate' }
const grants = [mainGrant, subGrant]
const avatar = bootstrap.participant.profile.avatar
const profile = (id: string, name: string) => ({ user_id: id, display_name: name, bio: null, avatar })

interface Stored {
  id: string; physical: string; art: string; revision: number; authorId: string; authorName: string
  body: string; createdAt: number; expiresAt: number; audience: 'FRIENDS' | 'EVERYONE'
  expiry: 'ONE_HOUR' | 'VENUE_END_DAY'; parentId: string | null; withdrawn?: boolean
}
const noteId = (suffix: string) => `00000000-0000-4000-8000-0000000003${suffix}`
let clockMs = at
const clockListeners = new Set<() => void>()
const store: Stored[] = [
  { id: noteId('01'), physical: 'real-main-gate', art: 'main-gate', revision: 4, authorId: friendId, authorName: 'Fixture friend',
    body: 'See you by the main gate after the show.', createdAt: at - 300_000, expiresAt: at + 3_600_000,
    audience: 'FRIENDS', expiry: 'ONE_HOUR', parentId: null },
  { id: noteId('02'), physical: 'real-main-gate', art: 'main-gate', revision: 1, authorId: userId, authorName: 'Fixture fan',
    body: 'This notice expires shortly.', createdAt: at - 2_500_000, expiresAt: at + 3_600_000,
    audience: 'EVERYONE', expiry: 'ONE_HOUR', parentId: null },
  { id: noteId('11'), physical: 'real-main-gate', art: 'main-gate', revision: 1, authorId: friendId, authorName: 'Fixture friend',
    body: 'Bring the light sticks.', createdAt: at - 200_000, expiresAt: at + 3_600_000,
    audience: 'FRIENDS', expiry: 'ONE_HOUR', parentId: noteId('01') },
  { id: noteId('21'), physical: 'real-sub-gate', art: 'sub-gate', revision: 2, authorId: friendId, authorName: 'Fixture friend',
    body: 'Meet at the sub gate if the main gate is full.', createdAt: at - 400_000, expiresAt: at + 3_600_000,
    audience: 'EVERYONE', expiry: 'ONE_HOUR', parentId: null },
  { id: noteId('22'), physical: 'real-sub-gate', art: 'sub-gate', revision: 3, authorId: userId, authorName: 'Fixture fan',
    body: 'Queue forms from 17:00 near the sub gate.', createdAt: at - 600_000, expiresAt: at + 3_600_000,
    audience: 'FRIENDS', expiry: 'ONE_HOUR', parentId: null },
]
const stateParam = new URLSearchParams(location.search).get('state')
const mode = stateParam === 'denied' ? 'denied' : stateParam === 'expired' ? 'expired' : 'normal'
if (mode === 'expired') store[1]!.expiresAt = at + 2_500

const auth = createSessionController({ eventId, fetch: async () => { throw Error('No live transport') },
  cookiePolicy: { available: true, onInvalidate: () => () => {}, run: (_kind, work) => work(), dispose() {} } })
auth.session.replace(auth.session.read().generation, { userId, eventId, accessToken: 'TEST_ONLY_MEMORY' })
const scope = auth.privacy.capture(), lifetime = new AbortController()
const clock: MeetingClockPort = {
  read: () => ({ scope, nowMs: clockMs }),
  subscribe(listener) { clockListeners.add(listener); return () => { clockListeners.delete(listener) } },
}
const community = { source: bootstrap, scope, signal: lifetime.signal, isCurrent: () => true,
  read: () => ({ clock }), dispose() {} } as unknown as CommunityBootstrapBinding
const snapshot = (): LocationSnapshot => ({
  event_id: eventId, actor_id: userId, server_time: iso(-1000), access_until: iso(600000),
  policy: { event_id: eventId, venue_key: 'venue-key', provider_id: mainGrant.provider_id, source_revision: mainGrant.source_revision, revision: 9,
    timezone: 'Asia/Seoul', valid_until: iso(600000), fresh_ms: 10000, last_known_ms: 30000, fix_max_age_ms: 5000, future_skew_ms: 100,
    sharing_lease_ms: 60000, request_ttl_ms: 10000, check_in_ttl_ms: 10000, coordinate_retention_ms: 60000, command_retention_ms: 60000,
    max_accuracy_m: 50, max_viewers: 5 },
  clock: { event_id: eventId, venue_key: 'venue-key', policy_revision: 9, timezone: 'Asia/Seoul', server_time: iso(-1000), end_of_day: iso(600000), valid_until: iso(600000) },
  sharing: { state: 'OFF', revision: 12, viewer_ids: [], expires_at: null, updated_at: iso(-1000) }, self_location: null, friends: [],
  capabilities: { can_share_phone: true, can_pause: true, can_revoke: true, check_in_landmark_ids: ['real-main-gate'], geographic_projection: 'UNAVAILABLE' },
  incoming_requests: [], outgoing_requests: [],
})
const landmarkList = () => ({ event_id: eventId, server_time: iso(-1000), access_until: iso(600000), items: grants })
const errorBody = { code: 'PERMISSION_DENIED', message: 'Notes are unavailable for this viewer', traceId: null,
  timestamp: iso(-1000), details: { reason: 'PERMISSION_DENIED' } }
const noteDto = (row: Stored, meeting: boolean) => {
  const grant = grants.find(item => item.presentation_id === row.art)!
  return { id: row.id, landmark_id: row.physical, landmark_revision: grant.selection.landmark_revision,
    author: profile(row.authorId, row.authorName), text: row.withdrawn ? '' : row.body, created_at: new Date(row.createdAt).toISOString(),
    expires_at: new Date(row.expiresAt).toISOString(), revision: row.revision, audience: row.audience, expiry: row.expiry,
    parent_id: row.parentId, reply_count: store.filter(item => item.parentId === row.id && !item.withdrawn && item.expiresAt > clockMs).length,
    can_reply: row.parentId === null && row.authorId !== userId, can_withdraw: row.authorId === userId,
    meet_here: meeting && row.parentId === null && row.authorId !== userId ? { landmark_id: row.physical,
      recipient_user_id: friendId, landmark_revision: grant.selection.landmark_revision, access_until: iso(600000) } : null }
}
const page = (physical: string, limit: number) => {
  const roots = store.filter(row => !row.parentId && !row.withdrawn && row.physical === physical && row.expiresAt > clockMs)
  const items = roots.slice(0, limit).map(row => noteDto(row, true))
  const earliest = items.length ? Math.min(...items.map(row => Date.parse(row.expires_at))) : at - 1000
  return { items, visible_count: roots.length, parent: null, next_before: null, server_now: iso(-1000),
    access_until: new Date(Math.max(at + 1000, earliest)).toISOString() }
}
const thread = (note: string) => {
  const parent = store.find(row => row.id === note && !row.withdrawn)
  if (!parent) return null
  return { note: noteDto(parent, true), replies: store.filter(row => row.parentId === note && !row.withdrawn && row.expiresAt > clockMs)
    .map(row => noteDto(row, false)), next_before: null, server_now: iso(-1000), access_until: new Date(parent.expiresAt).toISOString() }
}
const meetingContext = () => ({ contextId: 'vm1_' + 'a'.repeat(64), eventId, actorId: userId,
  selection: { selectedFriendId: friendId, landmark: { landmarkId: 'real-main-gate', catalogRevision: 3, landmarkRevision: 4 } },
  conversationId: null, canRead: true, canSend: true, canInvite: true, relationshipRevision: 2, policyRevision: 9,
  serverTime: iso(-1000), validUntil: iso(600000), eventTimezone: 'Asia/Seoul', invitationExpiresAt: iso(600000),
  contentExpiresAt: iso(600000), purgeAt: iso(86_400_000) })
const messagePage = () => ({ contextId: 'vm1_' + 'a'.repeat(64), conversationId: null, items: [], cursor: '0', hasMore: false, gap: false, serverTime: iso(-1000) })
const receipt = (key: string, operation: 'createVenuePlaceNote' | 'createVenuePlaceReply' | 'withdrawVenuePlaceNote', id: string, revision: number) => {
  const value = { receipt_id: key, operation_id: operation, state: 'SUCCEEDED', resource: { id, revision }, failure_reason: null,
    created_at: iso(-1000), expires_at: iso(600000) }
  log.push({ path: 'receipt', method: operation, body: JSON.stringify(value) })
  return value
}

const log: { path: string; method: string; query?: string; body?: string; key?: string; result?: string; error?: string }[] = []
const http = { async request<T>(input: HttpRequest<T>): Promise<HttpResult<T>> {
  const ok = (value: unknown, status = 200) => ({ data: input.decode(value, { scope, status, path: input.path }), scope })
  const denied = () => ({ data: input.decode(errorBody, { scope, status: 403, path: input.path }), scope })
  const path = input.path
  const entry: { path: string; method: string; query?: string; body?: string; key?: string; error?: string } = { path, method: input.method }
  if (input.query) entry.query = JSON.stringify(input.query)
  if (input.body && 'json' in input.body) entry.body = JSON.stringify(input.body.json)
  if (input.idempotencyKey) entry.key = input.idempotencyKey
  if (path.includes('/venue/landmarks/')) log.push(entry)
  try { return respondNotes(input, path, ok, denied) } catch (error) { entry.error = error instanceof Error ? error.message : String(error); throw error }
} }
function respondNotes<T>(input: HttpRequest<T>, path: string, ok: (value: unknown, status?: number) => HttpResult<T>, denied: () => HttpResult<T>): HttpResult<T> {
  if (path.endsWith('/venue/location')) return ok(snapshot())
  if (path.endsWith('/venue/landmarks')) return ok(landmarkList())
  if (path.includes('/venue-meeting/')) {
    if (path.endsWith('/context')) return ok(meetingContext())
    if (path.endsWith('/messages')) return ok(messagePage())
    return ok({ commandKey: crypto.randomUUID(), state: 'UNKNOWN', serverTime: iso(-1000) })
  }
  if (path.includes('/people/')) return ok(profile(path.split('/').at(-1)!, 'Fixture friend'))
  const notes = path.indexOf('/venue/landmarks/')
  if (notes >= 0) {
    if (mode === 'denied') return denied()
    const suffix = path.slice(notes + '/venue/landmarks/'.length)
    const parts = suffix.split('/')
    const physical = decodeURIComponent(parts[0]!)
    const note = parts[1] === 'notes' ? parts[2] : undefined
    const replies = parts[3] === 'replies'
    if (input.method === 'POST' && note && replies) {
      const body = (input.body as { json: Record<string, unknown> }).json
      const parent = store.find(row => row.id === note)
      if (!parent) return denied()
      parent.revision += 1
      const id = crypto.randomUUID()
      store.push({ id, physical, art: parent.art, revision: 1, authorId: userId, authorName: 'Fixture fan',
        body: String(body.text), createdAt: clockMs, expiresAt: parent.expiresAt, audience: parent.audience,
        expiry: parent.expiry, parentId: parent.id })
      return ok(receipt(input.idempotencyKey!, 'createVenuePlaceReply', id, 1), 201)
    }
    if (input.method === 'POST' && !note) {
      const body = (input.body as { json: Record<string, unknown> }).json
      const grant = grants.find(item => item.presentation_id === parts[0]) ?? grants.find(item => item.selection.landmark_id === physical)
      if (!grant) return denied()
      const id = crypto.randomUUID()
      store.push({ id, physical, art: grant.presentation_id!, revision: 1, authorId: userId, authorName: 'Fixture fan',
        body: String(body.text), createdAt: clockMs, expiresAt: clockMs + (body.expiry === 'VENUE_END_DAY' ? 600000 : 3_600_000),
        audience: body.audience as Stored['audience'], expiry: body.expiry as Stored['expiry'], parentId: null })
      return ok(receipt(input.idempotencyKey!, 'createVenuePlaceNote', id, 1), 201)
    }
    if (input.method === 'DELETE' && note) {
      const parent = store.find(row => row.id === note)
      if (!parent || parent.authorId !== userId) return denied()
      parent.withdrawn = true; parent.revision += 1
      return ok(receipt(input.idempotencyKey!, 'withdrawVenuePlaceNote', parent.id, parent.revision))
    }
    if (note && replies) return ok(page(physical, Number(input.query?.limit ?? 20)))
    if (note) {
      const value = thread(note)
      return value ? ok(value) : denied()
    }
    return ok(page(physical, Number(input.query?.limit ?? 20)))
  }
  return ok({ user_id: path.split('/').at(-1), display_name: 'Fixture friend', bio: null, avatar })
}
const people: ReadPort<LocationPeople | null> = { read: () => ({ scope, venue: identity, accessUntilMs: at + 600000,
  self: { userId, displayName: 'Fixture fan', avatar: null, canMessage: true, canMeet: true },
  friends: [{ userId: friendId, displayName: 'Fixture friend', avatar: null, canMessage: true, canMeet: true }] }), subscribe: () => () => {} }
const sources: VenueShellSources = { http, people }

function Layout() {
  return <VenueShellFeatures auth={auth} scope={scope} community={community} participant={bootstrap.participant} sources={sources}><Outlet /></VenueShellFeatures>
}
const router = createMemoryRouter([{ element: <Layout />, children: [{ path: '*', element: <ParadiseCityMapPanel /> }] }], { initialEntries: ['/world/map'] })
declare global {
  interface Window {
    __VENUE_NOTES__: { state: string; advance(ms: number): void; log: typeof log }
  }
}
window.__VENUE_NOTES__ = { state: mode, advance(ms: number) { clockMs += ms; for (const listener of Array.from(clockListeners)) listener() }, log }
createRoot(document.getElementById('root')!).render(<StrictMode><RouterProvider router={router} /></StrictMode>)
