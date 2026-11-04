// TEST_ONLY: adapted Community adoption native setup from published 5da64426; synthetic media transport added.
// Synthetic HTTP/C05/clock policy; actual default router, providers, renderer and Phaser runtime.
// Actual canonical Auth, browser router, default production loader and whole-character renderer.
// Synthetic HTTP/C05 and isolated map admission. No product/source approval or physical arrival.
import { giveaway } from '../../fixtures/data'
import original from '../../../../../../../contracts/fixtures/core/bootstrap-open.json'
import { contentHash } from '../../../../../world/map/json'
import type { CommunityClockProfile } from '../../../../../app/community-adoption/bootstrap'
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { RouterProvider } from 'react-router-dom'
import { createAppRouter } from '../../../../../app/router'
import { createProductionPorts } from '../../../../../app/ports'
import { createSessionController } from '../../../../auth/model/session'
import { isolatedCookiePolicy, capabilities, authResponse } from '../../../../auth/__tests__/fixtures/data'
import { wholeData, eventId, userId, frame, participant } from '../../../../../../e2e/reference-integrated/whole/data'
import { domainResponse } from '../../../../../../e2e/reference-integrated/data'
import { response as socialResponse } from '../../../../../app/social-composition/__tests__/fixtures'
import { rooms } from '../../../../../app/world-entry/__tests__/data'
import { ticket } from '../../../../../world/composition/__tests__/fixtures/data'
import { MockSocket } from '../../../../../world/network/mock'
import type { WorldTicketRequest } from '../../../../../world/network/WorldTransport'
import '../../../../../shared/styles/base.css'
import '../../../../../app/shell.css'

if (!import.meta.env.DEV) throw Error('QA06 fixture is development-only')
const params = new URLSearchParams(location.search), data = await wholeData()
const era = params.get('era'), authMode = params.get('authMode')
const scenario: string = params.get('scenario') ?? 'enabled'
let mediaMode = params.get('mediaMode') ?? 'ready'
const mediaId = '00000000-0000-4000-8000-000000000052'
const giftId = '00000000-0000-4000-8000-000000000005'
const pending: { request: Request; resolve: (response: Response) => void }[] = []
const png = Uint8Array.from(atob('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aGr8AAAAASUVORK5CYII='), c => c.charCodeAt(0))
const createdUrls: string[] = [], revokedUrls: string[] = [], protectedUrls: string[] = []
const originalCreate = URL.createObjectURL.bind(URL), originalRevoke = URL.revokeObjectURL.bind(URL)
// TEST_ONLY classification of the existing exact-size PNG transport bytes; public character textures stay separate.
URL.createObjectURL = blob => { const url = originalCreate(blob); createdUrls.push(url); if (blob instanceof Blob && blob.type === 'image/png' && blob.size === png.byteLength) protectedUrls.push(url); return url }
URL.revokeObjectURL = url => { revokedUrls.push(url); originalRevoke(url) }
if (!['world-only', 'enabled', 'no-policy', 'virtual'].includes(scenario)) throw Error('Unknown TEST_ONLY scenario')
document.title = `TEST_ONLY Gifts protected media: ${mediaMode}`
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
const requests: { path: string; method: string; authorized: boolean; aborted: boolean }[] = [], sockets: Socket[] = []
let serial = 0, socketPeak = 0, disposed = false
// Project contract fixture identities/avatars to the current selected whole-character shape.
// Never load or display the legacy fixture avatar assets.
function bind(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(bind)
  if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([key, item]) =>
    [key, key === 'event_id' ? eventId : ['owner_id', 'actor_id'].includes(key) ? userId : key === 'avatar' ? structuredClone(data.avatar) : bind(item)]))
  return value
}
function gift() { return { ...giveaway(), event_id: eventId, image_media_id: mediaMode === 'none' ? null : mediaId, public_place_id: data.bootstrap.map_manifest.places[0]!.place_id } }
const auth = createSessionController({ eventId, cookiePolicy: isolatedCookiePolicy(), fetch: async input => {
  const request = input as Request, path = new URL(request.url).pathname
  const recorded = { path, method: request.method, authorized: !!request.headers.get('authorization'), aborted: request.signal.aborted }; requests.push(recorded)
  request.signal.addEventListener('abort', () => { recorded.aborted = true }, { once: true })
  if (path.includes('/media/')) {
    if (mediaMode === 'denied') return new Response(null, { status: 403 })
    if (path.endsWith('/content')) {
      if (mediaMode === 'held') return new Promise<Response>(resolve => pending.push({ request, resolve }))
      return new Response(png, { headers: { 'Content-Type': 'image/png' } })
    }
    return Response.json({ media_id: mediaId, state: 'READY', revision: 1, width: 1, height: 1, content_type: 'image/png', purpose: 'GIVEAWAY', read_path: `/api/concert/events/${eventId}/media/${mediaId}/content` })
  }
  if (path.endsWith(`/giveaways/${giftId}`)) return Response.json(gift())
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
  const domain = domainResponse(request, eventId, userId, data.bootstrap.map_manifest.places[0]!.place_id)
  if (path.endsWith('/giveaways') && !path.endsWith('/me/giveaways')) return Response.json({ items: [gift()], next_cursor: null })
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
      mediaMode, createdUrls, revokedUrls, protectedUrls, pending: pending.length, mediaId, giftId, canvases: document.querySelectorAll('.shell-world-host canvas').length, disposed } },
  setMediaMode(value: string) { mediaMode = value },
  releaseMedia() { pending.splice(0).forEach(item => item.resolve(new Response(png, { headers: { 'Content-Type': 'image/png' } }))) },
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
Object.assign(window, { __GIFTS_MEDIA_NATIVE__: api })
declare global { interface Window { __GIFTS_MEDIA_NATIVE__: typeof api } }
