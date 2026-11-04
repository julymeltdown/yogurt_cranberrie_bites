// TEST ONLY controlled network producer around actual main. No Auth, controller,
// availability reader, World composition or renderer is constructed by this harness.
import { capabilities, authResponse } from '../../../../features/auth/__tests__/fixtures/data'
import { admission } from '../../../../features/auth/__tests__/enrollment-consent/fixtures'
import { MockSocket } from '../../../../world/network/mock'
import { eventId, rooms } from '../../../world-entry/__tests__/data'
import { ticket } from '../../../../world/composition/__tests__/fixtures/data'
import { wholeData, frame as worldFrame } from '../../../../../e2e/reference-integrated/whole/data'
import { user as fixtureUser } from '../fixtures'
import type { WorldTicketRequest } from '../../../../world/network/WorldTransport'
import { scenarioService } from './service'

const data = await wholeData(), actor = data.bootstrap.participant.user_id
const service = scenarioService(actor, eventId), sockets: Socket[] = []
const requests: { path: string; url: string; method: string; key: string | null; baseline: boolean; authorized: boolean; cache: RequestCache }[] = []
const frames: { type: string; streamId: string; policyRevision?: number }[] = []
const originalFetch = globalThis.fetch
let serial = 0, baselineEnabled = false, policyRevision = 1, peakCanvases = 0, peakSockets = 0
let firstHost: Element | null = null, firstCanvas: Element | null = null
const own = structuredClone(data.bootstrap.participant)
function bind(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(bind)
  if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([key, item]) =>
    [key, key === 'event_id' ? eventId : ['user_id', 'owner_id', 'actor_id'].includes(key) && item === fixtureUser ? actor : bind(item)]))
  return value
}
globalThis.fetch = async (input, init) => {
  const r = input instanceof Request ? new Request(input, init) : new Request(new URL(String(input), location.origin), init)
  const url = new URL(r.url), path = url.pathname
  if (path.startsWith('/api/')) {
    requests.push({ path, url: r.url, method: r.method, key: r.headers.get('Idempotency-Key'), baseline: baselineEnabled, authorized: /^Bearer .+/.test(r.headers.get('Authorization') ?? ''), cache: r.cache })
    if (path === '/api/auth/capabilities') return Response.json(capabilities())
    if (path === '/api/auth/login') return Response.json(authResponse())
    if (path === '/api/auth/enrollment/admission') return Response.json(admission())
    if (path === '/api/auth/logout') return new Response(null, { status: 204 })
    if (path.endsWith('/bootstrap')) return Response.json(data.bootstrap)
    if (path.endsWith('/rooms')) return Response.json(rooms())
    if (path.endsWith('/world-tickets')) return Response.json(ticket(await r.json() as WorldTicketRequest, ++serial))
    if (path.endsWith('/places')) return Response.json({ event_id: eventId, revision: 1, items: data.bootstrap.map_manifest.places })
    if (path.endsWith('/me')) return Response.json(own)
    const response = await service.respond(r)
    if (/\/me\/blocks\//.test(path) && ['PUT', 'DELETE'].includes(r.method) && response.ok) policyRevision++
    return Response.json(bind(await response.json()), { status: response.status })
  }
  return originalFetch(input, init)
}
class Socket extends MockSocket {
  stream: string | null = null
  timer: ReturnType<typeof setInterval> | null = null
  constructor() { super(); sockets.push(this); peakSockets = Math.max(peakSockets, sockets.filter(s => s.readyState < 2).length); queueMicrotask(() => { if (this.readyState === 0) this.open() }) }
  baseline() {
    if (!this.stream || this.readyState !== 1 || !baselineEnabled) return
    frames.push({ type: 'policy.baseline', streamId: this.stream, policyRevision })
    this.receive(JSON.stringify({ protocolVersion: '1.0.0', type: 'policy.baseline', eventId, streamId: this.stream, scope: 'SOCIAL', policyRevision }))
  }
  override send(text: string) {
    super.send(text); const message = JSON.parse(text)
    if (message.type === 'authenticate' || message.type === 'switchRoom') queueMicrotask(() => {
      if (this.readyState !== 1) return
      const frame = worldFrame(data, message.roomId, data.avatar, message.type === 'switchRoom' ? 'ROOM_SWITCH' : 'INITIAL', message.requestId ?? null)
      this.stream = frame.streamId; frames.push({ type: 'snapshot', streamId: frame.streamId }); this.receive(JSON.stringify(frame)); this.baseline()
      if (!this.timer) this.timer = setInterval(() => { if (this.readyState === 1) this.receive(JSON.stringify({ protocolVersion: '1.0.0', type: 'ping', heartbeatId: crypto.randomUUID() })) }, 10000)
    })
  }
  override close() { if (this.timer) clearInterval(this.timer); this.timer = null; super.close() }
}
Object.defineProperty(globalThis, 'WebSocket', { configurable: true, writable: true, value: Socket })
const observer = new MutationObserver(() => {
  const host = document.querySelector('[data-world-host]'), canvas = host?.querySelector('canvas')
  if (host && !firstHost) firstHost = host
  if (canvas && !firstCanvas) firstCanvas = canvas
  peakCanvases = Math.max(peakCanvases, document.querySelectorAll('[data-world-host] canvas').length)
})
observer.observe(document.documentElement, { subtree: true, childList: true })
const fixture = {
  recipient: service.recipient,
  baseline() { baselineEnabled = true; sockets.forEach(s => s.baseline()) },
  mode(value: string) { service.mode(value) },
  state: () => ({ path: location.pathname, requests, frames, baselineEnabled, policyRevision, sockets: sockets.length,
    activeSockets: sockets.filter(s => s.readyState < 2).length, peakSockets, peakCanvases,
    canvases: document.querySelectorAll('[data-world-host] canvas').length,
    sameHost: firstHost === document.querySelector('[data-world-host]'), sameCanvas: firstCanvas === document.querySelector('[data-world-host] canvas') }),
}
Object.assign(window, { __SHARING_SAFETY__: fixture })
declare global { interface Window { __SHARING_SAFETY__: typeof fixture } }
history.replaceState(null, '', '/login?returnTo=%2Fmeet%2Fpeople')
await import('../../../../main')
