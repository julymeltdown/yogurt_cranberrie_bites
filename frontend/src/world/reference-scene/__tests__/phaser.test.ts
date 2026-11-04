import { expect, it, vi } from 'vitest'
import type Phaser from 'phaser'
import { decodedFixture } from './lease-fixture'
import { prepareReferenceSceneSkin } from '../phaser'
import { sceneryDepth } from '../../illustrated/model'

type TestImage = { x: number; y: number; key: string; frame: string | undefined;
  setOrigin: ReturnType<typeof vi.fn>; setVisible: ReturnType<typeof vi.fn>; setMask: ReturnType<typeof vi.fn>;
  setDisplaySize: ReturnType<typeof vi.fn>; setDepth: ReturnType<typeof vi.fn>; destroy: ReturnType<typeof vi.fn> }
async function setup() {
  const map = await decodedFixture(), rows: TestImage[] = [], textures = new Map<string, { add: ReturnType<typeof vi.fn>; setFilter: ReturnType<typeof vi.fn> }>()
  let alive = true
  function image(x: number, y: number, key: string, frame?: string): TestImage {
    const row = { x, y, key, frame, setOrigin: vi.fn(), setVisible: vi.fn(), setMask: vi.fn(), setDisplaySize: vi.fn(), setDepth: vi.fn(), destroy: vi.fn() }
    for (const fn of [row.setOrigin, row.setVisible, row.setMask, row.setDisplaySize, row.setDepth]) fn.mockReturnValue(row)
    rows.push(row); return row
  }
  const mask = { destroy: vi.fn() }, graphics = { fillStyle: vi.fn(), fillRect: vi.fn(), createGeometryMask: vi.fn(() => mask), destroy: vi.fn() }
  graphics.fillStyle.mockReturnValue(graphics); graphics.fillRect.mockReturnValue(graphics)
  const scene = { textures: {
    addImage: vi.fn((key: string) => { const texture = { setFilter: vi.fn(), add: vi.fn(() => ({})) }; textures.set(key, texture); return texture }),
    exists: (key: string) => textures.has(key), remove: vi.fn((key: string) => { textures.delete(key) }), get: (key: string) => textures.get(key),
  }, make: { graphics: vi.fn(() => graphics) }, add: { image: vi.fn(image) } }
  const build = () => prepareReferenceSceneSkin(scene as unknown as Phaser.Scene, map, 'mcr-reviewed-map-test', 1, vi.fn(), () => alive)
  return { map, rows, textures, scene, graphics, mask, build, revoke: () => { alive = false } }
}
it('prepares two textures and exactly22 rooted props plus ground, clipping without adding physics or MAP illustrations', async () => {
  const s = await setup(), skin = s.build()
  expect(s.textures.size).toBe(2); expect(s.rows).toHaveLength(23)
  expect(s.graphics.fillRect).toHaveBeenCalledExactlyOnceWith(0, 0, 768, 1296)
  expect(s.rows.every(row => row.setVisible.mock.calls[0]?.[0] === false)).toBe(true)
  expect(s.rows[0]!.setDepth).toHaveBeenCalledExactlyOnceWith(0)
  s.map.scenery!.manifest.objects.forEach((o, i) => {
    const row = s.rows[i + 1]!; expect([row.x, row.y, row.frame]).toEqual([o.x, o.y, o.id])
    expect(row.setDisplaySize).toHaveBeenCalledExactlyOnceWith(o.width, o.height)
    expect(row.setDepth).toHaveBeenCalledExactlyOnceWith(sceneryDepth(o.depth_y, 1296))
    expect(row.setMask).toHaveBeenCalledExactlyOnceWith(s.mask)
    expect(s.textures.get(row.key)!.add).toHaveBeenCalledWith(o.id, 0, o.frame.x, o.frame.y, o.frame.width, o.frame.height)
  })
  skin.show(); expect(s.rows.every(row => row.setVisible.mock.lastCall?.[0] === true)).toBe(true)
  skin.remove(); skin.remove(); expect(s.textures.size).toBe(0); expect(s.rows.every(row => row.destroy.mock.calls.length === 1)).toBe(true)
  expect(s.mask.destroy).toHaveBeenCalledOnce(); expect(s.graphics.destroy).toHaveBeenCalledOnce(); expect(s.map.dispose).not.toHaveBeenCalled()
})
it('cleans partial allocations when texture upload reenters teardown', async () => {
  const s = await setup(), add = s.scene.textures.addImage.getMockImplementation()!
  s.scene.textures.addImage.mockImplementationOnce(key => { const result = add(key); s.revoke(); return result })
  expect(s.build).toThrow(); expect(s.textures.size).toBe(0); expect(s.rows).toHaveLength(0)
})
it('cleans mask, objects and both textures if any source frame registration fails', async () => {
  const s = await setup(), add = s.scene.textures.addImage.getMockImplementation()!
  s.scene.textures.addImage.mockImplementation(key => { const result = add(key); result.add.mockReturnValueOnce(null as never); return result })
  expect(s.build).toThrow(); expect(s.textures.size).toBe(0); expect(s.rows[0]!.destroy).toHaveBeenCalledOnce()
  expect(s.mask.destroy).toHaveBeenCalledOnce(); expect(s.graphics.destroy).toHaveBeenCalledOnce()
})
it('a reentrant failed show releases its candidate while leaving the previous scene owner untouched', async () => {
  const s = await setup(), skin = s.build()
  s.rows[0]!.setVisible.mockImplementationOnce(() => { s.revoke(); return s.rows[0] })
  expect(skin.show).toThrow(); expect(s.textures.size).toBe(0); expect(s.rows.every(row => row.destroy.mock.calls.length === 1)).toBe(true)
  expect(s.map.dispose).not.toHaveBeenCalled()
})
it('continues cleanup after one object destructor fails', async () => {
  const s = await setup(), skin = s.build(); s.rows[0]!.destroy.mockImplementationOnce(() => { throw Error('broken object') })
  skin.remove(); expect(s.rows.at(-1)!.destroy).toHaveBeenCalledOnce(); expect(s.textures.size).toBe(0); expect(s.graphics.destroy).toHaveBeenCalledOnce()
})
