import { expect, it } from 'vitest'
import { readFile } from 'node:fs/promises'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import document from '../../spatial-v5/map-cozy-source.json'
import pins from '../../spatial-v5/map-cozy-pins.json'
import architecture from '../../spatial-v5/architecture-source.json'
import base from '../../spatial-v5/source-inputs.json'
import { spatialV5CozyScenery, validateSpatialV5Cozy, verifySpatialV5Cozy } from '../../spatial-v5/map-cozy'
import { getSpatialV5Scenery, spatialV5ThemeIds } from '../../spatial-v5/model'
import type { Scenery } from '../../../illustrated/model'
const hash = (raw: Uint8Array) => 'sha256:' + createHash('sha256').update(raw).digest('hex')
it('closes the committed producer and frozen semantic inputs and reproducibly emits only two additive files', async () => {
  for (const pin of [pins.handoff, ...pins.outputs, ...pins.frozen_inputs, pins.document]) {
    const raw = await readFile('../' + pin.path)
    expect(raw.length, pin.path).toBe(pin.bytes); expect(hash(raw), pin.path).toBe(pin.sha256)
  }
  const paths = ['src/world/reference-scene/spatial-v5/map-cozy-source.json', 'src/world/reference-scene/spatial-v5/map-cozy-pins.json']
  const before = await Promise.all(paths.map(p => readFile(p)))
  execFileSync('python3', ['-B','src/world/reference-scene/spatial-v5/build-map-cozy-inputs.py',
    '--source-revision',pins.source_revision,'--handoff-sha',pins.handoff.sha256])
  expect(await Promise.all(paths.map(p => readFile(p)))).toEqual(before)
  await expect(verifySpatialV5Cozy()).resolves.toBeUndefined()
})
it.each(spatialV5ThemeIds)('changes only the explicitly sourced %s environment descriptors, preserving all34 objects and atlas identity', era => {
  const original = architecture.scenes[era].scenery as Scenery
  const next = spatialV5CozyScenery(era, original)
  expect(next.objects).toBe(original.objects)
  expect(next.objects).toHaveLength(34)
  expect(next.atlases[0]!.id).toBe(original.atlases[0]!.id)
  expect(next.ground.id).toBe(original.ground.id)
  const scene = document.scenes[era]
  expect(next.ground).toEqual(scene.ground); expect(next.atlases).toEqual([scene.atlas])
  expect(next.ground.path).not.toBe(original.ground.path)
  expect(next.atlases[0]!.path).not.toBe(original.atlases[0]!.path)
  expect(scene.frame_rects).toEqual(base.scenes[era].frames)
  expect(next.ground.bytes + next.atlases[0]!.bytes).toBeLessThanOrEqual(2097152)
  expect(getSpatialV5Scenery(era)).toEqual(next)
})
it.each(['admission','geometry','base','era','ground-path','ground-hash','ground-dimensions','atlas-id','atlas-path','atlas-bytes','frame','handoff','source-revision','cross-era-tuple'] as const)('rejects tampered cozy %s even after document rehashing', async fault => {
  const d = structuredClone(document), p = structuredClone(pins), scene = d.scenes.REVENGE
  if (fault === 'admission') d.production_status = 'ADMITTED'
  if (fault === 'geometry') d.geometry_hash = 'sha256:FOREIGN'
  if (fault === 'base') d.base_source_revision = 'FOREIGN'
  if (fault === 'era') d.eras.push('DANGER')
  if (fault === 'ground-path') scene.ground.path = 'world/foreign/ground.png'
  if (fault === 'ground-hash') scene.ground.sha256 = 'sha256:FOREIGN'
  if (fault === 'ground-dimensions') scene.ground.width_px++
  if (fault === 'atlas-id') scene.atlas.id = 'foreign'
  if (fault === 'atlas-path') scene.atlas.path = architecture.scenes.DANGER.scenery.atlases[0]!.path
  if (fault === 'atlas-bytes') scene.atlas.bytes++
  if (fault === 'frame') scene.frame_rects.stage.x++
  if (fault === 'handoff') p.handoff.sha256 = 'sha256:FOREIGN'
  if (fault === 'source-revision') p.source_revision = 'FOREIGN'
  if (fault === 'cross-era-tuple') Object.assign(scene, structuredClone(d.scenes.DANGER))
  p.document_sha256 = hash(new TextEncoder().encode(JSON.stringify(d)))
  expect(() => validateSpatialV5Cozy(d,p)).toThrow()
  await expect(verifySpatialV5Cozy(d,p)).rejects.toThrow()
})
it('rejects foreign or cross-era architecture, frame or placement input rather than silently repainting it', () => {
  for (const change of ['frame','feet','cross-era','ground'] as const) {
    const original = structuredClone(architecture.scenes.PARADE.scenery) as Scenery
    if (change === 'frame') Object.assign(original.objects[0]!.frame,{x:original.objects[0]!.frame.x+1})
    if (change === 'feet') Object.assign(original.objects[0]!,{y:original.objects[0]!.y+1})
    if (change === 'cross-era') Object.assign(original,{atlases:architecture.scenes.DANGER.scenery.atlases})
    if (change === 'ground') Object.assign(original.ground,{path:'world/foreign/ground.png'})
    expect(() => spatialV5CozyScenery('PARADE',original)).toThrow()
  }
})
