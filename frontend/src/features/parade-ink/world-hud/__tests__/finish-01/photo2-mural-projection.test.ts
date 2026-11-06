import { expect, it } from 'vitest'
import { readFileSync } from 'node:fs'
import type Phaser from 'phaser'
import { createPhoto2Projection, PHOTO2_VERSION, type DisplayMesh } from '../../photo2-world-display-v1'
import { createProfiledMuralScene, paintPhoto2Mural, photo2MuralPlane } from '../../../../../world/reference-scene/photo2-world-display-v1/phaser'
import { isPhoto2ReferenceClaim, registeredPhoto2Release, type Photo2SceneDisplay } from '../../../../../world/reference-scene/photo2-world-display-v1/model'
import type { LoadedMap } from '../../../../../world/map/resources'
import { assertSameWorld } from '../../../../../world/theme/compatibility'

const candidate = JSON.parse(readFileSync(new URL('../../../../../../../docs/mcr/evidence/pi08-finish-01/photo2-world-contract/producer-input-v2.json', import.meta.url), 'utf8')) as { mesh: DisplayMesh; ground: { displayBounds: Photo2SceneDisplay['groundBounds'] } }
const display: Photo2SceneDisplay = { version: PHOTO2_VERSION, mesh: candidate.mesh, projection: createPhoto2Projection(candidate.mesh),
  groundBounds: candidate.ground.displayBounds, camera: { x: 0, y: 0, width: 390, height: 504 }, frames: [], boards: [] }
const anchor = { x: 270, y: 330, width: 120, height: 160, depth: 1.4 }
const rgba = new Uint8ClampedArray(65536)
for (let y = 0; y < 128; y++) for (let x = 0; x < 128; x++) rgba.set([x, y, (x + y) % 256, 255], (y * 128 + x) * 4)

it('rasterizes mural pixel centers using the same inverse across multiple non-affine mesh cells', () => {
  const plane = photo2MuralPlane(display, anchor)
  expect(plane.triangles.length).toBeGreaterThan(2)
  const output = new Uint8ClampedArray(plane.bounds.width * plane.bounds.height * 4)
  paintPhoto2Mural(plane, anchor, rgba, output)
  let painted = 0
  for (let y = 0; y < plane.bounds.height; y++) for (let x = 0; x < plane.bounds.width; x++) {
    const native = display.projection.inverse({ x: plane.bounds.x + x + .5, y: plane.bounds.y + y + .5 })
    const offset = (y * plane.bounds.width + x) * 4
    if (native && native.x >= anchor.x + 1e-6 && native.y >= anchor.y + 1e-6
      && native.x < anchor.x + anchor.width - 1e-6 && native.y < anchor.y + anchor.height - 1e-6) {
      const sx = Math.max(0, Math.min(127, Math.floor((native.x - anchor.x) * 128 / anchor.width)))
      const sy = Math.max(0, Math.min(127, Math.floor((native.y - anchor.y) * 128 / anchor.height)))
      expect([...output.slice(offset, offset + 4)]).toEqual([sx, sy, (sx + sy) % 256, 255]); painted++
    } else if (!native || native.x < anchor.x - 1e-6 || native.y < anchor.y - 1e-6
      || native.x > anchor.x + anchor.width + 1e-6 || native.y > anchor.y + anchor.height + 1e-6) expect(output[offset + 3]).toBe(0)
  }
  expect(painted).toBeGreaterThan(100)
})

it('rejects invalid raster lengths and anchors outside the mesh', () => {
  expect(() => photo2MuralPlane(display, { x: 2000, y: 2000, width: 10, height: 10 })).toThrow()
  const plane = photo2MuralPlane(display, anchor)
  expect(() => paintPhoto2Mural(plane, anchor, rgba.slice(1), new Uint8ClampedArray(1))).toThrow()
})

function fixture() {
  const textures = new Map<string, object>(), images = new Set<object>()
  let fail = false
  const scene = {
    textures: {
      exists: (key: string) => textures.has(key), remove: (key: string) => { textures.delete(key) },
      createCanvas(key: string, width: number, height: number) {
        if (fail) { fail = false; throw new Error('Injected allocation failure') }
        if (textures.has(key)) throw new Error('Duplicate texture')
        const texture = { width, height, setFilter() {}, refresh() {}, getContext: () => ({
          createImageData: (w: number, h: number) => ({ data: new Uint8ClampedArray(w * h * 4) }), putImageData() {},
        }) }
        textures.set(key, texture); return texture
      },
    },
    add: { image() {
      const image = { setOrigin: () => image, setDisplaySize: () => image, setDepth: () => image, destroy: () => { images.delete(image) } }
      images.add(image); return image
    } },
    events: { on() {}, off() {}, once() {} },
  } as unknown as Phaser.Scene
  return { scene, textures, images, failNext: () => { fail = true } }
}

it('retains one public mural lease and RGBA through legacy to Photo2 and back, then releases every resource', () => {
  const f = fixture(), refreshers = new Set<() => void>()
  let profile: Photo2SceneDisplay | null = null
  const port = createProfiledMuralScene(f.scene, anchor, 'test-mural', () => profile, refreshers)
  const texture = port.texture(), object = port.object(texture); texture.replace(rgba)
  expect(f.textures.size).toBe(1); expect(f.images.size).toBe(1)
  profile = display; for (const refresh of refreshers) refresh()
  expect(f.textures.size).toBe(1); expect(f.images.size).toBe(1)
  texture.replace(rgba)
  profile = null; for (const refresh of refreshers) refresh()
  expect(f.textures.size).toBe(1); expect(f.images.size).toBe(1)
  object.dispose(); texture.dispose(); object.dispose(); texture.dispose()
  expect(f.textures.size).toBe(0); expect(f.images.size).toBe(0); expect(refreshers.size).toBe(0)
})

it('recovers the previous mural profile after an injected replacement allocation failure', () => {
  const f = fixture(), refreshers = new Set<() => void>()
  let profile: Photo2SceneDisplay | null = null
  const port = createProfiledMuralScene(f.scene, anchor, 'rollback-mural', () => profile, refreshers)
  const texture = port.texture(), object = port.object(texture); texture.replace(rgba)
  profile = display; f.failNext()
  expect(() => { for (const refresh of refreshers) refresh() }).toThrow('Injected allocation failure')
  profile = null; for (const refresh of refreshers) refresh()
  expect(f.textures.size).toBe(1); expect(f.images.size).toBe(1)
  texture.replace(rgba); object.dispose(); texture.dispose()
  expect(f.textures.size).toBe(0); expect(f.images.size).toBe(0)
})

it('recognizes new and mixed Photo2 namespace claims but refuses unregistered theme labels', () => {
  const map = { map: { content_hash: 'sha256:' + 'a'.repeat(64) }, theme: { theme_id: 'PARADE', theme_revision: 2, content_hash: 'sha256:' + 'b'.repeat(64),
    scenery: { ground: { path: 'world/legacy.png' }, atlases: [{ path: 'world/reference-scene-v2/photo2-world-display-v1/parade/atlas.png' }] } } } as unknown as LoadedMap
  expect(isPhoto2ReferenceClaim(map)).toBe(true)
  expect(() => registeredPhoto2Release(map)).toThrow('Map resource unavailable')
  const objects = [{ id: 'stage', x: 10, y: 20, width: 100, height: 80, depth_y: 100 },
    { id: 'wall', x: 200, y: 240, width: 90, height: 60, depth_y: 300 }]
  const incumbent = { map: { map_id: 'frozen-map', map_revision: 4 }, widthPx: 768, heightPx: 1296, spawn: { cell_x: 29, cell_y: 26 },
    theme: { avatar_manifest_id: 'same', avatar_manifest_version: 1, avatar_content_hash: 'same', scenery: { objects, ground: { width_px: 768, height_px: 1296 } } } } as unknown as LoadedMap
  const next = structuredClone(incumbent)
  Object.assign(next.theme.scenery!.ground, { width_px: 499, height_px: 1200 })
  expect(() => assertSameWorld(incumbent, next)).not.toThrow()
  for (const altered of [objects.map((o, i) => i ? o : { ...o, x: o.x + 1 }), objects.map((o, i) => i ? o : { ...o, id: 'renamed' }), [...objects].reverse()]) {
    const invalid = { ...next, theme: { ...next.theme, scenery: { ...next.theme.scenery!, objects: altered } } } as unknown as LoadedMap
    expect(() => assertSameWorld(incumbent, invalid)).toThrow()
  }
})
