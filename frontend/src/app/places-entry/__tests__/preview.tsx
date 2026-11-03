import { createRoot } from 'react-dom/client'
import { createBrowserRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../../../app/router'
import type { ShellPorts } from '../../../app/ports'
import { bootstrapFor } from '../../../app/__tests__/fixtures/ports'
import { createSessionController } from '../../../features/auth/model/session'
import { PlacesEntryProvider, PlacesOverviewRoute, PlaceDetailRoute, usePlacesEntry, type PlacesEntryRelease } from '../index'
import type { PlacesController } from '../../../features/places'
import { createWorldRuntime, type WorldRuntime } from '../../../world/runtime'
import { attachMovement, type MovementAttachment } from '../../../world/input/controller'
import { loadMap } from '../../../world/map/resources'
import { movementData } from '../../../world/__tests__/fixtures/movement-data'
import { rehash } from '../../../world/__tests__/fixtures/synthetic'
import { WorldHelp } from '../../../features/world-help/WorldHelp'
import type { PlaceMapRelease } from '../../../features/places'
import { catalog, eventId, otherUser, places, userId } from '../../../features/places/__tests__/fixtures/data'
import '../../../shared/styles/base.css'
import '../../../app/shell.css'

const f = await movementData(), metadata = places()
f.map.places = metadata
f.map.geometry.interactions = metadata.map((place, i) => ({ interaction_id: `inspect_${i}`, place_id: place.place_id, cell_x: [4, 6, 3, 4, 7][i]!, cell_y: [3, 10, 6, 7, 8][i]! }))
f.theme.place_ids = metadata.map(place => place.place_id); f.theme.interaction_ids = f.map.geometry.interactions.map(p => p.interaction_id)
const pin = await rehash(f.map, f.theme)
let mode = 'ok', map: PlaceMapRelease | null = null, movement: MovementAttachment | null = null, runtime: WorldRuntime | null = null
let mounts = 0, disposals = 0, controls: HTMLDivElement | null = null, disposed = false
const mapListeners = new Set<() => void>(), requests: string[] = []
const documentId = crypto.randomUUID()
let binding: PlacesEntryRelease | null = null, controller: PlacesController | null = null
let ownerCount = 0, lastController: PlacesController | null = null
const sourceListeners = new Set<() => void>()
const source = { read: () => binding, subscribe(fn: () => void) { sourceListeners.add(fn); return () => { sourceListeners.delete(fn) } } }
const mapPort = { read: () => map, subscribe(fn: () => void) { mapListeners.add(fn); return () => { mapListeners.delete(fn) } } }
function publishMap() { for (const listener of Array.from(mapListeners)) listener(); for (const listener of Array.from(sourceListeners)) listener() }
function Probe() {
  controller = usePlacesEntry()
  if (controller && controller !== lastController) { lastController = controller; ownerCount++ }
  return null
}
const auth = createSessionController({ eventId, cookiePolicy: { available: true, onInvalidate: () => () => {}, run: (_kind, work) => work(), dispose() {} }, fetch: async input => {
  const request = input as Request, path = new URL(request.url).pathname; requests.push(path)
  const envelope = (status: number, reason: string) => Response.json({ code: 'SYNTHETIC', message: 'Not rendered', details: { reason } }, { status })
  if (mode === 'error') return envelope(503, 'DEPENDENCY_UNAVAILABLE')
  if (mode === 'map-error') return envelope(409, 'MAP_VERSION_MISMATCH')
  if (mode === 'rate') return envelope(429, 'RATE_LIMITED')
  if (mode === 'denied') return envelope(403, 'PERMISSION_DENIED')
  const id = path.split('/').at(-1), data = catalog(mode === 'empty' ? [] : metadata)
  if (id === 'places') return Response.json(data)
  if (mode === 'detail-map-error') return envelope(409, 'MAP_VERSION_MISMATCH')
  if (mode === 'removed') return envelope(404, 'RESOURCE_UNAVAILABLE')
  return Response.json({ event_id: eventId, revision: mode === 'conflict' ? 8 : 7, place: metadata.find(place => place.place_id === id) })
} })
const session = auth.session
session.replace(0, { userId, eventId, accessToken: 'synthetic-memory-only-token' })
const ports: ShellPorts = { session, auth, readiness: { read: () => 'ready', subscribe: () => () => {} },
  bootstrap: { load: async scope => ({ ...bootstrapFor(scope), event: { eventId: scope.eventId, title: 'FE19 synthetic event', phase: 'OPEN' } }) }, admin: { authorize: async () => false },
  world: { mount(host, context) {
    mounts++
    const mountScope = context.scope, mountSignal = context.signal
    controls = document.createElement('div'); controls.style.cssText = 'position:absolute;bottom:0;left:0;right:0'; host.append(controls)
    const owner = createWorldRuntime({ host,
      load: signal => loadMap({ mapText: JSON.stringify(f.map), themeText: JSON.stringify(f.theme), pin,
        resources: { read: async resource => resource.kind === 'png' ? f.png : f.tiledBytes } }, signal),
      attachControls(context) {
        const attached = attachMovement(context, { controls: controls!, zoom: 1,
          policy: { speedPxPerSecond: 120, footWidthPx: 8, footHeightPx: 6, interactionRadiusPx: 42 }, consume() {} })
        movement = attached
        map = { scope: mountScope,
          manifest: context.map.map, expected: { mapId: f.map.map_id, revision: f.map.map_revision, contentHash: pin.mapHash, geometryHash: f.map.geometry_hash } }
        binding = { scope: auth.privacy.capture(), signal: mountSignal, map: mapPort, camera: { read: () => movement?.camera ?? null } }
        publishMap()
        return { setEnabled: value => attached.setEnabled(value), dispose() { attached.dispose(); if (movement === attached) movement = null } }
      },
    }); runtime = owner; void owner.start(context.signal)
    return { setInputBlocked: value => owner.setInputBlocked(value), setVisible: value => owner.setVisible(value),
      dispose() { disposals++; owner.dispose(); controls?.remove(); controls = null; map = null; binding = null; publishMap() } }
  } },
}
history.replaceState(null, '', new URLSearchParams(location.search).has('detail') ? `/world/places/${metadata[1]!.place_id}` : '/world/map')
const router = createBrowserRouter(createAppRoutes(ports, { panels: { '/world/map': <PlacesOverviewRoute />,
  '/world/places/:placeId': <PlaceDetailRoute />, '/world/help': <WorldHelp /> } }))
const root = createRoot(document.getElementById('root')!); root.render(<PlacesEntryProvider auth={auth} source={source}><Probe /><RouterProvider router={router} /></PlacesEntryProvider>)
const style = document.createElement('style'); style.textContent = '#qualification{font:12px/1.4 system-ui;padding:8px;background:#faf6ed;position:relative;z-index:3}body{padding:0}.world-shell{height:calc(100dvh - 58px)}'; document.head.append(style)
window.__PLACES_ENTRY__ = {
  state: () => ({ documentId, places: controller?.read() ?? null, runtime: runtime?.read(), camera: movement?.camera.read(), foot: movement?.read().foot,
    mounts, disposals, ownerCount, requests, canvases: document.querySelectorAll('canvas').length, controls: document.querySelectorAll('.world-controls').length }),
  mode(value) { mode = value; if (value === 'unavailable') binding = null; publishMap(); if (value !== 'unavailable') void controller?.refresh() },
  replace() { session.replace(session.read().generation, { userId: otherUser, eventId, accessToken: 'replacement-fixture-token' }) },
  async dispose() { if (disposed) return; disposed = true; root.unmount(); router.dispose(); auth.dispose(); runtime?.dispose(); await runtime?.settled(); style.remove() },
}
declare global { interface Window { __PLACES_ENTRY__: {
  state(): { documentId: string; places: ReturnType<PlacesController['read']> | null; runtime: ReturnType<WorldRuntime['read']> | undefined; camera: ReturnType<MovementAttachment['camera']['read']> | undefined;
    foot: ReturnType<MovementAttachment['read']>['foot'] | undefined; mounts: number; disposals: number; ownerCount: number; requests: string[]; canvases: number; controls: number }
  mode(value: string): void; replace(): void; dispose(): Promise<void>
} } }
