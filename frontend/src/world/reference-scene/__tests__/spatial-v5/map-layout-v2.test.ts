import { expect, it } from 'vitest'
import { createHash } from 'node:crypto'
import { readFile } from 'node:fs/promises'
import { spawnSync } from 'node:child_process'
import labels from '../../spatial-v5/map-labels-source.json'
import plants from '../../spatial-v5/map-landscape-source.json'
import labelPins from '../../spatial-v5/map-labels-pins.json'
import plantPins from '../../spatial-v5/map-landscape-pin.json'
import source from '../../spatial-v5/source-inputs.json'
import { validateSpatialV5MapLayoutPair, verifySpatialV5MapLayoutPair, getSpatialV5OverviewMetadata, spatialV5SceneIdentity } from '../../spatial-v5/model'
import { spatialV5LabelObstacles } from '../../../../features/places/ui/reference-scene/spatial-v5-label-obstacles'

const hash = (raw: Uint8Array) => 'sha256:' + createHash('sha256').update(raw).digest('hex')
it('closes both published source sidecars and their bundled-document hashes', async () => {
  for (const [file, pin, document] of [['label-presentation.json', labelPins.layout_pair.labels, labels], ['landscape-presentation.json', plantPins.layout_pair.landscape, plants]] as const) {
    const bytes = await readFile('public/world/reference-scene-v2/map-layout-v2/' + file)
    expect(bytes.length).toBe(pin.bytes); expect(hash(bytes)).toBe(pin.sha256)
    expect(JSON.parse(bytes.toString())).toEqual(document)
  }
  await expect(verifySpatialV5MapLayoutPair()).resolves.toBeUndefined()
})
it.each(['REVENGE', 'PARADE', 'DANGER'])('overrides only %s display anchors and exact painted rectangles', themeId => {
  const identity = { ...spatialV5SceneIdentity, mapRevision: 7005, themeId }
  expect(getSpatialV5OverviewMetadata(identity)!.mapLabels.map(p => [p.x, p.y])).toEqual([[422, 201], [327, 529], [177, 700], [652, 685], [371, 918], [518, 1159]])
  expect(source.map_labels[0]).toMatchObject({ x: 335, y: 147 })
  const boxes = spatialV5LabelObstacles(identity, 768, 1296)
  expect(boxes.map(p => ({ x: p.left, y: p.top, width: p.width, height: p.height }))).toEqual(labels.labels.map(p => p.rect))
  expect(boxes.filter(p => p.interactionId !== null).map(p => p.interactionId)).toEqual(['stage_info', 'gifts_info'])
})
it.each(['old-label', 'old-plants', 'pair', 'geometry', 'binding', 'offcenter', 'outside', 'zero-width', 'nan', 'duplicate-bed', 'bound-bed'] as const)('rejects %s before exposing mixed or malformed layout', async fault => {
  const l = structuredClone(labels), pin = structuredClone(plantPins)
  let p = structuredClone(plants)
  if (fault === 'old-label') l.source_layout_version = 'map-layout-v1'
  if (fault === 'old-plants') p = JSON.parse(await readFile('public/world/reference-scene-v2/map-landscape-v1/presentation.json', 'utf8'))
  if (fault === 'pair') pin.layout_pair.labels.sha256 = 'sha256:' + '0'.repeat(64)
  if (fault === 'geometry') p.geometry_hash = 'sha256:' + '0'.repeat(64)
  if (fault === 'binding') l.labels[0]!.interaction_id = 'gifts_info'
  if (fault === 'offcenter') l.labels[0]!.x += 2
  if (fault === 'outside') l.labels[0]!.rect.x = -1
  if (fault === 'zero-width') l.labels[0]!.rect.width = 0
  if (fault === 'nan') l.labels[0]!.x = NaN
  if (fault === 'duplicate-bed') p.decorations[1]!.id = p.decorations[0]!.id
  if (fault === 'bound-bed') Object.assign(p.decorations[0]!, { interaction_id: 'stage_info' })
  expect(() => validateSpatialV5MapLayoutPair(l, p, labelPins, pin)).toThrow()
})
it.each(['labels', 'planting'] as const)('rejects structurally valid tampered %s by its closed hash', async fault => {
  const l = structuredClone(labels), p = structuredClone(plants)
  if (fault === 'labels') { l.labels[0]!.x++; l.labels[0]!.rect.x++ }
  else p.decorations[0]!.x++
  expect(() => validateSpatialV5MapLayoutPair(l, p)).not.toThrow()
  await expect(verifySpatialV5MapLayoutPair(l, p)).rejects.toThrow()
})
it('reproduces the generated pair without writing any excluded World tuple input', async () => {
  const folder = 'src/world/reference-scene/spatial-v5/'
  const frozen = [folder + 'source-inputs.json', folder + 'tile-pin.json', folder + 'backend-allocation.json', folder + 'camera.ts', ...['canonical-map.json', 'source-binding.json'].map(p => 'public/world/reference-scene-v2/runtime/spatial-v5/' + p)]
  const generated = ['map-labels-source.json', 'map-labels-pins.json', 'map-landscape-source.json', 'map-landscape-pin.json'].map(p => folder + p)
  const before = await Promise.all([...frozen, ...generated].map(async p => hash(await readFile(p))))
  for (const name of ['build-source-inputs.py', 'build-map-landscape-inputs.py']) {
    const result = spawnSync('python3', ['-B', folder + name, '--map-layout-v2'], { encoding: 'utf8' })
    expect(result.status, result.stderr).toBe(0)
  }
  expect(await Promise.all([...frozen, ...generated].map(async p => hash(await readFile(p))))).toEqual(before)
})
