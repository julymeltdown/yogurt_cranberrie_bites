import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { createMemoryRouter, Outlet, RouterProvider } from 'react-router-dom'
import { VenueShellFeatures } from '../../../WorldShell'
import { ParadiseCityMapPanel } from '../../../venue-entry/ParadiseCityMapPanel'
import { VenueInvitationPanel } from '../../../venue-community-entry'
import type { VenueShellSources } from '../../../WorldShell'
import type { CommunityBootstrapBinding } from '../../../community-adoption/bootstrap'
import type { MeetingClockPort } from '../../../meeting-entry/ports'
import type { HttpRequest, HttpResult } from '../../../../shared/api/http'
import { TransportError } from '../../../../shared/api/errors'
import type { LocationSnapshot, LandmarkGrant } from '../../../../shared/api/paradise-city-v1/location'
import type { LocationPeople } from '../../../../features/paradise-city-map/model/location-adapter'
import type { ReadPort } from '../../../../features/paradise-city-map/ports'
import { createSessionController } from '../../../../features/auth/model/session'
import bootstrap from '../../../../../../contracts/fixtures/core/bootstrap-open.json'

const eventId = '00000000-0000-4000-8000-000000000201'
const userId = '00000000-0000-4000-8000-000000000101'
const friendId = '00000000-0000-4000-8000-000000000102'
const helenaId = '00000000-0000-4000-8000-000000000103'
const bulletId = '00000000-0000-4000-8000-000000000104'
const waydownId = '00000000-0000-4000-8000-000000000105'
const invitationId = '00000000-0000-4000-8000-000000000401'
const conversationId = '00000000-0000-4000-8000-000000000402'
const messageId = (n: number) => `00000000-0000-4000-8000-00000000050${n}`
const contextId = 'vm1_' + 'a'.repeat(64)
const at = 1_800_000_000_000
const iso = (offset = 0) => new Date(at + offset).toISOString()
const identity = { venueId: 'paradise-city', siteplanId: 'paradise-city-siteplan', revision: 1, variantId: 'photo-5-provisional' }

const grant = (landmarkId: string, label: string, presentation: string, revision: number): LandmarkGrant => ({
  event_id: eventId, selection: { landmark_id: landmarkId, catalog_revision: 3, landmark_revision: revision },
  provider_id: 'reviewed-provider', source_revision: 'source-1', venue_key: 'venue-key', map_id: 'map-1', map_revision: 2,
  policy_revision: 9, label, review_reference: 'review-1', reviewed_at: iso(-120000), server_time: iso(-1000),
  valid_until: iso(600000), physical_binding: null, presentation_id: presentation, can_check_in: true, can_meet: true, can_note: true,
})
const mainGrant = grant('real-main-gate', 'Main Gate', 'main-gate', 4)
const subGrant = grant('real-sub-gate', 'Sub Gate', 'sub-gate', 5)
const stationGrant: LandmarkGrant = { ...grant('real-station', 'Paradise City Station', 'station', 6), can_meet: false }
const plazaGrant: LandmarkGrant = { ...grant('real-art-paradiso-plaza', 'Art Paradiso / Plaza', 'art-paradiso-plaza', 7), can_meet: false }
const grants = [mainGrant, subGrant, stationGrant, plazaGrant]
const avatar = bootstrap.participant.profile.avatar
const profile = (id: string, name: string) => ({ user_id: id, display_name: name, bio: null, avatar })

type Mode = 'default' | 'map-markers' | 'sharing-active' | 'sharing-paused' | 'unavailable' | 'stale'
  | 'pending-invite' | 'accepted-invite' | 'conversation'
const rawState = new URLSearchParams(location.search).get('state')
const modes: readonly string[] = ['default', 'map-markers', 'sharing-active', 'sharing-paused', 'unavailable', 'stale',
  'pending-invite', 'accepted-invite', 'conversation']
const mode = (rawState !== null && modes.includes(rawState) ? rawState : 'default') as Mode

interface Stored {
  id: string; physical: string; art: string; revision: number; authorId: string; authorName: string
  body: string; createdAt: number; expiresAt: number; audience: 'FRIENDS' | 'EVERYONE'
  expiry: 'ONE_HOUR' | 'VENUE_END_DAY'; parentId: string | null; withdrawn?: boolean
}
const noteId = (suffix: string) => `00000000-0000-4000-8000-0000000003${suffix}`
let clockMs = at
const clockListeners = new Set<() => void>()
const store: Stored[] = [
  { id: noteId('01'), physical: 'real-main-gate', art: 'main-gate', revision: 4, authorId: friendId, authorName: 'inkparade',
    body: 'Trading stickers here until 18:30.', createdAt: at - 300_000, expiresAt: at + 3_600_000,
    audience: 'FRIENDS', expiry: 'ONE_HOUR', parentId: null },
  { id: noteId('02'), physical: 'real-main-gate', art: 'main-gate', revision: 1, authorId: userId, authorName: 'Fixture fan',
    body: 'This notice expires shortly.', createdAt: at - 2_500_000, expiresAt: at + 3_600_000,
    audience: 'EVERYONE', expiry: 'ONE_HOUR', parentId: null },
  { id: noteId('11'), physical: 'real-main-gate', art: 'main-gate', revision: 1, authorId: helenaId, authorName: 'helena',
    body: 'I will be there in a moment.', createdAt: at - 200_000, expiresAt: at + 3_600_000,
    audience: 'FRIENDS', expiry: 'ONE_HOUR', parentId: noteId('01') },
  { id: noteId('21'), physical: 'real-photo-zone', art: 'photo-zone', revision: 2, authorId: friendId, authorName: 'inkparade',
    body: 'Photo zone meetup after the encore.', createdAt: at - 400_000, expiresAt: at + 3_600_000,
    audience: 'EVERYONE', expiry: 'ONE_HOUR', parentId: null },
]

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

type FriendMode = 'phone' | 'check-in' | 'last-known' | 'unavailable'
function visible(id: string, kind: FriendMode, landmark: LandmarkGrant | null, observedOffset: number, freshOffset: number | null): Record<string, unknown> {
  return {
    user_id: id, source: kind === 'phone' ? 'PHONE' : kind === 'check-in' ? 'SELF_REPORTED_CHECK_IN' : kind === 'last-known' ? 'LAST_KNOWN' : 'UNAVAILABLE',
    position: kind === 'phone' || kind === 'last-known' ? { latitude: 37, longitude: 126, accuracy_m: 10,
      captured_at: iso(observedOffset), received_at: iso(observedOffset) } : null,
    landmark: kind === 'check-in' ? landmark : null,
    observed_at: kind === 'unavailable' ? null : iso(observedOffset),
    fresh_until: freshOffset === null ? null : iso(freshOffset),
    expires_at: kind === 'unavailable' ? null : iso(600000),
    access_until: iso(600000), visibility_revision: 8, physical_arrival_verified: false, can_request_location: true,
  }
}
const people = [
  { id: friendId, name: 'inkparade' }, { id: helenaId, name: 'helena' },
  { id: bulletId, name: 'bulletxs' }, { id: waydownId, name: 'waydown' },
]
const snapshot = (): LocationSnapshot => {
  const markers = mode === 'map-markers' || mode === 'sharing-active' || mode === 'conversation'
  const friends = markers ? [
    visible(friendId, 'check-in', mainGrant, -20_000, null),
    visible(helenaId, 'check-in', mainGrant, -480_000, null),
    visible(bulletId, 'check-in', mainGrant, -720_000, null),
    visible(waydownId, 'check-in', subGrant, -700_000, null),
  ] : [
    visible(friendId, 'phone', null, -20_000, 10_000),
    visible(helenaId, 'check-in', subGrant, -480_000, null),
    visible(bulletId, 'last-known', null, -720_000, null),
    visible(waydownId, 'unavailable', null, 0, null),
  ]
  const sharing = mode === 'sharing-active' || mode === 'map-markers' || mode === 'conversation'
    ? { state: 'ACTIVE' as const, revision: 12, viewer_ids: [friendId], expires_at: iso(60000), updated_at: iso(-1000) }
    : mode === 'sharing-paused'
      ? { state: 'PAUSED' as const, revision: 13, viewer_ids: [], expires_at: null, updated_at: iso(-1000) }
      : { state: 'OFF' as const, revision: 12, viewer_ids: [], expires_at: null, updated_at: iso(-1000) }
  return {
    event_id: eventId, actor_id: userId, server_time: iso(-1000), access_until: iso(600000),
    policy: { event_id: eventId, venue_key: 'venue-key', provider_id: mainGrant.provider_id, source_revision: mainGrant.source_revision, revision: 9,
      timezone: 'Asia/Seoul', valid_until: iso(600000), fresh_ms: 10000, last_known_ms: 30000, fix_max_age_ms: 5000, future_skew_ms: 100,
      sharing_lease_ms: 60000, request_ttl_ms: 10000, check_in_ttl_ms: 10000, coordinate_retention_ms: 60000, command_retention_ms: 60000,
      max_accuracy_m: 50, max_viewers: 5 },
    clock: { event_id: eventId, venue_key: 'venue-key', policy_revision: 9, timezone: 'Asia/Seoul', server_time: iso(-1000), end_of_day: iso(600000), valid_until: iso(600000) },
    sharing,
    self_location: (mode === 'sharing-active' || mode === 'map-markers' || mode === 'conversation')
      ? visible(userId, 'check-in', plazaGrant, -30_000, null) as never : null,
    friends: friends as never,
    capabilities: { can_share_phone: true, can_pause: true, can_revoke: true, check_in_landmark_ids: [mainGrant.selection.landmark_id, subGrant.selection.landmark_id], geographic_projection: 'UNAVAILABLE' },
    incoming_requests: [], outgoing_requests: [],
  }
}
const landmarkList = () => ({ event_id: eventId, server_time: iso(-1000), access_until: iso(600000), items: grants })

const invitationState = mode === 'accepted-invite' || mode === 'conversation' ? 'ACCEPTED' : 'PENDING'
const invitation = () => ({
  id: invitationId, eventId, senderId: friendId, recipientId: userId, landmarkId: mainGrant.selection.landmark_id,
  revision: 1, state: invitationState,
  createdAt: iso(-300_000), expiresAt: iso(86_400_000),
  allowedInvitationActions: invitationState === 'PENDING' ? ['ACCEPT', 'DECLINE'] : [],
})
const meetingContext = () => ({
  contextId, eventId, actorId: userId,
  selection: { selectedFriendId: friendId, landmark: { landmarkId: mainGrant.selection.landmark_id,
    catalogRevision: mainGrant.selection.catalog_revision, landmarkRevision: mainGrant.selection.landmark_revision } },
  conversationId: mode === 'conversation' || mode === 'accepted-invite' ? conversationId : null,
  canRead: true, canSend: true, canInvite: true, relationshipRevision: 2, policyRevision: 9,
  serverTime: iso(-1000), validUntil: iso(600000), eventTimezone: 'Asia/Seoul',
  invitationExpiresAt: iso(86_400_000), contentExpiresAt: iso(600000), purgeAt: iso(86_400_000),
})
const messages = () => ({
  contextId, conversationId: mode === 'conversation' || mode === 'accepted-invite' ? conversationId : null,
  items: mode === 'conversation' || mode === 'accepted-invite' ? [
    { id: messageId(1), sequence: 1, senderId: userId, body: 'I am near Main Gate. Meet here?',
      invitationId, createdAt: iso(-120_000), expiresAt: iso(600000) },
    { id: messageId(2), sequence: 2, senderId: friendId, body: 'On my way.',
      invitationId: null, createdAt: iso(-60_000), expiresAt: iso(600000) },
  ] : [],
  cursor: mode === 'conversation' || mode === 'accepted-invite' ? '2' : '0', hasMore: false, gap: false, serverTime: iso(-1000),
})
const inboxPage = (direction: string) => ({
  items: direction === 'INCOMING' ? [{ selection: { selectedFriendId: friendId,
    landmark: { landmarkId: mainGrant.selection.landmark_id, catalogRevision: mainGrant.selection.catalog_revision,
      landmarkRevision: mainGrant.selection.landmark_revision } }, invitation: invitation() }] : [],
  nextCursor: null, serverTime: iso(-1000),
})

const noteDto = (row: Stored, meeting: boolean) => {
  const g = grants.find(item => item.presentation_id === row.art)!
  return { id: row.id, landmark_id: row.physical, landmark_revision: g.selection.landmark_revision,
    author: profile(row.authorId, row.authorName), text: row.withdrawn ? '' : row.body, created_at: new Date(row.createdAt).toISOString(),
    expires_at: new Date(row.expiresAt).toISOString(), revision: row.revision, audience: row.audience, expiry: row.expiry,
    parent_id: row.parentId, reply_count: store.filter(item => item.parentId === row.id && !item.withdrawn && item.expiresAt > clockMs).length,
    can_reply: row.parentId === null && row.authorId !== userId, can_withdraw: row.authorId === userId,
    meet_here: meeting && row.parentId === null && row.authorId !== userId ? { landmark_id: row.physical,
      recipient_user_id: friendId, landmark_revision: g.selection.landmark_revision, access_until: iso(600000) } : null }
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
const receipt = (key: string, operation: 'createVenuePlaceNote' | 'createVenuePlaceReply' | 'withdrawVenuePlaceNote', id: string, revision: number) =>
  ({ receipt_id: key, operation_id: operation, state: 'SUCCEEDED', resource: { id, revision }, failure_reason: null,
    created_at: iso(-1000), expires_at: iso(600000) })
const deniedBody = { code: 'PERMISSION_DENIED', message: 'Notes are unavailable for this viewer', traceId: null,
  timestamp: iso(-1000), details: { reason: 'PERMISSION_DENIED' } }

const log: { path: string; method: string }[] = []
const http = { async request<T>(input: HttpRequest<T>): Promise<HttpResult<T>> {
  const ok = (value: unknown, status = 200) => ({ data: input.decode(value, { scope, status, path: input.path }), scope })
  const path = input.path
  log.push({ path, method: input.method })
  if (path.endsWith('/venue/location')) {
    if (mode === 'unavailable') throw new TransportError('http', { status: 503 })
    return ok(snapshot())
  }
  if (path.endsWith('/venue/landmarks')) return ok(landmarkList())
  if (path.includes('/venue-meeting/')) {
    if (path.endsWith('/context')) return ok(meetingContext())
    if (path.endsWith('/messages')) return ok(messages())
    if (path.endsWith('/invitations')) return ok(inboxPage(String(input.query?.direction ?? 'INCOMING')))
    if (path.includes('/invitations/') && !path.endsWith('/decision')) return ok(invitation())
    return ok({ commandKey: crypto.randomUUID(), state: 'UNKNOWN', serverTime: iso(-1000) })
  }
  if (path.includes('/people/')) return ok(profile(path.split('/').at(-1)!, 'Venue friend'))
  const notes = path.indexOf('/venue/landmarks/')
  if (notes >= 0) {
    const suffix = path.slice(notes + '/venue/landmarks/'.length)
    const parts = suffix.split('/')
    const physical = decodeURIComponent(parts[0]!)
    const note = parts[1] === 'notes' ? parts[2] : undefined
    const replies = parts[3] === 'replies'
    if (input.method === 'POST' && note && replies) {
      const body = (input.body as { json: Record<string, unknown> }).json
      const parent = store.find(row => row.id === note)
      if (!parent) return ok(deniedBody, 403)
      parent.revision += 1
      const id = crypto.randomUUID()
      store.push({ id, physical, art: parent.art, revision: 1, authorId: userId, authorName: 'Fixture fan',
        body: String(body.text), createdAt: clockMs, expiresAt: parent.expiresAt, audience: parent.audience,
        expiry: parent.expiry, parentId: parent.id })
      return ok(receipt(input.idempotencyKey!, 'createVenuePlaceReply', id, 1), 201)
    }
    if (input.method === 'POST' && !note) {
      const body = (input.body as { json: Record<string, unknown> }).json
      const g = grants.find(item => item.presentation_id === parts[0])
      if (!g) return ok(deniedBody, 403)
      const id = crypto.randomUUID()
      store.push({ id, physical, art: g.presentation_id!, revision: 1, authorId: userId, authorName: 'Fixture fan',
        body: String(body.text), createdAt: clockMs, expiresAt: clockMs + (body.expiry === 'VENUE_END_DAY' ? 600000 : 3_600_000),
        audience: body.audience as Stored['audience'], expiry: body.expiry as Stored['expiry'], parentId: null })
      return ok(receipt(input.idempotencyKey!, 'createVenuePlaceNote', id, 1), 201)
    }
    if (input.method === 'DELETE' && note) {
      const parent = store.find(row => row.id === note)
      if (!parent || parent.authorId !== userId) return ok(deniedBody, 403)
      parent.withdrawn = true; parent.revision += 1
      return ok(receipt(input.idempotencyKey!, 'withdrawVenuePlaceNote', parent.id, parent.revision))
    }
    if (note && replies) return ok(page(physical, Number(input.query?.limit ?? 20)))
    if (note) {
      const value = thread(note)
      return value ? ok(value) : ok(deniedBody, 403)
    }
    return ok(page(physical, Number(input.query?.limit ?? 20)))
  }
  return ok(profile(path.split('/').at(-1)!, 'Venue friend'))
} }

const directory: ReadPort<LocationPeople | null> = { read: () => ({ scope, venue: identity, accessUntilMs: at + 600000,
  self: { userId, displayName: 'Fixture fan', avatar: null, canMessage: true, canMeet: true },
  friends: people.map(person => ({ userId: person.id, displayName: person.name, avatar: null, canMessage: true, canMeet: true })) }),
  subscribe: () => () => {} }
const sources: VenueShellSources = { http, people: directory }

function Layout() {
  return <div style={{ display: 'flex', flexDirection: 'column', height: '100dvh' }}>
    <VenueShellFeatures auth={auth} scope={scope} community={community} participant={bootstrap.participant} sources={sources}>
      <div style={{ flex: 1, minHeight: 0, display: 'flex', flexDirection: 'column' }}><Outlet /></div>
    </VenueShellFeatures>
  </div>
}
const router = createMemoryRouter([{ element: <Layout />, children: [
  { path: '/world/map', element: <ParadiseCityMapPanel /> },
  { path: '/meet/invitations/:invitationId', element: <VenueInvitationPanel /> },
] }], { initialEntries: [new URLSearchParams(location.search).get('route') === 'invitation' ? `/meet/invitations/${invitationId}` : '/world/map'] })
declare global { interface Window { __VENUE_IN01__: { mode: Mode; advance(ms: number): void; log: typeof log } } }
window.__VENUE_IN01__ = { mode, advance(ms) { clockMs += ms; for (const listener of Array.from(clockListeners)) listener() }, log }
createRoot(document.getElementById('root')!).render(<StrictMode><RouterProvider router={router} /></StrictMode>)
