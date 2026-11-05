// TEST_ONLY: adapted byte-preserving application setup from Root32fed3 cozy-backgrounds-v1.
// Synthetic HTTP/C05/clock policy; actual default router, providers, renderer and Phaser runtime.
// Actual canonical Auth, browser router, default production loader and whole-character renderer.
// Synthetic HTTP/C05 and isolated map admission. No product/source approval or physical arrival.
import original from '../../../../../../contracts/fixtures/core/bootstrap-open.json'
import { contentHash } from '../../../../world/map/json'
import type { CommunityClockProfile } from '../../../../app/community-adoption/bootstrap'
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { RouterProvider } from 'react-router-dom'
import { createAppRouter } from '../../../../app/router'
import { createProductionPorts } from '../../../../app/ports'
import { createSessionController } from '../../../../features/auth/model/session'
import { isolatedCookiePolicy, capabilities, authResponse } from '../../../../features/auth/__tests__/fixtures/data'
import { admission } from '../../../../features/auth/__tests__/enrollment-consent/fixtures'
import { wholeData, eventId, userId, frame, participant } from '../../../../../e2e/reference-integrated/whole/data'
import { domainResponse } from '../../../../../e2e/reference-integrated/data'
import { response as socialResponse } from '../../../../app/social-composition/__tests__/fixtures'
import { rooms } from '../../../../app/world-entry/__tests__/data'
import { ticket } from '../../../../world/composition/__tests__/fixtures/data'
import { MockSocket } from '../../../../world/network/mock'
import type { WorldTicketRequest } from '../../../../world/network/WorldTransport'
import '../../../../shared/styles/base.css'
import { gift as fixtureGift, reservation as fixtureReservation } from '../../../gift-pickup/__tests__/harness'
import { errorResponse, pendingReceipt } from '../../../party-discovery/__tests__/fixtures/data'
import '../../../../app/shell.css'

if (!import.meta.env.DEV) throw Error('QA06 fixture is development-only')
const params = new URLSearchParams(location.search), data = await wholeData()
const era = params.get('era'), authMode = params.get('authMode')
const scenario = params.get('scenario') ?? 'enabled'
const visualCase = params.get('case') ?? 'normal', isOwner = params.get('owner') === '1'
if (!['world-only', 'enabled', 'no-policy', 'virtual'].includes(scenario)) throw Error('Unknown TEST_ONLY scenario')
document.title = `TEST_ONLY PI04 Giveaway: ${scenario}`
document.documentElement.dataset.mcrFixture = 'TEST_ONLY community authority'
if (scenario === 'world-only') Object.assign(data.bootstrap.capabilities, structuredClone(original.capabilities))
if (scenario === 'enabled' || scenario === 'no-policy') {
  const place = data.bootstrap.map_manifest.places[0]!
  Object.assign(place, { place_kind: 'PHYSICAL', verification_state: 'VERIFIED', closure_state: 'OPEN',
    provenance: { ...place.provenance, source_kind: 'PLACE_REFERENCE', source_uri: 'https://fixture.example.invalid/venue',
      review_state: 'REVIEWED', reviewed_by: 'TEST_ONLY synthetic reviewer', reviewed_at: data.bootstrap.server_time } })
  data.bootstrap.map_manifest.content_hash = await contentHash(data.bootstrap.map_manifest)
  for (const theme of data.bootstrap.theme_manifests) {
    theme.map_content_hash = data.bootstrap.map_manifest.content_hash
    theme.content_hash = await contentHash(theme)
  }
}
const testProfile: CommunityClockProfile = Object.freeze({ classification: 'TEST_ONLY', evidence: 'Synthetic native fixture, not measured platform guarantees',
  maxAgeMs: 30000, uncertaintyMs: 1000, maxElapsedRate: 1.01, maxObservationGapMs: 5000 })
if (era === 'REVENGE' || era === 'PARADE' || era === 'DANGER') data.bootstrap.participant.preferences.theme = era
const initial = params.get('entry') ?? '/world', signedIn = !['/', '/login'].includes(initial)
const requests: { path: string; method: string }[] = [], sockets: Socket[] = []
let serial = 0, socketPeak = 0, disposed = false, claimed = false
// Project contract fixture identities/avatars to the current selected whole-character shape.
// Never load or display the legacy fixture avatar assets.
function bind(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(bind)
  if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([key, item]) =>
    [key, key === 'event_id' ? eventId : ['owner_id', 'actor_id'].includes(key) ? userId : key === 'avatar' ? structuredClone(data.avatar) : bind(item)]))
  return value
}
const auth = createSessionController({ eventId, cookiePolicy: isolatedCookiePolicy(), fetch: async input => {
  const request = input as Request, path = new URL(request.url).pathname
  requests.push({ path, method: request.method })
  if (request.method === 'GET' && path === '/api/auth/enrollment/admission') return Response.json(admission())
  if (path === '/api/auth/capabilities') return authMode === 'options-unavailable' ? new Response(null,{status:503}) : Response.json(capabilities())
  if (path === '/api/auth/login') {
    if (authMode === 'pending') return new Promise<Response>((_resolve,reject) => request.signal.addEventListener('abort', () => reject(new DOMException('Synthetic pending login aborted','AbortError')), { once:true }))
    if (authMode === 'error') return Response.json({code:'UNAUTHENTICATED',message:'Synthetic denied credentials',details:{reason:'UNAUTHENTICATED'}},{status:401})
    return Response.json(authResponse())
  }
  if (path === '/api/auth/logout') return new Response(null, { status: 204 })
  if (path.endsWith('/bootstrap')) return Response.json(data.bootstrap)
  if (path.endsWith('/rooms')) return Response.json(rooms())
  if (path.endsWith('/world-tickets')) return Response.json(ticket(await request.json() as WorldTicketRequest, ++serial))
  if (path.endsWith('/places')) return Response.json({ event_id: eventId, revision: 1, items: data.bootstrap.map_manifest.places })
  if (path.includes('/places/')) return Response.json({ event_id: eventId, revision: 1, place: data.bootstrap.map_manifest.places.find(p => p.place_id === path.split('/').at(-1)) })
  if (path.endsWith('/me')) return Response.json(participant(data, data.avatar))
  const instant = Date.parse(data.bootstrap.server_time), at = (offset: number) => new Date(instant + offset).toISOString()
  const sourceGift = fixtureGift(), sourceReservation = fixtureReservation()
  const giver = isOwner ? userId : '00000000-0000-4000-8000-000000000044'
  const g = { ...sourceGift, event_id: eventId, owner: { ...sourceGift.owner, user_id: giver, avatar: structuredClone(data.avatar) },
    image_media_id: null, public_place_id: data.bootstrap.map_manifest.places[0]!.place_id,
    available: visualCase === 'sold-out' ? 0 : sourceGift.available, pickup_starts_at: at(-300000), pickup_ends_at: at(3600000) }
  const r = { ...sourceReservation, event_id: eventId, giveaway_id: g.giveaway_id,
    recipient_id: isOwner ? '00000000-0000-4000-8000-000000000045' : userId,
    claim_receipt_id: claimed ? '00000000-0000-4000-8000-000000000081' : null, revision: claimed ? 2 : 1,
    state: claimed ? 'CLAIMED' as const : visualCase === 'expired' ? 'EXPIRED' as const : visualCase === 'cancelled' ? 'CANCELLED' as const : 'READY' as const,
    created_at: at(-600000), expires_at: at(visualCase === 'expired' ? -60000 : 1200000) }
  if (request.method !== 'GET' && path.endsWith('/reservations')) {
    if (visualCase === 'uncertain') throw new TypeError('TEST_ONLY lost command response')
    if (visualCase === 'pending') return Response.json(pendingReceipt(request.headers.get('Idempotency-Key')!, { event_id: eventId, actor_id: userId, operation_id: 'reserveConcertGiveaway' }), { status: 202 })
    return Response.json({ ...r, state: 'HELD' })
  }
  if (request.method !== 'GET' && path.endsWith('/claim')) {
    claimed = true
    return Response.json({ receipt_id: '00000000-0000-4000-8000-000000000081', reservation_id: r.reservation_id, giveaway_id: g.giveaway_id, state: 'CLAIMED', revision: 2, claimed_at: at(0) })
  }
  if (request.method !== 'GET' && /\/giveaways\/[^/]+$/.test(path) && visualCase === 'conflict') return errorResponse(409, 'VERSION_CONFLICT')
  if (path.endsWith('/me/giveaways')) return Response.json({ giving: { items: [{ ...g, owner: { ...g.owner, user_id: userId }, state: 'DRAFT' }], next_cursor: null }, receiving: { items: [r], next_cursor: null } })
  if (visualCase === 'loading' && path.endsWith('/giveaways')) return new Promise<Response>(() => {})
  if (visualCase === 'error' && path.endsWith('/giveaways')) return new Response(null, { status: 503 })
  if (path.endsWith('/giveaways')) return Response.json({ items: [g, { ...g, giveaway_id: '00000000-0000-4000-8000-000000000062', title: 'Long TEST_ONLY title that wraps naturally across available space', available: 0, created_at: new Date(Date.parse(g.created_at) - 86400000).toISOString() }], next_cursor: null })
  if (path.endsWith('/management')) return Response.json({ giveaway: g, reserved_unclaimed: 1, quantity_claimed: 1, reservations: [r], next_cursor: null })
  // The default adoption owner has no reviewed pickup authority producer.
  if (path.endsWith('/pickup')) return errorResponse(404, 'RESOURCE_UNAVAILABLE')
  if (/\/giveaways\/[^/]+$/.test(path)) return Response.json(g)
  if (/\/reservations\/[^/]+$/.test(path)) return Response.json(r)
  const domain = domainResponse(request, eventId, userId, data.bootstrap.map_manifest.places[0]!.place_id)
  const response = domain ?? await socialResponse(request)
  const value = bind(await response.json()) as Record<string, unknown>
  // Public authors stay distinct from the actor. Private age status and a queried
  // public-profile subject are explicitly bound to their requested fixture identity.
  if (path.endsWith('/me/age-status')) value.user_id = userId
  if (path.includes('/people/')) value.user_id = path.split('/').at(-1)
  return Response.json(value, { status: response.status })
} })
class Socket extends MockSocket {
  constructor() { super(); sockets.push(this); socketPeak = Math.max(socketPeak, sockets.filter(s => s.readyState < 2).length); queueMicrotask(() => this.open()) }
  override send(text: string) {
    super.send(text); const message = JSON.parse(text)
    if (['authenticate', 'switchRoom', 'resync'].includes(message.type)) queueMicrotask(() => {
      if (this.readyState === 1) this.receive(JSON.stringify(frame(data, message.roomId ?? rooms().default_room_id, data.avatar,
        message.type === 'switchRoom' ? 'ROOM_SWITCH' : message.type === 'resync' ? 'RESYNC' : 'INITIAL', message.requestId ?? null)))
    })
  }
}
Object.defineProperty(window, 'WebSocket', { configurable: true, writable: true, value: Socket })
auth.start(); while (auth.read().capabilities === 'loading') await new Promise(resolve => setTimeout(resolve, 5))
if (signedIn && !await auth.login('fixture@example.test', 'fixture-password')) throw Error('Synthetic canonical login failed')
history.replaceState(null, '', initial)
// No loader/resolver/assetFetch/engine/renderer or panel override. Availability is TEST ONLY.
const ports = createProductionPorts(auth, { origin: location.origin, localInput: {
  revision: 'TEST_ONLY_QA06_not_C05_authority', reviewedBy: 'Synthetic visual fixture',
  movementPolicy: { speedPxPerSecond: 96, footWidthPx: 12, footHeightPx: 8, interactionRadiusPx: 96 }, zoom: 1,
} }, { availability(context) { const value = { bootstrap: context.bootstrap, scope: context.scope, services: { feed: true, people: true, intro: true, age: true, inbox: true } }; return { read: () => value, subscribe: () => () => {} } } })
const configuredPorts = { ...ports, ...(scenario === 'no-policy' ? {} : { communityClock: testProfile }) }
const router = createAppRouter(configuredPorts), root = createRoot(document.getElementById('root')!)
root.render(<StrictMode><RouterProvider router={router} /></StrictMode>)
const api = {
  state() { const binding = ports.worldAccess?.read(), presentation = binding?.presentation?.read(), replication = binding?.replication.read()
    return { scenario, capabilityMode: scenario === 'world-only' ? 'Exact committed C03 fixture: ALL capabilities disabled, including World' : 'TEST_ONLY synthetic capabilities', classification: 'TEST_ONLY', policy: scenario === 'no-policy' ? null : testProfile, path: router.state.location.pathname, published: !!presentation, theme: presentation?.theme.read(), camera: presentation?.camera.read(),
      phase: replication?.phase, entities: replication?.entities.length, avatarPin: data.pin, requests,
      sockets: sockets.length, activeSockets: sockets.filter(s => s.readyState < 2).length, socketPeak,
      canvases: document.querySelectorAll('.shell-world-host canvas').length, disposed } },
  navigate: (path: string) => router.navigate(path),
  interruptClock() {
    Object.defineProperty(document, 'visibilityState', { configurable: true, value: 'hidden' })
    document.dispatchEvent(new Event('visibilitychange'))
    Object.defineProperty(document, 'visibilityState', { configurable: true, value: 'visible' })
    document.dispatchEvent(new Event('visibilitychange'))
  },
  suspendPrivacy() { auth.privacy.changePolicy(auth.privacy.capture(), 'suspended') },
  async dispose() { if (disposed) return; disposed = true; root.unmount(); router.dispose(); ports.dispose?.(); auth.dispose() },
}
Object.assign(window, { __PI04_NATIVE__: api })
declare global { interface Window { __PI04_NATIVE__: typeof api } }
