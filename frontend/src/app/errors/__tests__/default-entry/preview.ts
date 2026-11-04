// TEST ONLY: actual main.tsx, canonical Auth/login UI and default production loader.
// HTTP/socket simulation adapted from app/__tests__/cozy-backgrounds-v1/preview.tsx.
// Selected character originals stay unchanged; map admission remains synthetic.
import { wholeData, eventId, userId, frame, participant } from '../../../../../e2e/reference-integrated/whole/data'
import { domainResponse } from '../../../../../e2e/reference-integrated/data'
import { response as socialResponse } from '../../../social-composition/__tests__/fixtures'
import { capabilities, authResponse } from '../../../../features/auth/__tests__/fixtures/data'
import { rooms } from '../../../world-entry/__tests__/data'
import { ticket } from '../../../../world/composition/__tests__/fixtures/data'
import { MockSocket } from '../../../../world/network/mock'
import type { WorldTicketRequest } from '../../../../world/network/WorldTransport'

if (!import.meta.env.DEV) throw Error('FE41 fixture is development-only')
const data = await wholeData(), originalFetch = window.fetch.bind(window)
const requests: { path: string; method: string }[] = [], sockets: Socket[] = []
let serial = 0, peak = 0, holdSnapshot = false, snapshots = 0
function bind(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(bind)
  if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([key, item]) =>
    [key, key === 'event_id' ? eventId : ['owner_id', 'actor_id'].includes(key) ? userId : key === 'avatar' ? structuredClone(data.avatar) : bind(item)]))
  return value
}
window.fetch = async (input, init) => {
  const request = input instanceof Request ? input : new Request(new URL(String(input), location.origin), init)
  const path = new URL(request.url).pathname
  if (!path.startsWith('/api/')) return originalFetch(input, init)
  requests.push({ path, method: request.method })
  if (path === '/api/auth/capabilities') return Response.json(capabilities())
  if (path === '/api/auth/login') return Response.json(authResponse())
  if (path === '/api/auth/logout') return new Response(null, { status: 204 })
  if (path.endsWith('/bootstrap')) return Response.json(data.bootstrap)
  if (path.endsWith('/rooms')) return Response.json(rooms())
  if (path.endsWith('/world-tickets')) return Response.json(ticket(await request.json() as WorldTicketRequest, ++serial))
  if (path.endsWith('/places')) return Response.json({ event_id: eventId, revision: 1, items: data.bootstrap.map_manifest.places })
  if (path.includes('/places/')) return Response.json({ event_id: eventId, revision: 1, place: data.bootstrap.map_manifest.places.find(p => p.place_id === path.split('/').at(-1)) })
  if (path.endsWith('/me')) return Response.json(participant(data, data.avatar))
  const response = domainResponse(request, eventId, userId, data.bootstrap.map_manifest.places[0]!.place_id) ?? await socialResponse(request)
  const value = bind(await response.json()) as Record<string, unknown>
  if (path.endsWith('/me/age-status')) value.user_id = userId
  if (path.includes('/people/')) value.user_id = path.split('/').at(-1)
  return Response.json(value, { status: response.status })
}
class Socket extends MockSocket {
  room = rooms().default_room_id
  constructor() { super(); sockets.push(this); peak = Math.max(peak, sockets.filter(s => s.readyState < 2).length); queueMicrotask(() => this.open()) }
  override send(text: string) {
    super.send(text); const message = JSON.parse(text)
    if (['authenticate', 'switchRoom', 'resync'].includes(message.type)) {
      this.room = message.roomId ?? this.room
      if (!holdSnapshot) queueMicrotask(() => this.admit(message.type === 'switchRoom' ? 'ROOM_SWITCH' : message.type === 'resync' ? 'RESYNC' : 'INITIAL', message.requestId ?? null))
    }
  }
  admit(cause: 'INITIAL' | 'ROOM_SWITCH' | 'RESYNC' = 'INITIAL', requestId: string | null = null) {
    if (this.readyState !== 1) return
    const snapshot = { ...frame(data, this.room, data.avatar, cause, requestId),
      streamId: `00000000-0000-4000-8000-${String(++snapshots).padStart(12, '0')}` }
    // Different server stream on every explicit admission; no client state injection.
    this.receive(JSON.stringify(snapshot))
  }
}
Object.defineProperty(window, 'WebSocket', { configurable: true, writable: true, value: Socket })
const api = {
  state: () => ({ sockets: sockets.length, active: sockets.filter(s => s.readyState < 2).length, peak,
    canvases: document.querySelectorAll('.shell-world-host canvas').length, requests, snapshots }),
  offline(value: boolean) { Object.defineProperty(navigator, 'onLine', { configurable: true, get: () => !value }); dispatchEvent(new Event(value ? 'offline' : 'online')) },
  close() { sockets.at(-1)?.serverClose() },
  hold(value: boolean) { holdSnapshot = value },
  admit() { sockets.at(-1)?.admit() },
}
Object.assign(window, { __FE41_ENTRY__: api })
declare global { interface Window { __FE41_ENTRY__: typeof api } }
// No replacement controller, ports, router, auth/session, renderer or panel.
await import('../../../../main')
