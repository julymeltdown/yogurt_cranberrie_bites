import { createRoot } from 'react-dom/client'
import { createBrowserRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../../../../app/router'
import { bootstrapFor } from '../../../../app/__tests__/fixtures/ports'
import type { ShellPorts } from '../../../../app/ports'
import { createSessionController } from '../../../auth/model/session'
import { createWorldReplication, type WorldReplication } from '../../../../world/network/replication'
import { WebSocketWorldTransport } from '../../../../world/network/websocket'
import type { Scheduler, WorldTicketResponse } from '../../../../world/network/WorldTransport'
import { createWorldAccessController, type WorldAccessRelease } from '../../model/controller'
import { WorldRooms } from '../../ui/WorldRooms'
import { WorldNearby } from '../../../world-nearby/WorldNearby'
import { catalog, initial, uid, errorResponse } from './harness'
import ticket from '../../../../../../contracts/fixtures/realtime/ticket-response.json'
import '../../../../shared/styles/base.css'
import '../../../../app/shell.css'
let accessRevoked = false
let roomMode = 'ok', replication: WorldReplication | null = null, release: WorldAccessRelease | null = null, disposed = false
let mounts = 0, disposals = 0, tickets = 0
const timers = new Set<ReturnType<typeof setTimeout>>()
const scheduler: Scheduler = { now: () => Date.now(), set(ms, fn) { const h = setTimeout(() => { timers.delete(h); fn() }, ms); timers.add(h); return h }, clear(h) { clearTimeout(h as ReturnType<typeof setTimeout>); timers.delete(h as ReturnType<typeof setTimeout>) } }
const listeners = new Set<() => void>(), roomRequests: { method: string; path: string; cache: string }[] = []
const auth = createSessionController({ eventId: initial.eventId, cookiePolicy: { available: true, onInvalidate: () => () => {}, run: (_kind, work) => work(), dispose() {} }, fetch: async input => {
  const r = input as Request; roomRequests.push({ method: r.method, path: new URL(r.url).pathname, cache: r.cache })
  if (roomMode === 'private') return errorResponse(403, 'PERMISSION_DENIED')
  if (roomMode === 'rate') return errorResponse(429, 'RATE_LIMITED')
  if (roomMode === 'outage') return errorResponse(503, 'DEPENDENCY_UNAVAILABLE')
  return Response.json(catalog(roomMode === 'full' ? 'FULL' : roomMode === 'unavailable' ? 'UNAVAILABLE' : 'AVAILABLE'))
} })
auth.session.replace(0, { userId: initial.selfUserId, eventId: initial.eventId, accessToken: 'fixture-token-memory-only' })
const controller = createWorldAccessController({ auth, world: { read: () => release, subscribe(fn) { listeners.add(fn); return () => { listeners.delete(fn) } } } })
const publish = () => listeners.forEach(fn => fn())
// The fixture's shell owner, not the feature, withdraws network authority on FE02 policy changes.
const stopOwnerPrivacy = auth.privacy.registerCleanup(() => { replication?.dispose(); release = null; publish() })
const ports: ShellPorts = { auth, session: auth.session, readiness: { read: () => 'ready', subscribe: () => () => {} }, bootstrap: { load: async scope => { if (accessRevoked) throw new Error('Fixture access revoked'); return bootstrapFor(scope) } }, admin: { authorize: async () => false },
  world: { mount(host, context) {
    mounts++
    const sink = document.createElement('p'); sink.id = 'authorized-world-sink'; sink.style.cssText = 'padding:1rem'; host.append(sink)
    const resources = { eventId: initial.eventId, roomId: initial.roomId, mapId: initial.mapId, mapRevision: initial.mapRevision, avatarManifestId: initial.avatarManifestId, avatarManifestVersion: initial.avatarManifestVersion }
    const transport = new WebSocketWorldTransport({ session: auth.session, origin: location.origin, scheduler, acquireTicket: async ({ body }) => {
      tickets++; const now = Date.now()
      return { ...ticket, ...body, ticket: String.fromCharCode(65 + tickets).repeat(43), server_time: new Date(now).toISOString(), expires_at: new Date(now + 30000).toISOString() } as WorldTicketResponse
    } })
    const owner = createWorldReplication({ session: auth.session, transport, resources, scheduler, sink: { replace(view) { sink.textContent = view.entities.length ? `Authorized world projection: ${view.entities.map(e => e.player.displayName).join(', ')}` : 'World projection cleared.' } } })
    replication = owner
    release = { scope: auth.privacy.capture(), capabilities: context.bootstrap.capabilities, replication: owner }; publish(); void owner.start()
    let closed = false
    return { setVisible(value) { owner.setVisible(value) }, setInputBlocked() {}, dispose() { if (closed) return; closed = true; disposals++; owner.dispose(); release = null; publish(); sink.remove() } }
  } },
}
history.replaceState(null, '', '/world/rooms')
const router = createBrowserRouter(createAppRoutes(ports, { panels: { '/world/rooms': <WorldRooms controller={controller} />, '/world/nearby': <WorldNearby controller={controller} /> } }))
const root = createRoot(document.getElementById('root')!); root.render(<RouterProvider router={router} />)
const style = document.createElement('style'); style.textContent = '#qualification{font:12px/1.4 system-ui;padding:8px;background:#faf6ed;position:relative;z-index:3}.world-shell{height:calc(100dvh - 50px)}'; document.head.append(style)
const bridge = {
  state: () => ({ feature: controller.read(), replication: replication?.read(), mounts, disposals, tickets, timers: timers.size, roomRequests }),
  async mode(value: string) { roomMode = value; await controller.loadRooms() },
  revoke() { accessRevoked = true; auth.privacy.changePolicy(auth.privacy.capture(), 'block'); return { nearby: controller.read().nearby.length, sinkPeople: replication?.read().entities.length, timers: timers.size } },
  pause() { replication?.setVisible(false) }, resume() { replication?.setVisible(true) },
  replace() { auth.session.replace(auth.session.read().generation, { userId: uid(99), eventId: initial.eventId, accessToken: 'replacement-fixture-token' }) },
  dispose() { if (disposed) return; disposed = true; controller.dispose(); root.unmount(); router.dispose(); stopOwnerPrivacy(); auth.dispose(); style.remove() },
}
window.__FE18__ = bridge
declare global { interface Window { __FE18__: typeof bridge } }
