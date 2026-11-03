import { vi } from 'vitest'
import { createSessionController } from '../../../features/auth/model/session'
import { bootstrapFor, createFixturePorts } from '../../__tests__/fixtures/ports'
import type { ShellPorts } from '../../ports'
import type { PlaceMapRelease, PlaceCameraPort } from '../../../features/places'
import { catalog, eventId, places, release, userId } from '../../../features/places/__tests__/fixtures/data'
import type { PlacesEntryRelease } from '../owner'

/** Existing synthetic C03/C06 fixtures through actual canonical Auth HTTP. */
export async function entryFixture(handler?: (request: Request) => Response | Promise<Response>) {
  const requests: Request[] = []
  let mode: 'ok' | 'mismatch' | 'removed' = 'ok'
  const auth = createSessionController({ eventId,
    cookiePolicy: { available: true, onInvalidate: () => () => {}, run: (_kind, work) => work(), dispose() {} },
    fetch: async input => {
      const request = input as Request; requests.push(request)
      if (handler) return handler(request)
      const id = new URL(request.url).pathname.split('/').at(-1)
      if (mode === 'mismatch') return Response.json({ code: 'FIXTURE', message: 'Never rendered', details: { reason: 'MAP_VERSION_MISMATCH' } }, { status: 409 })
      if (id !== 'places' && mode === 'removed') return Response.json({ code: 'FIXTURE', message: 'Never rendered', details: { reason: 'RESOURCE_UNAVAILABLE' } }, { status: 404 })
      return Response.json(id === 'places' ? catalog() : { event_id: eventId, revision: 7, place: places().find(place => place.place_id === id) })
    } })
  auth.session.replace(0, { userId, eventId, accessToken: 'synthetic-memory-token' })
  let map: PlaceMapRelease | null = await release()
  const camera = { lookAt: vi.fn(), recenter: vi.fn(), setZoom: vi.fn(), read: () => ({ following: true, camera: { zoom: 2, scrollX: 0, scrollY: 0, width: 390, height: 400 } }) }
  let currentCamera: ReturnType<PlaceCameraPort['read']> = camera
  const maps = new Set<() => void>(), sources = new Set<() => void>()
  const mapPort = { read: () => map, subscribe(fn: () => void) { maps.add(fn); return () => { maps.delete(fn) } } }
  const lease = new AbortController()
  let binding: PlacesEntryRelease | null = { scope: auth.privacy.capture(), signal: lease.signal, map: mapPort, camera: { read: () => currentCamera } }
  const source = { read: () => binding, subscribe(fn: () => void) { sources.add(fn); return () => { sources.delete(fn) } } }
  const world = createFixturePorts()
  const ports: ShellPorts = { ...world.ports, session: auth.session, auth, bootstrap: { load: async scope => bootstrapFor(scope) } }
  return { auth, source, lease, ports, stats: world.stats, camera, mapPort, requests,
    setMode(value: typeof mode) { mode = value },
    setCamera(value: typeof currentCamera) { currentCamera = value },
    setMap(value: PlaceMapRelease | null) { map = value; [...maps].forEach(fn => fn()) },
    publish(value: PlacesEntryRelease | null) { binding = value; [...sources].forEach(fn => fn()) },
    subscriptions: () => ({ maps: maps.size, sources: sources.size }),
    dispose() { lease.abort(); auth.dispose() },
  }
}
