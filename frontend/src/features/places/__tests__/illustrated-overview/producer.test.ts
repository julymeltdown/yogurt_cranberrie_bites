// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { createSceneryOverview } from '../../../../world/illustrated/overview'
import type { LoadedMap } from '../../../../world/map/resources'
import map from '../../../../../../contracts/fixtures/assets/map-manifest.json'
import theme from '../../../../../../contracts/fixtures/assets/theme-revenge.json'

afterEach(() => vi.restoreAllMocks())
function setup() {
  const draw = vi.fn(), jobs: BlobCallback[] = [], canvases: HTMLCanvasElement[] = []
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(function(this: HTMLCanvasElement) {
    canvases.push(this); return { drawImage: draw } as unknown as CanvasRenderingContext2D
  })
  vi.spyOn(HTMLCanvasElement.prototype, 'toBlob').mockImplementation(cb => { jobs.push(cb) })
  let serial = 0
  const create = vi.fn(() => `blob:overview-${++serial}`), revoke = vi.fn()
  vi.stubGlobal('URL', Object.assign(class {}, { createObjectURL: create, revokeObjectURL: revoke }))
  const texture = (id: string) => ({ id, path: `world/test/${id}.png`, sha256: 'sha256:test', width_px: 160, height_px: 160, bytes: 10 })
  const ground = texture('ground'), atlas = texture('atlas')
  const scenery = { version: 1 as const, ground, atlases: [atlas], objects: [90, -1, 30].map((depth_y, i) => ({ id: `object${i}`, atlas_id: 'atlas', frame: { x: i, y: 0, width: 1, height: 1 }, x: i, y: 0, width: 1, height: 1, depth_y })) }
  const loaded = { spawn: map.geometry.spawns[0]!, image: {} as HTMLImageElement, dispose() {}, map, theme: { ...theme, scenery }, widthPx: 160, heightPx: 160,
    scenery: { manifest: scenery, assets: [ground, atlas].map(descriptor => ({ descriptor, image: { naturalWidth: 160, naturalHeight: 160 } as HTMLImageElement })), transferBytes: 20, decodedBytes: 204800 } } as LoadedMap
  return { owner: createSceneryOverview(), loaded, draw, jobs, canvases, create, revoke }
}
afterEach(() => vi.unstubAllGlobals())
const png = () => new Blob(['test encoding'], { type: 'image/png' })
it('projects only ground and stable depth-sorted individual scenery, pinned to selected map/theme', () => {
  const f = setup(); f.owner.replace(f.loaded)
  expect(f.draw.mock.calls.map(c => c[1])).toEqual([0, 1, 2, 0])
  expect(f.owner.read()).toBeNull(); f.jobs[0]!(png())
  expect(f.owner.read()).toMatchObject({ imageUrl: 'blob:overview-1', mapId: map.map_id, mapHash: map.content_hash, geometryHash: map.geometry_hash, themeId: 'REVENGE', themeRevision: 1, themeHash: theme.content_hash })
  expect(f.canvases[0]!.width).toBe(0)
  f.owner.dispose(); expect(f.revoke).toHaveBeenCalledWith('blob:overview-1')
})
it('withdraws synchronously before revocation; fences late PNGs after stop/disposal', () => {
  const f = setup(); f.owner.replace(f.loaded); f.jobs[0]!(png())
  const observed: unknown[] = []; f.owner.subscribe(() => observed.push([f.owner.read(), f.revoke.mock.calls.length]))
  f.owner.replace(f.loaded)
  expect(observed[0]).toEqual([null, 0]); expect(f.revoke).toHaveBeenCalledOnce()
  f.owner.dispose(); f.jobs[1]!(png()); expect(f.create).toHaveBeenCalledTimes(1); expect(f.owner.read()).toBeNull()
})
it('bounds rapid replacements to one encoder and latest pending raster; drops obsolete themes', () => {
  const f = setup(); f.owner.replace(f.loaded)
  for (let i = 0; i < 8; i++) f.owner.replace({ ...f.loaded, theme: { ...f.loaded.theme, theme_revision: i + 2 } })
  expect(f.jobs).toHaveLength(1); expect(f.canvases.filter(c => c.width > 0)).toHaveLength(1)
  f.jobs[0]!(png()); expect(f.jobs).toHaveLength(2); expect(f.create).not.toHaveBeenCalled()
  f.jobs[1]!(png()); expect(f.owner.read()?.themeRevision).toBe(9); expect(f.create).toHaveBeenCalledOnce(); f.owner.dispose()
})
it.each([null, new Blob(['wrong'], { type: 'text/plain' }), new Blob([new Uint8Array(2_097_153)], { type: 'image/png' })])('retains fallback for missing/invalid/oversize encoding %s', blob => {
  const f = setup(); f.owner.replace(f.loaded); f.jobs[0]!(blob); expect(f.owner.read()).toBeNull(); expect(f.create).not.toHaveBeenCalled(); f.owner.dispose()
})
it('rejects incomplete decoded scenery without fetching or exposing its images', () => {
  const f = setup(); f.owner.replace({ ...f.loaded, scenery: { ...f.loaded.scenery!, assets: [] } })
  expect(f.draw).not.toHaveBeenCalled(); expect(f.jobs).toHaveLength(0); expect(f.owner.read()).toBeNull(); f.owner.dispose()
})
