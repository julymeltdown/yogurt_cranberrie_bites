import { expect, it } from 'vitest'
import { readFile } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { spawnSync } from 'node:child_process'
import { inflateSync } from 'node:zlib'
import input from '../../spatial-v5/architecture-source.json'
import pins from '../../spatial-v5/architecture-pin.json'
import base from '../../spatial-v5/source-inputs.json'
import { spatialV5ArchitectureScenery } from '../../spatial-v5/architecture'
import { getSpatialV5Scenery, spatialV5ThemeIds } from '../../spatial-v5/model'
import type { Scenery } from '../../../illustrated/model'

const sha = (raw: Uint8Array) => 'sha256:' + createHash('sha256').update(raw).digest('hex')
const read = (path: string) => readFile('../' + path)
it('closes the published handoff, all18 outputs and unchanged geometry/MAP inputs', async () => {
  for (const p of [pins.handoff, ...pins.outputs, ...pins.frozen_inputs, pins.document]) {
    const raw = await read(p.path); expect(raw.length, p.path).toBe(p.bytes); expect(sha(raw), p.path).toBe(p.sha256)
  }
  expect(pins.outputs).toHaveLength(18)
  expect(pins.outputs.reduce((n, p) => n + p.bytes, 0)).toBe(7921243)
  const binding = JSON.parse(await readFile('public/world/reference-scene-v2/runtime/architecture-v2/source-binding.json', 'utf8'))
  expect(binding).toMatchObject({ status: 'NOT_RELEASE_BOUND', production_status: 'NOT_ADMITTED', geometry_hash: base.geometry_hash })
  expect(binding.document).toEqual(pins.document); expect(binding.frozen_inputs).toEqual(pins.frozen_inputs)
  expect(input.base_source_revision).toBe(base.source_revision)
})

it.each(spatialV5ThemeIds)('changes only %s atlas identity while preserving every object/ground/frame/label/camera field', async id => {
  const scenery = getSpatialV5Scenery(id), original = base.scenes[id].scenery
  expect(scenery).toEqual(input.scenes[id].scenery)
  expect(scenery.ground).toEqual(original.ground)
  expect(scenery.objects.map(({ atlas_id: _atlas, ...o }) => o)).toEqual(original.objects.map(({ atlas_id: _atlas, ...o }) => o))
  expect(scenery.objects).toHaveLength(34)
  const folder = 'public/world/reference-scene-v2/architecture-v2/' + id.toLowerCase() + '/'
  const layout = JSON.parse(await readFile(folder + 'layout.json', 'utf8'))
  expect(layout.frames).toEqual(base.scenes[id].frames)
  const resources = JSON.parse(await readFile(folder + 'resource-inputs.json', 'utf8'))
  expect(resources.resources).toEqual([scenery.ground, ...scenery.atlases])
  expect(resources.total_bytes).toBeLessThanOrEqual(2097152); expect(resources.total_pixels).toBe(2043904)
  expect(scenery.atlases[0]).toEqual(pins.runtime_atlases[id])
})

it.each(['version', 'geometry', 'revision', 'admission', 'atlas-hash', 'atlas-size', 'atlas-path', 'cross-era', 'frame', 'depth', 'feet', 'ground', 'object-id', 'missing', 'pin', 'unknown-scene'] as const)('rejects malformed architecture %s', fault => {
  const doc = structuredClone(input), p = structuredClone(pins), scene = doc.scenes.PARADE.scenery
  if (fault === 'version') doc.version++
  if (fault === 'geometry') doc.geometry_hash = 'sha256:' + '0'.repeat(64)
  if (fault === 'revision') doc.base_source_revision = 'foreign'
  if (fault === 'admission') doc.production_status = 'ADMITTED'
  if (fault === 'atlas-hash') scene.atlases[0]!.sha256 = 'sha256:' + '0'.repeat(64)
  if (fault === 'atlas-size') scene.atlases[0]!.bytes++
  if (fault === 'atlas-path') scene.atlases[0]!.path = base.scenes.PARADE.scenery.atlases[0]!.path
  if (fault === 'cross-era') scene.atlases = structuredClone(input.scenes.DANGER.scenery.atlases)
  if (fault === 'frame') scene.objects[0]!.frame.x++
  if (fault === 'depth') scene.objects[0]!.depth_y++
  if (fault === 'feet') scene.objects[0]!.y++
  if (fault === 'ground') scene.ground.sha256 = 'sha256:' + '0'.repeat(64)
  if (fault === 'object-id') scene.objects[0]!.id = 'foreign'
  if (fault === 'missing') scene.objects.pop()
  if (fault === 'pin') p.handoff.sha256 = 'sha256:' + '0'.repeat(64)
  if (fault === 'unknown-scene') Reflect.deleteProperty(doc.scenes, 'PARADE')
  expect(() => spatialV5ArchitectureScenery('PARADE', base.scenes.PARADE.scenery as Scenery, doc, p)).toThrow()
})

/** Independent lossless PNG decoder for the pinned8-bitRGBA source atlases; no image library export. */
function rgba(png: Buffer) {
  expect(png.subarray(0, 8).toString('hex')).toBe('89504e470d0a1a0a')
  const width = png.readUInt32BE(16), height = png.readUInt32BE(20)
  expect([png[24], png[25], png[28]]).toEqual([8, 6, 0])
  const chunks = []
  for (let i = 8; i < png.length;) {
    const size = png.readUInt32BE(i), kind = png.toString('ascii', i + 4, i + 8)
    if (kind === 'IDAT') chunks.push(png.subarray(i + 8, i + 8 + size))
    i += size + 12
  }
  const raw = inflateSync(Buffer.concat(chunks)), stride = width * 4, pixels = Buffer.alloc(width * height * 4)
  for (let y = 0; y < height; y++) {
    const filter = raw[y * (stride + 1)]!
    for (let x = 0; x < stride; x++) {
      const offset = y * stride + x, a = x >= 4 ? pixels[offset - 4]! : 0, b = y ? pixels[offset - stride]! : 0, c = x >= 4 && y ? pixels[offset - stride - 4]! : 0
      const estimate = a + b - c, pa = Math.abs(estimate - a), pb = Math.abs(estimate - b), pc = Math.abs(estimate - c)
      const predict = filter === 0 ? 0 : filter === 1 ? a : filter === 2 ? b : filter === 3 ? Math.floor((a + b) / 2) : filter === 4 ? (pa <= pb && pa <= pc ? a : pb <= pc ? b : c) : NaN
      if (!Number.isFinite(predict)) throw Error('Invalid PNG filter')
      pixels[offset] = (raw[y * (stride + 1) + x + 1]! + predict) & 255
    }
  }
  return { width, height, pixels }
}
it.each(spatialV5ThemeIds)('preserves every %s RGBA channel outside the four fixed building frames', async id => {
  const a = rgba(await readFile('public/' + base.scenes[id].scenery.atlases[0]!.path))
  const b = rgba(await readFile('public/' + input.scenes[id].scenery.atlases[0]!.path))
  expect([a.width, a.height, b.width, b.height]).toEqual([1024, 1024, 1024, 1024])
  const rects = [[6, 550, 244, 180], [262, 550, 244, 180], [518, 550, 244, 180], [774, 550, 244, 180]]
  let outsideDifferent = 0, insideDifferent = 0
  for (let y = 0; y < 1024; y++) for (let x = 0; x < 1024; x++) {
    const inside = rects.some(([rx, ry, w, h]) => x >= rx! && x < rx! + w! && y >= ry! && y < ry! + h!)
    const i = (y * 1024 + x) * 4
    for (let c = 0; c < 4; c++) if (a.pixels[i + c] !== b.pixels[i + c]) { if (inside) insideDifferent++; else outsideDifferent++ }
  }
  expect(outsideDifferent).toBe(0); expect(insideDifferent).toBeGreaterThan(0)
})

it('reproduces exactly three new outputs without touching frozen World/MAP/art originals', async () => {
  const targets = ['src/world/reference-scene/spatial-v5/architecture-source.json', 'src/world/reference-scene/spatial-v5/architecture-pin.json', 'public/world/reference-scene-v2/runtime/architecture-v2/source-binding.json']
  const paths = [...targets, ...pins.frozen_inputs.map(p => p.path.replace('frontend/', '')), ...pins.outputs.map(p => p.path.replace('frontend/', ''))]
  const before = await Promise.all(paths.map(async p => sha(await readFile(p))))
  const result = spawnSync('python3', ['-B', 'src/world/reference-scene/spatial-v5/build-architecture-inputs.py'], { encoding: 'utf8' })
  expect(result.status, result.stderr).toBe(0)
  expect(await Promise.all(paths.map(async p => sha(await readFile(p))))).toEqual(before)
})
