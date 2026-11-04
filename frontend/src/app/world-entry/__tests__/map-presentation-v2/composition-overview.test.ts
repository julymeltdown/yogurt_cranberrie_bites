// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { loadedMap4 } from '../map-presentation-v1/fixture'
import { requireReferenceScene } from '../../../../world/reference-scene/model'
import { createReferenceSceneOverview, drawReferenceOverview, type ReferenceMapCompositionPaint } from '../../../../world/reference-scene/overview'

afterEach(() => { vi.restoreAllMocks(); vi.unstubAllGlobals() })
function rasterHarness() {
  const order: string[] = [], encoders: BlobCallback[] = []
  const context = { drawImage: vi.fn(() => order.push('scene')) } as unknown as CanvasRenderingContext2D
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(() => context)
  vi.spyOn(HTMLCanvasElement.prototype, 'toBlob').mockImplementation(callback => { encoders.push(callback) })
  const create = vi.fn(() => 'blob:composition-test'), revoke = vi.fn()
  vi.stubGlobal('URL', class extends URL { static createObjectURL = create; static revokeObjectURL = revoke })
  return { order, encoders, context, create, revoke }
}

it('draws a qualified full Map scene once without incumbent plaza or prop overlays', async () => {
  const map = await loadedMap4('PARADE'), binding = requireReferenceScene(map)
  const scene = { naturalWidth: 768, naturalHeight: 1296 } as HTMLImageElement
  const context = { drawImage: vi.fn(), beginPath: vi.fn(), save: vi.fn(), restore: vi.fn(),
    fill: vi.fn(), stroke: vi.fn(), imageSmoothingEnabled: true } as unknown as CanvasRenderingContext2D
  const paint: ReferenceMapCompositionPaint = { mode: 'full-composition', scene,
    presentationVersion: 'map4-reference-composition-v1', presentationHash: 'sha256:' + '1'.repeat(64), dispose: vi.fn() }
  const before = JSON.stringify(map.map.geometry)
  drawReferenceOverview(context, binding, paint)
  expect(context.drawImage).toHaveBeenCalledExactlyOnceWith(scene, 0, 0)
  expect(context.beginPath).not.toHaveBeenCalled()
  expect(context.fill).not.toHaveBeenCalled()
  expect(context.stroke).not.toHaveBeenCalled()
  expect(context.imageSmoothingEnabled).toBe(false)
  expect(JSON.stringify(map.map.geometry)).toBe(before)
})

it('keeps original zero-option Map projection draw order and all original props', async () => {
  const binding = requireReferenceScene(await loadedMap4('DANGER'))
  const context = { drawImage: vi.fn(), beginPath: vi.fn(), save: vi.fn(), restore: vi.fn(), moveTo: vi.fn(),
    lineTo: vi.fn(), closePath: vi.fn(), fill: vi.fn(), stroke: vi.fn() } as unknown as CanvasRenderingContext2D
  drawReferenceOverview(context, binding)
  expect(context.drawImage).toHaveBeenCalledTimes(binding.overview.length + 1)
  expect(context.fill).toHaveBeenCalledOnce()
  expect(context.stroke).toHaveBeenCalledOnce()
  expect(context.drawImage).toHaveBeenNthCalledWith(1, binding.loaded.assets.find(a => a.descriptor.id === binding.loaded.manifest.ground.id)!.image, 0, 0)
})

it('publishes the complete-scene identity only after scene plus labels and PNG encoding finish together', async () => {
  const h = rasterHarness(), map = await loadedMap4()
  let finish!: (value: ReferenceMapCompositionPaint) => void
  const provider = vi.fn(() => new Promise<ReferenceMapCompositionPaint>(resolve => { finish = resolve }))
  const owner = createReferenceSceneOverview({ paint: provider })
  owner.replace(map); await vi.waitFor(() => expect(provider).toHaveBeenCalledOnce())
  expect(owner.read()).toBeNull(); expect(h.encoders).toHaveLength(0)
  const dispose = vi.fn()
  finish({ mode: 'full-composition', scene: {} as HTMLImageElement, presentationVersion: 'map4-reference-composition-v1',
    presentationHash: 'sha256:' + '1'.repeat(64), drawLabels() { h.order.push('labels') }, dispose })
  await vi.waitFor(() => expect(h.encoders).toHaveLength(1))
  expect(h.order).toEqual(['scene', 'labels']); expect(owner.read()).toBeNull(); expect(dispose).toHaveBeenCalledOnce()
  h.encoders[0]!(new Blob(['png'], { type: 'image/png' }))
  expect(owner.read()).toMatchObject({ presentationVersion: 'map4-reference-composition-v1', presentationHash: 'sha256:' + '1'.repeat(64) })
  owner.replace(null); expect(owner.read()).toBeNull(); expect(h.revoke).toHaveBeenCalledWith('blob:composition-test'); owner.dispose()
})

it('keeps the scene and new marker withdrawn when label drawing fails', async () => {
  const h = rasterHarness(), dispose = vi.fn()
  const owner = createReferenceSceneOverview({ paint: async () => ({ mode: 'full-composition', scene: {} as HTMLImageElement,
    presentationVersion: 'map4-reference-composition-v1', presentationHash: 'sha256:' + '1'.repeat(64),
    drawLabels() { throw Error('Actual font exceeds label clearance') }, dispose }) })
  owner.replace(await loadedMap4()); await vi.waitFor(() => expect(dispose).toHaveBeenCalledOnce())
  expect(owner.read()).toBeNull(); expect(h.encoders).toHaveLength(0); expect(h.create).not.toHaveBeenCalled(); owner.dispose()
})
