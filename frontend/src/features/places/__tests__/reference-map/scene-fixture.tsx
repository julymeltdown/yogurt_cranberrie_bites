// TEST ONLY. Candidate geometry and projection are coherent; production admission
// is tested separately. The fixture host is instrumented, NOT Phaser/live C05.
import { createRoot } from 'react-dom/client'
import { createMemoryRouter, RouterProvider, useParams } from 'react-router-dom'
import { createAppRoutes } from '../../../../app/router'
import { createFixturePorts } from '../../../../app/__tests__/fixtures/ports'
import { createHttpClient } from '../../../../shared/api/http'
import { createPlacesController, type PlaceMapRelease } from '../../model/controller'
import { PlacesOverview, PlaceDetail } from '../../ui/Places'
import { createReferenceSceneOverview, loadReferenceScene } from '../../../../world/reference-scene'
import { referenceFixture, referenceFixtureResources } from '../../../../world/reference-scene/__tests__/fixture'
import '../../../../shared/styles/base.css'
import '../../../../app/shell.css'

const f = await referenceFixture(), requests: string[] = []
const host = createRoot(document.getElementById('root')!)

  const fixture = createFixturePorts(), signal = new AbortController()
  const files = referenceFixtureResources(f, location.origin)
  const loaded = await loadReferenceScene({ mapText: JSON.stringify(f.map), themeText: JSON.stringify(f.theme), pin: f.pin, resources: files.resources }, signal.signal)
  const overview = createReferenceSceneOverview(), listeners = new Set<() => void>()
  let mode = 'valid', config = 'valid', writes = 0, disposed = false
  const scope = fixture.ports.session.read(), release: PlaceMapRelease = { scope: { generation: scope.generation, userId: scope.userId, eventId: scope.eventId },
    manifest: f.map, expected: { mapId: f.map.map_id, revision: f.map.map_revision, contentHash: f.map.content_hash, geometryHash: f.map.geometry_hash } }
  const cameraState = Object.freeze({ following: true, camera: null })
  const controller = createPlacesController({ session: fixture.ports.session,
    map: { read: () => release, subscribe: () => () => {} }, camera: { read: () => ({ read: () => cameraState, setZoom() { writes++; return true }, recenter() { writes++ }, lookAt() { writes++ } }) },
    http: createHttpClient({ session: fixture.ports.session, fetch: async input => {
      const path = new URL((input as Request).url).pathname; requests.push(path)
      const items = f.map.places.map((p, i) => mode === 'mismatch' && i === 0 ? { ...p, label: 'TEST ONLY catalog mismatch' }
        : mode === 'closed' && i === 0 ? { ...p, closure_state: 'CLOSED' } : p).filter((_, i) => mode !== 'removed' || i !== 0)
      return Response.json(path.endsWith('/places') ? { event_id: scope.eventId, revision: 1, items }
        : { event_id: scope.eventId, revision: 1, place: items.find(p => p.place_id === path.split('/').at(-1)) })
    } }), overview: { read() {
      const value = overview.read()
      return value && config !== 'valid' ? { ...value, geometryHash: 'sha256:test-only-mismatch' } : value
    }, subscribe(listener) { listeners.add(listener); return () => { listeners.delete(listener) } } },
  })
  const stop = overview.subscribe(() => listeners.forEach(listener => listener()))
  overview.replace(loaded)
  function SceneDetail() { const { placeId } = useParams(); return <PlaceDetail controller={controller} placeId={placeId!} /> }
  const router = createMemoryRouter(createAppRoutes(fixture.ports, { panels: {
    '/world/map': <PlacesOverview controller={controller} />, '/world/places/:placeId': <SceneDetail />,
  } }), { initialEntries: ['/world/map'] })
  host.render(<RouterProvider router={router} />)
  function withdraw() { overview.dispose(); signal.abort(); loaded.dispose(); files.dispose() }
  Object.assign(window, { __SCENE_MAP__: {
    state: () => ({ production: false, path: router.state.location.pathname, candidate: f.map.map_id, geometryHash: f.map.geometry_hash,
      overview: overview.read(), requests: [...requests], writes, camera: cameraState, stub: { ...fixture.stats }, disposed }),
    navigate: (path: string) => router.navigate(path), mode(value: string) { mode = value }, refresh: () => controller.refresh(), projection: () => overview.replace(loaded),
    config(value: string) { config = value; listeners.forEach(listener => listener()) },
    account() { withdraw(); fixture.replace(); controller.dispose() },
    dispose() { if (disposed) return; disposed = true; host.unmount(); router.dispose(); controller.dispose(); stop(); withdraw(); listeners.clear() },
  } })
