import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { createHttpClient, createMemorySession } from '../../../../shared/api/http'
import { createPlacesController } from '../../model/controller'
import type { SceneryOverview } from '../../../../world/illustrated/overview'
import { catalog, eventId, otherUser, release, userId } from '../fixtures/data'

beforeEach(() => vi.stubGlobal('location', { origin: 'https://mcr.example.invalid' }))
afterEach(() => vi.unstubAllGlobals())
async function setup() {
  const session = createMemorySession(); session.replace(0, { userId, eventId, accessToken: 'memory-test-token' })
  const map = await release(), listeners = new Set<() => void>(), g = map.manifest.geometry
  const pins: SceneryOverview = { imageUrl: 'blob:test-1', width: g.width_cells * g.tile_width_px, height: g.height_cells * g.tile_height_px,
    mapId: map.expected.mapId, mapRevision: map.expected.revision, mapHash: map.expected.contentHash, geometryHash: map.expected.geometryHash,
    themeId: 'REVENGE', themeRevision: 1, themeHash: 'sha256:verified-theme' }
  let image: SceneryOverview | null = null, currentCatalog = catalog()
  const lookAt = vi.fn()
  const controller = createPlacesController({ session, http: createHttpClient({ session, fetch: async () => Response.json(currentCatalog) }),
    map: { read: () => map, subscribe: () => () => {} },
    overview: { read: () => image, subscribe(listener) { listeners.add(listener); return () => { listeners.delete(listener) } } },
    camera: { read: () => ({ lookAt, recenter() {}, setZoom: () => true, read: () => ({ following: true, camera: null }) }) } })
  controller.start(); await vi.waitFor(() => expect(controller.read().list.phase).toBe('ready'))
  return { controller, session, pins, lookAt, listeners, catalog: (value: typeof currentCatalog) => { currentCatalog = value },
    publish(value: SceneryOverview | null) { image = value; [...listeners].forEach(f => f()) } }
}
it('uses schematic without optional resources, accepts exact pins, updates theme and withdraws synchronously', async () => {
  const f = await setup(); expect(f.controller.read().overview).toBeNull()
  f.publish(f.pins); expect(f.controller.read().overview).toEqual(f.pins)
  f.publish(null); expect(f.controller.read().overview).toBeNull()
  const next = { ...f.pins, imageUrl: 'blob:test-2', themeId: 'PARADE', themeRevision: 2, themeHash: 'sha256:next-verified' }
  f.publish(next); expect(f.controller.read().overview).toEqual(next)
  f.controller.dispose(); expect(f.listeners.size).toBe(0); expect(f.controller.read().overview).toBeNull()
})
it.each(['mapId', 'mapHash', 'geometryHash', 'mapRevision', 'width', 'height', 'imageUrl'] as const)('rejects mismatched overview %s without changing map or authorized camera targets', async field => {
  const f = await setup(), value = { ...f.pins, [field]: typeof f.pins[field] === 'number' ? 999 : 'https://untrusted.invalid/map.png' }
  f.publish(value); expect(f.controller.read().overview).toBeNull()
  expect(f.controller.showOnMap(catalog().items[0]!.place_id)).toBe(true); expect(f.lookAt).toHaveBeenCalledWith({ x: 48, y: 48 }); f.controller.dispose()
})
it('current catalog closure/removal governs illustrated targets; catalog mismatch hides overview', async () => {
  const f = await setup(); f.publish(f.pins)
  const list = catalog(); list.items[0]!.closure_state = 'CLOSED'; f.catalog(list); await f.controller.refresh()
  expect(f.controller.canShow(list.items[0]!.place_id)).toBe(false); expect(f.controller.read().overview).toEqual(f.pins)
  list.items.shift(); f.catalog(list); await f.controller.refresh(); expect(f.controller.read().list.data?.items).toHaveLength(4)
  list.items[0]!.label = 'Changed target'; f.catalog(list); await f.controller.refresh()
  expect(f.controller.read().mapMismatch).toBe(true); expect(f.controller.read().overview).toBeNull(); f.controller.dispose()
})
it('evicts image on account/event scope change; late notifications cannot restore it', async () => {
  const f = await setup(); f.publish(f.pins)
  f.session.replace(f.session.read().generation, { userId: otherUser, eventId, accessToken: 'new-memory-token' })
  expect(f.controller.read().overview).toBeNull(); f.publish(f.pins); expect(f.controller.read().overview).toBeNull()
  await vi.waitFor(() => expect(f.controller.read().list.phase).toBe('ready')); expect(f.controller.read().mapMismatch).toBe(true); f.controller.dispose()
})
