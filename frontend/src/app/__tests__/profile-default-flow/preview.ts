// Development-only wire input. No React/provider/availability/controller replacement.
// Adapted read-only from Root26227920 art05-default-entry/preview.ts; actual main is last.
import roomInput from '../../../../../contracts/fixtures/realtime/rooms.json'
import movement from '../../../../../contracts/fixtures/realtime/movement-heartbeat.json'
import postInput from '../../../../../contracts/fixtures/domain/public-post.json'
import { authResponse, capabilities } from '../../../features/auth/__tests__/fixtures/data'
import { parse, type Page, type Post } from '../../../features/event-feed/contract'
import { MockSocket } from '../../../world/network/mock'
import { validate } from '../../../world/network/transport/contract'
import type { Snapshot, WorldTicketRequest } from '../../../world/network/WorldTransport'
import { eligibleAttendeeFixture } from '../../world-entry/__tests__/map-presentation-v1/eligible-attendee.mjs'

if (!import.meta.env.DEV) throw Error('Controlled Profile journey is development-only')
const mode = new URLSearchParams(location.search).get('mode') ?? 'loaded'
if (!['loaded', 'denied-media', 'cold'].includes(mode)) throw Error('Unknown controlled Profile scenario')
const originalFetch = window.fetch.bind(window), originalBootstrap = eligibleAttendeeFixture()
// Explicit controlled server C03 capability, never a client availability port.
// The driver must prove it cannot enable Feed before an actual matching policy.baseline.
const bootstrap = { ...originalBootstrap, capabilities: { ...originalBootstrap.capabilities, feed: { available: true, reason: null } } }
bootstrap.participant.preferences.theme = 'PARADE'
const eventId = bootstrap.event.event_id, userId = bootstrap.participant.user_id
const base = `/api/concert/events/${eventId}`
const id = (tail: number) => `00000000-0000-4000-8000-${String(tail).padStart(12, '0')}`
const media = [
  { id: id(75), path: '/reference-feed-media-v1/reference-card-1.png', sha256: 'a28db3162d4fe822b17d3228ad82866d4eeb0e2524fb606d469ccd925fd7385d', width: 197, height: 67 },
  { id: id(76), path: '/reference-feed-media-v1/reference-card-2.png', sha256: '2c5863585bce002be0ea0b057f4927d67c6f99e97cc7f5edee6ba9c47589ffdd', width: 197, height: 46 },
]
const rows = parse<Page<Post>>('Domain_feed_FeedPage', { items: media.map((image, i) => ({
  ...postInput, post_id: id(92 - i), event_id: eventId,
  author: { bio: postInput.author.bio, user_id: userId, display_name: bootstrap.participant.profile.display_name, avatar: bootstrap.participant.profile.avatar },
  text: i === 0 ? 'Controlled own post: photos after the encore.' : 'Controlled own post: memories from the queue.',
  media_ids: i === 0 ? [image.id, id(77)] : [image.id], links: [],
  created_at: `2026-11-04T${i === 0 ? '10' : '09'}:00:00Z`, reaction_count: 0, comment_count: 0,
})), next_cursor: null })
const rooms = { ...structuredClone(roomInput), event_id: eventId }
const login = { ...authResponse(), userId }
const requests: { path: string; method: string; status: number }[] = []
const sockets: Socket[] = [], urls = { created: [] as string[], revoked: [] as string[] }
let peakSockets = 0, peakCanvases = 0
const createURL = URL.createObjectURL.bind(URL), revokeURL = URL.revokeObjectURL.bind(URL)
URL.createObjectURL = blob => { const url = createURL(blob); urls.created.push(url); return url }
URL.revokeObjectURL = url => { urls.revoked.push(url); revokeURL(url) }
const observe = () => {
  peakSockets = Math.max(peakSockets, sockets.filter(socket => socket.readyState < 2).length)
  peakCanvases = Math.max(peakCanvases, document.querySelectorAll('.shell-world-host canvas').length)
}
const observer = new MutationObserver(observe); observer.observe(document.body, { childList: true, subtree: true })

function snapshot(roomId: string): Snapshot {
  const geometry = bootstrap.map_manifest.geometry, spawn = geometry.spawns.find(row => row.spawn_id === geometry.default_spawn_id)!
  const preset = bootstrap.avatar_manifest.presets.find(row => row.config.preset_id === bootstrap.participant.profile.avatar.config.preset_id)!
  return validate<Snapshot>('Snapshot', { ...movement.messages[1]!.message, streamId: crypto.randomUUID(), eventId, roomId,
    mapId: bootstrap.map_manifest.map_id, mapRevision: bootstrap.map_manifest.map_revision,
    avatarManifestId: bootstrap.avatar_manifest.manifest_id, avatarManifestVersion: bootstrap.avatar_manifest.manifest_version,
    accessExpiresAt: new Date(Date.now() + 240_000).toISOString(), selfUserId: userId,
    players: [{ userId, displayName: bootstrap.participant.profile.display_name,
      position: { x: (spawn.cell_x + .5) * 16, y: (spawn.cell_y + .5) * 16 }, facing: 'down', moving: false,
      avatar: { revision: 1, config: preset.config } }],
  })
}
window.fetch = async (input, init) => {
  const request = input instanceof Request ? input : new Request(new URL(String(input), location.origin), init)
  const path = new URL(request.url).pathname
  if (!path.startsWith('/api/')) return originalFetch(input, init)
  const done = (response: Response) => { requests.push({ path, method: request.method, status: response.status }); return response }
  const denied = () => Response.json({ code: 'PERMISSION_DENIED', message: 'Controlled access denied', details: { reason: 'PERMISSION_DENIED' } }, { status: 403, headers: { 'Cache-Control': 'no-store' } })
  if (path === '/api/auth/capabilities') return done(Response.json(capabilities()))
  if (path === '/api/auth/login' && request.method === 'POST') return done(Response.json(login))
  if (path === '/api/auth/logout' && request.method === 'POST') return done(new Response(null, { status: 204 }))
  if (request.headers.get('authorization') !== `Bearer ${login.accessToken}`) return done(denied())
  if (path === `${base}/bootstrap`) return done(Response.json(bootstrap))
  if (path === `${base}/rooms`) return done(Response.json(rooms))
  if (path === `${base}/world-tickets` && request.method === 'POST') {
    const body = await request.json() as WorldTicketRequest
    return done(Response.json({ ...body, protocol_version: '1.0.0', event_id: eventId, ticket: 'A'.repeat(43),
      server_time: new Date().toISOString(), expires_at: new Date(Date.now() + 30_000).toISOString(), ws_path: '/world-ws', auth_timeout_ms: 3000 }))
  }
  if (path === `${base}/me`) return done(Response.json(bootstrap.participant))
  if (path === `${base}/feed` && request.method === 'GET') return done(Response.json(rows))
  const image = media.find(image => path === `${base}/media/${image.id}` || path === `${base}/media/${image.id}/content`)
  if (image && request.method === 'GET') {
    if (mode === 'denied-media') return done(denied())
    if (!path.endsWith('/content')) return done(Response.json({ media_id: image.id, state: 'READY', revision: 1,
      width: image.width, height: image.height, content_type: 'image/png', purpose: 'POST', read_path: `${base}/media/${image.id}/content` }))
    const response = await originalFetch(image.path, { cache: 'no-store', signal: request.signal })
    if (!response.ok) throw Error('Retained approved Feed media unavailable')
    const bytes = await response.arrayBuffer()
    const actual = [...new Uint8Array(await crypto.subtle.digest('SHA-256', bytes))].map(byte => byte.toString(16).padStart(2, '0')).join('')
    if (actual !== image.sha256) throw Error('Retained approved Feed image changed')
    return done(new Response(bytes, { headers: { 'Content-Type': 'image/png', 'Cache-Control': 'no-store' } }))
  }
  return done(Response.json({ code: 'RESOURCE_UNAVAILABLE', message: 'Controlled dependency unavailable', details: { reason: 'RESOURCE_UNAVAILABLE' } }, { status: 404 }))
}
class Socket extends MockSocket {
  baseline: Snapshot | null = null
  heartbeat: ReturnType<typeof setInterval> | null = null
  constructor() { super(); sockets.push(this); observe(); queueMicrotask(() => { this.open(); observe() }) }
  override close() { if (this.heartbeat) clearInterval(this.heartbeat); this.heartbeat = null; super.close(); observe() }
  override send(text: string) {
    super.send(text)
    const message = JSON.parse(text)
    if (message.type === 'authenticate') queueMicrotask(() => {
      if (this.readyState !== 1) return
      this.baseline = snapshot(message.roomId); this.receive(JSON.stringify(this.baseline))
      this.heartbeat = setInterval(() => { if (this.readyState === 1) this.receive(JSON.stringify({ protocolVersion: '1.0.0', type: 'ping', heartbeatId: crypto.randomUUID() })) }, 5000)
    })
  }
}
Object.defineProperty(window, 'WebSocket', { configurable: true, writable: true, value: Socket })
// These methods deliver server wire frames only; never call Auth/privacy/availability hooks.
function deliverPolicy(invalidate = false) {
  const socket = [...sockets].reverse().find(socket => socket.readyState === 1 && socket.baseline)
  if (!socket?.baseline) throw Error('No incumbent admitted World stream')
  const frame = { protocolVersion: '1.0.0', type: invalidate ? 'policy.invalidate' : 'policy.baseline',
    streamId: socket.baseline.streamId, eventId, scope: 'SOCIAL', policyRevision: invalidate ? 18 : 17,
    ...(invalidate ? { reason: 'CONSENT_CHANGED' } : {}) }
  validate(invalidate ? 'PolicyInvalidated' : 'PolicyBaseline', frame)
  socket.receive(JSON.stringify(frame))
}
const api = {
  baseline: () => deliverPolicy(), invalidate: () => deliverPolicy(true),
  state() {
    observe()
    return { qualification: 'ACTUAL_MAIN_CONTROLLED_CANONICAL_HTTP_C05_NOT_LIVE', mode, path: location.pathname,
      requests: structuredClone(requests), feedReads: requests.filter(row => row.path === `${base}/feed`).length,
      mediaReads: requests.filter(row => row.path.includes('/media/')).length,
      socketsCreated: sockets.length, activeSockets: sockets.filter(socket => socket.readyState < 2).length, peakSockets,
      admittedStreams: sockets.filter(socket => socket.readyState === 1 && socket.baseline !== null).length,
      canvases: document.querySelectorAll('.shell-world-host canvas').length, peakCanvases, urls: structuredClone(urls) }
  },
  disposeInputs() { observer.disconnect(); for (const socket of sockets) socket.close() },
}
Object.assign(window, { __PROFILE_DEFAULT_FLOW__: api })
declare global { interface Window { __PROFILE_DEFAULT_FLOW__: typeof api } }
await import('../../../main')
