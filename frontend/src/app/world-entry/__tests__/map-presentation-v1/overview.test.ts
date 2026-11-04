// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { createReferenceSceneOverview, drawReferenceOverview, type ReferenceMapPaint } from '../../../../world/reference-scene/overview'
import { requireReferenceScene } from '../../../../world/reference-scene/model'
import { loadedMap4 } from './fixture'

afterEach(() => { vi.restoreAllMocks(); vi.unstubAllGlobals() })
function context() {
  const commands: unknown[][] = []
  const target = { fillStyle: '', strokeStyle: '', lineWidth: 0, imageSmoothingEnabled: true,
    drawImage: (...args: unknown[]) => commands.push(['image', ...args]),
    save() {}, restore() {}, beginPath() {}, closePath() {},
    moveTo: (...args: number[]) => commands.push(['move', ...args]),
    lineTo: (...args: number[]) => commands.push(['line', ...args]),
    fill() { commands.push(['fill', target.fillStyle]) }, stroke() { commands.push(['stroke', target.strokeStyle, target.lineWidth]) },
  }
  return { context: target as unknown as CanvasRenderingContext2D, commands }
}
function rasterHarness() {
  const c = context(), encoders: BlobCallback[] = []
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(() => c.context)
  vi.spyOn(HTMLCanvasElement.prototype, 'toBlob').mockImplementation(callback => { encoders.push(callback) })
  const create = vi.fn(() => 'blob:map4-test'), revoke = vi.fn()
  vi.stubGlobal('URL', class extends URL { static createObjectURL = create; static revokeObjectURL = revoke })
  return { ...c, encoders, create, revoke }
}
const paint = (): ReferenceMapPaint => ({ ground: { naturalWidth: 768, naturalHeight: 1296 } as HTMLImageElement, plazaFill: '#9fba96', dispose: vi.fn() })
it('changes only ground source and plaza fill: identical polygon/stroke/prop image/order/rect/depth', async () => {
  const map = await loadedMap4(), binding = requireReferenceScene(map), before = context(), after = context(), p = paint()
  drawReferenceOverview(before.context, binding); drawReferenceOverview(after.context, binding, p)
  expect(before.commands[0]![1]).toBe(map.image); expect(after.commands[0]![1]).toBe(p.ground)
  expect(after.commands.slice(1).filter(c => c[0] !== 'fill')).toEqual(before.commands.slice(1).filter(c => c[0] !== 'fill'))
  expect(before.commands.find(c => c[0] === 'fill')).toEqual(['fill', '#7a8742'])
  expect(after.commands.find(c => c[0] === 'fill')).toEqual(['fill', '#9fba96'])
})
it('preserves zero-options synchronous drawing and revokes its published identity before privacy teardown', async () => {
  const h = rasterHarness(), owner = createReferenceSceneOverview(), map = await loadedMap4()
  owner.replace(map); expect(h.encoders).toHaveLength(1)
  h.encoders[0]!(new Blob(['png'], { type: 'image/png' })); expect(owner.read()?.mapHash).toBe(map.map.content_hash)
  expect(owner.read()?.presentationHash).toBeUndefined()
  const observed: unknown[] = []; owner.subscribe(() => observed.push(owner.read()))
  owner.replace(null); expect(observed.at(-1)).toBeNull(); expect(h.revoke).toHaveBeenCalledWith('blob:map4-test')
  owner.dispose(); expect(h.revoke).toHaveBeenCalledOnce()
})
it('aborts replaced preparation, releases a late paint, and suppresses late encoding after disposal', async () => {
  const h = rasterHarness(), completions: ((p: ReferenceMapPaint) => void)[] = [], signals: AbortSignal[] = []
  const owner = createReferenceSceneOverview({ paint: (_map, signal) => { signals.push(signal); return new Promise(resolve => completions.push(resolve)) } })
  const map = await loadedMap4(); owner.replace(map)
  await vi.waitFor(() => expect(completions).toHaveLength(1))
  owner.replace(map); expect(signals[0]!.aborted).toBe(true)
  const old = paint(); completions[0]!(old)
  await vi.waitFor(() => expect(old.dispose).toHaveBeenCalledOnce()); expect(h.encoders).toHaveLength(0)
  await vi.waitFor(() => expect(completions).toHaveLength(2))
  const next = paint(); completions[1]!(next)
  await vi.waitFor(() => expect(h.encoders).toHaveLength(1)); expect(next.dispose).toHaveBeenCalledOnce()
  owner.dispose(); expect(signals[1]!.aborted).toBe(true)
  h.encoders[0]!(new Blob(['png'], { type: 'image/png' })); expect(owner.read()).toBeNull(); expect(h.create).not.toHaveBeenCalled()
})
it('keeps matching paint failure withdrawn; unsupported null paint renders the original source', async () => {
  const h = rasterHarness(), map = await loadedMap4(), failure = vi.fn(async () => { throw Error('hash mismatch') })
  const owner = createReferenceSceneOverview({ paint: failure }); owner.replace(map)
  await vi.waitFor(() => expect(failure).toHaveBeenCalledOnce()); await Promise.resolve(); await Promise.resolve()
  expect(owner.read()).toBeNull(); expect(h.encoders).toHaveLength(0); owner.dispose()
  const other = createReferenceSceneOverview({ paint: async () => null }); other.replace(map)
  await vi.waitFor(() => expect(h.encoders).toHaveLength(1)); expect(h.commands[0]![1]).toBe(map.image); other.dispose()
})
