// TEST ONLY: deterministic public seams; no product fallback resources.
import { useSyncExternalStore } from 'react'
import { createRoot } from 'react-dom/client'
import { createBrowserRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../../router'
import type { WorldAccessBinding, ShellPorts } from '../../ports'
import { bootstrapFor } from '../../__tests__/fixtures/ports'
import { createSessionController } from '../../../features/auth/model/session'
import { ConnectionStatus } from '../../../features/connection-status'
import { createConnectionObserver, type ServiceDecision, type RecoveryAction } from '../observer'
import { RouteState } from '../RouteState'
import { createWorldRuntime, type WorldRuntime } from '../../../world/runtime'
import { loadPhaserEngine } from '../../../world/scenes/MapScene'
import { loadMap } from '../../../world/map/resources'
import { movementData } from '../../../world/__tests__/fixtures/movement-data'
import { createWorldReplication, type WorldReplication } from '../../../world/network/replication'
import { MockWorldClock, MockWorldTransport } from '../../../world/network/mock'
import type { Snapshot, WorldTicketResponse } from '../../../world/network/WorldTransport'
import movement from '../../../../../contracts/fixtures/realtime/movement-heartbeat.json'
import ticket from '../../../../../contracts/fixtures/realtime/ticket-response.json'
import '../../../shared/styles/base.css'
import '../../shell.css'

function valuePort<T>(initial: T) {
  let value = initial
  const listeners = new Set<() => void>()
  return { read: () => value, subscribe(fn: () => void) { listeners.add(fn); return () => { listeners.delete(fn) } },
    set(next: T) { value = next; for (const fn of Array.from(listeners)) fn() } }
}
const f = await movementData(), initial = movement.messages[1]!.message as Snapshot
const auth = createSessionController({ eventId: initial.eventId, cookiePolicy: { available: true, onInvalidate: () => () => {}, run: (_k, w) => w(), dispose() {} } })
auth.session.replace(0, { userId: initial.selfUserId, eventId: initial.eventId, accessToken: 'synthetic-memory-only' })
const world = valuePort<WorldAccessBinding | null>(null), connectivity = valuePort<'online' | 'offline' | 'unknown'>('online')
const service = valuePort<ServiceDecision | null>(null), recovery = valuePort<RecoveryAction | null>(null), mode = valuePort('world')
const displayClock = new MockWorldClock(), networkClock = new MockWorldClock(Date.parse(ticket.server_time))
let runtime: WorldRuntime | null = null, replication: WorldReplication | null = null, transport: MockWorldTransport | null = null, stream = 200
let games = 0, peakGames = 0, mounts = 0, disposals = 0, subscriptions = 0, engineVisible = true, retryCalls = 0, issued = 0
const controller = createConnectionObserver({ auth, world, connectivity, service, recovery, scheduler: displayClock, pendingMs: 1000 })
controller.start()
const ports: ShellPorts = { auth, session: auth.session, readiness: { read: () => 'ready', subscribe: () => () => {} },
  bootstrap: { load: async scope => bootstrapFor(scope) }, admin: { authorize: async () => false }, worldAccess: world,
  world: { mount(host, context) {
    mounts++
    const resources = { eventId: initial.eventId, roomId: initial.roomId, mapId: initial.mapId, mapRevision: initial.mapRevision, avatarManifestId: initial.avatarManifestId, avatarManifestVersion: initial.avatarManifestVersion }
    const net = new MockWorldTransport({ session: auth.session, origin: location.origin, scheduler: networkClock,
      acquireTicket: async ({ body }) => ({ ...ticket, ...body, ticket: String.fromCharCode(65 + issued++).repeat(43) }) as WorldTicketResponse })
    transport = net
    const projection = createWorldReplication({ session: auth.session, resources, transport: net, scheduler: networkClock,
      sink: { replace() {} }, requestId: () => `00000000-0000-4000-8000-${String(++stream).padStart(12, '0')}` })
    replication = projection
    const engine = createWorldRuntime({ host,
      load: signal => loadMap({ mapText: JSON.stringify(f.map), themeText: JSON.stringify(f.theme), pin: f.pin,
        resources: { read: async resource => resource.kind === 'png' ? f.png : f.tiledBytes } }, signal),
      importEngine: async () => {
        const factory = await loadPhaserEngine()
        return (element, map) => {
          const lease = factory(element, map); games++; peakGames = Math.max(peakGames, games)
          return { ...lease, setVisible(v) { engineVisible = v; lease.setVisible(v) }, destroy() { games--; lease.destroy() } }
        }
      },
    }); runtime = engine
    const release = { scope: context.scope, privacyScope: auth.privacy.capture(), bootstrap: context.bootstrap, signal: context.signal }
    world.set({ release, replication: { read: projection.read, switchRoom: projection.switchRoom, subscribe(fn) {
      subscriptions++; const stop = projection.subscribe(fn); let stopped = false
      return () => { if (stopped) return; stopped = true; subscriptions--; stop() }
    } } })
    recovery.set({ release, reconnect() { retryCalls++; return projection.reconnect() } })
    void engine.start(context.signal); void projection.start()
    let disposed = false
    return { setInputBlocked: value => engine.setInputBlocked(value), setVisible(value) { engine.setVisible(value); projection.setVisible(value) },
      dispose() { if (disposed) return; disposed = true; disposals++; world.set(null); recovery.set(null); projection.dispose(); engine.dispose() } }
  } },
}
function Panel() {
  const current = useSyncExternalStore(mode.subscribe, mode.read, mode.read)
  if (current === '404') return <RouteState kind="not-found" reload={() => location.reload()} />
  return <><p>Connection help. You can keep using the navigation below.</p><ConnectionStatus controller={controller} reload={() => location.reload()} /></>
}
history.replaceState(null, '', '/world/help')
const router = createBrowserRouter(createAppRoutes(ports, { panels: { '/world/help': <Panel /> } }))
const root = createRoot(document.getElementById('root')!); root.render(<RouterProvider router={router} />)
const style = document.createElement('style'); style.textContent = '#qualification{font:12px/1.3 system-ui;padding:8px;background:#f5eddf;color:#201b19}body{padding:0}.world-shell{height:calc(100dvh - 48px)}'; document.head.append(style)
const fixture = {
  state: () => ({ status: controller.read(), replication: replication?.read().phase, runtime: runtime?.read().phase, games, peakGames, mounts, disposals, subscriptions, engineVisible, retryCalls,
    sockets: transport?.sockets.length ?? 0, activeSockets: transport?.sockets.filter(s => s.readyState < 2).length ?? 0,
    resyncs: transport?.sockets.flatMap(s => s.sent).filter(s => JSON.parse(s).type === 'resync').length ?? 0, canvases: document.querySelectorAll('canvas').length }),
  admit() { const socket = transport!.sockets.at(-1)!; socket.open(); socket.receive(JSON.stringify({ ...initial, streamId: `00000000-0000-4000-8000-${String(++stream).padStart(12, '0')}` })) },
  set(value: string) {
    const release = world.read()!.release
    if (value === 'offline') connectivity.set('offline')
    else if (value === 'online') connectivity.set('online')
    else if (value === 'maintenance' || value === 'version' || value === 'unavailable') service.set({ release, kind: value })
    else if (value === 'normal') service.set({ release, kind: 'normal' })
    else if (value === 'closed') transport!.sockets.at(-1)!.serverClose()
    else if (value === 'timeout') displayClock.advance(1000)
    else if (value === '404') mode.set('404')
    else if (value === 'resync') transport!.sockets.at(-1)!.receive(JSON.stringify({ protocolVersion: '1.0.0', type: 'resyncRequired', streamId: replication!.read().baseline!.streamId, reason: 'GAP' }))
    else if (value === 'map-version') transport!.sockets.at(-1)!.receive(JSON.stringify({ protocolVersion: '1.0.0', type: 'resyncRequired', streamId: replication!.read().baseline!.streamId, reason: 'MAP_VERSION_MISMATCH' }))
  },
  resnapshot() {
    const socket = transport!.sockets.at(-1)!, request = socket.sent.map(s => JSON.parse(s)).reverse().find(s => s.type === 'resync')
    socket.receive(JSON.stringify({ ...initial, streamId: `00000000-0000-4000-8000-${String(++stream).padStart(12, '0')}`, requestId: request.requestId, cause: 'RESYNC' }))
  },
  hidden(value: boolean) { Object.defineProperty(document, 'hidden', { configurable: true, value }); document.dispatchEvent(new Event('visibilitychange')) },
  navigate: (path: string) => router.navigate(path),
  async dispose() { root.unmount(); router.dispose(); controller.dispose(); auth.dispose(); await runtime?.settled(); style.remove() },
}
Object.assign(window, { __DEGRADED__: fixture })
declare global { interface Window { __DEGRADED__: typeof fixture } }
