// TEST ONLY: module composition, not a replacement/default WorldShell.
import { useLayoutEffect, useRef, useState, useSyncExternalStore } from 'react'
import { createRoot } from 'react-dom/client'
import { createBrowserRouter, Outlet, RouterProvider, useLocation } from 'react-router-dom'
import { NavigationMemory, ShellLink, usePanelBack } from '../../navigation'
import { panelRoutes, resolvePanel } from '../../routes'
import { ShellHeader, EraControls, BottomNavigation, PanelHeader, type EraPort } from '../index'
import { createThemeController, type ThemeController } from '../../../features/theme/model/controller'
import { createLocalThemePreferences, themeIds, type ThemeId } from '../../../features/theme/model/preferences'
import { createSessionController } from '../../../features/auth/model/session'
import { createThemeLoader, type ThemeRelease } from '../../../world/theme/loader'
import { syntheticRelease } from '../../../world/theme/__tests__/fixtures/releases'
import { createWorldRuntime, type WorldRuntime } from '../../../world/runtime'
import { loadPhaserEngine } from '../../../world/scenes/MapScene'
import type { WorldCamera } from '../../../world/camera/port'
import { MapError } from '../../../world/map/json'
import { MockWorldClock, MockWorldTransport } from '../../../world/network/mock'
import { createWorldReplication, type WorldReplication } from '../../../world/network/replication'
import type { Snapshot, WorldTicketResponse } from '../../../world/network/WorldTransport'
import movement from '../../../../../contracts/fixtures/realtime/movement-heartbeat.json'
import ticket from '../../../../../contracts/fixtures/realtime/ticket-response.json'
import '../../../shared/styles/base.css'
import './preview.css'

const releases = {} as Record<ThemeId, ThemeRelease>
for (const id of themeIds) {
  const f = await syntheticRelease(id)
  releases[id] = { mapText: JSON.stringify(f.map), themeText: JSON.stringify(f.theme), pin: f.pin,
    resources: { read: async resource => resource.kind === 'png' ? f.png : f.tiledBytes } }
}
const load = createThemeLoader(releases), initial = movement.messages[1]!.message as Snapshot
const documentId = crypto.randomUUID(), clock = new MockWorldClock(Date.parse(ticket.server_time))
const auth = createSessionController({ eventId: initial.eventId, cookiePolicy: { available: true, onInvalidate: () => () => {}, run: (_k, work) => work(), dispose() {} } })
auth.session.replace(0, { userId: initial.selfUserId, eventId: initial.eventId, accessToken: 'synthetic-memory-only' })
let publication: EraPort | null = null
const listeners = new Set<() => void>()
const publish = (port: EraPort | null) => { publication = port; for (const fn of Array.from(listeners)) fn() }
const source = { read: () => publication, subscribe(fn: () => void) { listeners.add(fn); return () => { listeners.delete(fn) } } }
let runtime: WorldRuntime | null = null, controller: ThemeController | null = null, replication: WorldReplication | null = null, transport: MockWorldTransport | null = null
let camera: WorldCamera | null = null, originalCamera: WorldCamera | null = null
let mounts = 0, games = 0, peakGames = 0, disposals = 0, themeSubscriptions = 0, mode = 'valid', writes = 0
const releaseGates = new Set<() => void>()
function FixtureWorld() {
  const host = useRef<HTMLDivElement>(null)
  useLayoutEffect(() => {
    const lifetime = new AbortController(), preferences = createLocalThemePreferences(localStorage)
    const net = new MockWorldTransport({ session: auth.session, origin: location.origin, scheduler: clock,
      acquireTicket: async ({ body }) => ({ ...ticket, ...body }) as WorldTicketResponse })
    transport = net
    const projection = createWorldReplication({ session: auth.session, transport: net, scheduler: clock,
      resources: { eventId: initial.eventId, roomId: initial.roomId, mapId: initial.mapId, mapRevision: initial.mapRevision, avatarManifestId: initial.avatarManifestId, avatarManifestVersion: initial.avatarManifestVersion }, sink: { replace() {} } })
    replication = projection
    const owner = createWorldRuntime({ host: host.current!, load: signal => load('REVENGE', signal),
      importEngine: async () => {
        const factory = await loadPhaserEngine()
        return (element, map) => {
          const engine = factory(element, map); mounts++; games++; peakGames = Math.max(games, peakGames)
          return { ...engine, destroy() { games--; disposals++; engine.destroy() } }
        }
      },
      attachControls(context) { camera = originalCamera = context.camera; context.camera.center({ x: 112, y: 112 }, 2)
        return { setEnabled() {}, dispose() { camera = null } }
      },
    }); runtime = owner
    const choice = createThemeController({ runtime: owner, initial: 'REVENGE', scope: { userId: initial.selfUserId, eventId: initial.eventId },
      preferences: { read: preferences.read, write(scope, theme) { preferences.write(scope, theme); writes++ } },
      async load(id, signal) {
        if (mode === 'fail') throw new MapError('load-failed')
        const candidate = await load(id, signal)
        if (mode === 'pending') await new Promise<void>(resolve => { releaseGates.add(resolve) })
        return candidate
      },
    }); controller = choice
    const port: EraPort = { ...choice, subscribe(fn) { themeSubscriptions++; const stop = choice.subscribe(fn); return () => { themeSubscriptions--; stop() } } }
    void owner.start(lifetime.signal).then(() => { if (!lifetime.signal.aborted) publish(port) })
    void projection.start()
    return () => { lifetime.abort(); publish(null); choice.dispose(); projection.dispose(); owner.dispose() }
  }, [])
  return <div className="chrome-fixture-world"><div ref={host} className="chrome-fixture-host" />
    <p className="chrome-fixture-art">Avatar artwork unavailable</p>
    <nav className="chrome-fixture-tools" aria-label="World tools"><ShellLink to="/world/map" id="world-map">Map</ShellLink><ShellLink to="/world/rooms" id="world-rooms">Rooms</ShellLink><ShellLink to="/world/help" id="world-help">Help</ShellLink></nav>
  </div>
}
function Frame() {
  const port = useSyncExternalStore(source.subscribe, source.read, source.read)
  return <NavigationMemory><div className="chrome-fixture">
    <aside className="chrome-fixture-qualification">MODULE FIXTURE · synthetic terrain / mock connection. No approved avatar art or live server.</aside>
    <ShellHeader eventTitle="Synthetic event" /><EraControls controller={port} />
    <div className="chrome-fixture-stage"><FixtureWorld /><Outlet /></div>
    <BottomNavigation />
  </div></NavigationMemory>
}
const gift = '00000000-0000-4000-8000-000000000001'
function Panel() {
  const location = useLocation(), resolved = resolvePanel(location.pathname), back = usePanelBack(), heading = useRef<HTMLHeadingElement>(null)
  const [size, setSize] = useState<'peek' | 'half' | 'full'>('half')
  useLayoutEffect(() => { heading.current?.focus({ preventScroll: true }) }, [location.key])
  if (location.pathname === '/world') return null
  return <section className="chrome-fixture-panel" aria-labelledby="panel-title" data-size={size}>
    <PanelHeader title={resolved?.route.title ?? 'Unavailable'} headingRef={heading} size={size} back={back}
      toggleSize={() => setSize(size === 'full' ? 'half' : 'full')} toggleContent={() => setSize(size === 'peek' ? 'half' : 'peek')} />
    <div className="chrome-fixture-panel__body" hidden={size === 'peek'}>
      <p>Module composition. This panel's feature content is not loaded.</p>
      {location.pathname === '/gifts' && <ShellLink to={`/gifts/${gift}`} id="fixture-gift">Open fixture giveaway</ShellLink>}
      <label>Fixture draft<input aria-label="Fixture draft" /></label>
    </div>
  </section>
}
const start = new URLSearchParams(location.search).get('start')
history.replaceState(null, '', start === 'detail' ? `/gifts/${gift}` : '/world')
const router = createBrowserRouter([{ element: <Frame />, children: panelRoutes.map(route => ({ path: route.path, element: <Panel /> })) }])
const root = createRoot(document.getElementById('root')!); root.render(<RouterProvider router={router} />)
const fixture = {
  state: () => ({ documentId, phase: runtime?.read().phase, theme: controller?.read(), published: !!publication, skin: runtime?.readSkin(), camera: camera?.read(), sameCamera: camera === originalCamera,
    mounts, games, peakGames, disposals, themeSubscriptions, writes, gates: releaseGates.size, sockets: transport?.sockets.length ?? 0, activeSockets: transport?.sockets.filter(s => s.readyState < 2).length ?? 0,
    room: replication?.read().baseline?.roomId, outfit: replication?.read().entities.find(e => e.player.userId === initial.selfUserId)?.player.avatar,
    canvases: document.querySelectorAll('canvas').length }),
  admit() { const socket = transport!.sockets.at(-1)!; socket.open(); socket.receive(JSON.stringify(initial)) },
  mode(value: string) { mode = value },
  release() { const gates = Array.from(releaseGates); releaseGates.clear(); gates.forEach(resolve => resolve()) },
  withdraw() { publish(null); controller?.dispose() },
  navigate: (path: string) => router.navigate(path),
  async dispose() { root.unmount(); router.dispose(); auth.dispose(); await runtime?.settled() },
}
Object.assign(window, { __CHROME__: fixture })
declare global { interface Window { __CHROME__: typeof fixture } }
