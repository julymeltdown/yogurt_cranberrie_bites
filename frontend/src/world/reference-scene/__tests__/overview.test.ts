// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { createReferenceSceneOverview, drawReferenceOverview, requireReferenceScene } from '..'
import { decodedFixture } from './lease-fixture'

afterEach(() => { vi.restoreAllMocks(); vi.unstubAllGlobals() })
async function setup() {
  const map = await decodedFixture(), jobs: { canvas: HTMLCanvasElement; done: BlobCallback }[] = [], drawImage = vi.fn()
  const ctx = { drawImage, save: vi.fn(), restore: vi.fn(), beginPath: vi.fn(), moveTo: vi.fn(), lineTo: vi.fn(), closePath: vi.fn(), fill: vi.fn(), stroke: vi.fn() } as unknown as CanvasRenderingContext2D
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(ctx)
  vi.spyOn(HTMLCanvasElement.prototype, 'toBlob').mockImplementation(function(this: HTMLCanvasElement, done) { jobs.push({ canvas: this, done }) })
  let serial = 0
  const create = vi.fn(() => 'blob:reference-' + ++serial), revoke = vi.fn()
  vi.stubGlobal('URL', { createObjectURL: create, revokeObjectURL: revoke })
  return { map, jobs, ctx, create, revoke, owner: createReferenceSceneOverview(), blob: () => new Blob(['actual encoding stub'], { type: 'image/png' }) }
}

it('draws exact MAP replacements in stable foot order from the same two decoded WORLD resources', async () => {
  const s = await setup(), binding = requireReferenceScene(s.map)
  drawReferenceOverview(s.ctx, binding)
  expect(s.ctx.drawImage).toHaveBeenCalledTimes(28)
  const ordered = [...binding.overview].sort((a, b) => a.depth_y - b.depth_y)
  ordered.forEach((o, i) => expect(vi.mocked(s.ctx.drawImage).mock.calls[i + 1]?.slice(1)).toEqual([o.frame.x, o.frame.y, o.frame.width, o.frame.height, o.x, o.y, o.width, o.height]))
  expect(s.ctx.fillStyle).toBe('#7a8742'); expect(s.ctx.strokeStyle).toBe('#b4a27d'); expect(s.ctx.lineWidth).toBe(6)
  expect(s.ctx.moveTo).toHaveBeenCalledWith(238, 563)
})
it('publishes matching immutable pins, withdraws synchronously and revokes each consumer URL', async () => {
  const s = await setup(); s.owner.replace(s.map); expect(s.owner.read()).toBeNull(); s.jobs[0]!.done(s.blob())
  expect(s.owner.read()).toMatchObject({ imageUrl: 'blob:reference-1', width: 768, height: 1296, mapId: s.map.map.map_id, mapHash: s.map.map.content_hash,
    geometryHash: s.map.map.geometry_hash, themeId: 'PARADE', themeHash: s.map.theme.content_hash })
  let observed: unknown = 'not-called'; s.owner.subscribe(() => { observed = s.owner.read() })
  s.owner.replace(null); expect(observed).toBeNull(); expect(s.revoke).toHaveBeenCalledExactlyOnceWith('blob:reference-1')
  expect(s.jobs[0]!.canvas.width).toBe(0); s.owner.dispose()
})
it('bounds encoding to one active and one latest pending raster, fencing stale callbacks and privacy withdrawal', async () => {
  const s = await setup(); s.owner.replace(s.map); s.owner.replace(s.map); s.owner.replace(s.map)
  expect(s.jobs).toHaveLength(1); expect(s.jobs[0]!.canvas.width).toBe(0)
  s.jobs[0]!.done(s.blob()); expect(s.jobs).toHaveLength(2); expect(s.create).not.toHaveBeenCalled()
  s.owner.replace(null); expect(s.jobs[1]!.canvas.width).toBe(0); s.jobs[1]!.done(s.blob())
  expect(s.owner.read()).toBeNull(); expect(s.create).not.toHaveBeenCalled(); s.owner.dispose()
})
it('prevents reentrant observers from creating an extra simultaneous encoding or resurrecting disposed pixels', async () => {
  const s = await setup(); s.owner.replace(s.map)
  let once = false
  s.owner.subscribe(() => { if (!once && s.owner.read()) { once = true; s.owner.replace(s.map); s.owner.replace(s.map) } })
  s.jobs[0]!.done(s.blob()); expect(s.jobs).toHaveLength(2)
  s.jobs[1]!.done(s.blob()); expect(s.jobs).toHaveLength(3); s.jobs[2]!.done(s.blob())
  expect(s.owner.read()?.imageUrl).toBe('blob:reference-2')
  s.owner.subscribe(() => s.owner.replace(s.map)); const before = s.jobs.length; s.owner.dispose()
  expect(s.jobs).toHaveLength(before); expect(s.owner.read()).toBeNull(); expect(s.revoke).toHaveBeenCalledTimes(2)
})
it('rejects malformed leases/encoding errors and applies the existing per-PNG limit to the local raster', async () => {
  const s = await setup()
  s.owner.replace({ ...s.map, scenery: { ...s.map.scenery!, transferBytes: 1 } }); expect(s.jobs).toHaveLength(0)
  s.owner.replace(s.map); s.jobs[0]!.done(null); expect(s.owner.read()).toBeNull()
  s.owner.replace(s.map); s.jobs[1]!.done(new Blob([new Uint8Array(4194305)], { type: 'image/png' })); expect(s.owner.read()).toBeNull()
  s.owner.replace(s.map); s.jobs[2]!.done(new Blob([new Uint8Array(2114297)], { type: 'image/png' })); expect(s.owner.read()).not.toBeNull(); s.owner.dispose()
})
