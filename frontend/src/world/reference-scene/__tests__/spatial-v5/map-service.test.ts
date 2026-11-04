import { expect, it } from 'vitest'
import { readFile } from 'node:fs/promises'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import source from '../../spatial-v5/source-inputs.json'
import architecture from '../../spatial-v5/architecture-source.json'
import plants from '../../spatial-v5/map-landscape-source.json'
import document from '../../spatial-v5/map-service-source.json'
import pins from '../../spatial-v5/map-service-pins.json'
import { getSpatialV5ServiceAnnotations, getSpatialV5ServicePresentation, spatialV5ServiceOverview, validateSpatialV5MapService, verifySpatialV5MapService } from '../../spatial-v5/map-service'
import { getSpatialV5Scenery, spatialV5SceneIdentity, spatialV5ThemeIds } from '../../spatial-v5/model'
import type { SceneryObject } from '../../../illustrated/model'

const hash = (raw: Uint8Array) => 'sha256:' + createHash('sha256').update(raw).digest('hex')
it('reproduces exactly two new files and closes producer, base, architecture and effective tuple bytes', async () => {
  const files = ['src/world/reference-scene/spatial-v5/map-service-source.json', 'src/world/reference-scene/spatial-v5/map-service-pins.json']
  const before = await Promise.all(files.map(p => readFile(p)))
  execFileSync(process.env.MCR_FE14_PYTHON ?? 'python3', ['-B', 'frontend/src/world/reference-scene/spatial-v5/build-map-service-inputs.py'], { cwd: '..' })
  const after = await Promise.all(files.map(p => readFile(p)))
  expect(after).toEqual(before)
  for (const pin of [pins.handoff, ...pins.outputs, ...pins.frozen_inputs]) {
    const raw = await readFile('../' + pin.path); expect(raw.length, pin.path).toBe(pin.bytes); expect(hash(raw), pin.path).toBe(pin.sha256)
  }
  await expect(verifySpatialV5MapService()).resolves.toBeUndefined()
})
it.each(spatialV5ThemeIds)('preserves %s World, all48 other MAP objects, placement of the explicit neutral replacement and two wings', era => {
  const scenery = getSpatialV5Scenery(era)
  expect(scenery).toEqual(architecture.scenes[era].scenery)
  const replaced = new Set(source.overview_representations.flatMap(p => 'replaces_prop_id' in p ? [p.replaces_prop_id] : []))
  const frames = source.scenes[era].frames
  const original: SceneryObject[] = [...scenery.objects.filter(o => !replaced.has(o.id)),
    ...[...source.overview_representations, ...plants.decorations].map(o => ({ id: o.id, atlas_id: scenery.atlases[0]!.id,
      frame: frames[o.frame as keyof typeof frames], x: o.x, y: o.y, width: o.width, height: o.height, depth_y: o.depth_y }))]
  expect(original).toHaveLength(49)
  const next = spatialV5ServiceOverview(era, original, scenery.atlases[0]!.id)
  expect(next).toHaveLength(51)
  original.forEach((o, i) => {
    if (o.id === 'food_counters') expect(next[i]).toEqual({ ...o, id: 'map_feed_seating_representation', frame: frames.plaza_furniture })
    else expect(next[i]).toBe(o)
  })
  expect(next.slice(49).map(o => ({ ...o, frame: undefined, atlas_id: undefined }))).toEqual(document.annexes.map(o => ({
    id: o.id, x: o.x, y: o.y, width: o.width, height: o.height, depth_y: o.depth_y, frame: undefined, atlas_id: undefined,
  })))
  expect(scenery.objects).toHaveLength(34)
  expect(next.find(o => o.id === 'stage_venue_representation')).toMatchObject({ x:185, y:163, width:300, height:210, depth_y:373 })
})
it('exposes only four fixed virtual destinations and no canonical annotation bindings', () => {
  const identity = { ...spatialV5SceneIdentity, mapRevision: 7005, themeId: 'PARADE' }
  expect(getSpatialV5ServiceAnnotations(identity).map(p => [p.label, p.path])).toEqual([
    ['WALL', '/wall'], ['FEED', '/feed'], ['MEET', '/meet'], ['GIFTS', '/gifts'],
  ])
  const presentation = getSpatialV5ServicePresentation()
  expect(presentation.labels.every(p => p.interaction_id === null && p.place_id === null)).toBe(true)
  expect(Object.isFrozen(presentation.labels[0]!.rect)).toBe(true)
  for (const patch of [{ geometryHash:'sha256:FOREIGN' }, { mapId:'foreign' }, { mapRevision:0 }, { themeId:'foreign' }, { width:0 }])
    expect(getSpatialV5ServiceAnnotations({ ...identity, ...patch })).toEqual([])
})
it.each(['geometry', 'base', 'legacy-label', 'foreign-route', 'canonical-place', 'interaction', 'rect', 'glyph', 'annex', 'food-frame', 'food-foot', 'world-authority'] as const)('rejects %s even with a recomputed document digest', async fault => {
  const d = structuredClone(document), pin = structuredClone(pins)
  if (fault === 'geometry') d.geometry_hash = 'sha256:FOREIGN'
  if (fault === 'base') d.base_source_revision = 'foreign'
  if (fault === 'legacy-label') d.presentation.labels[2]!.label = 'MERCH'
  if (fault === 'foreign-route') Object.assign(d.presentation.labels[2]!, { navigation_intent:'https://foreign.invalid' })
  if (fault === 'canonical-place') Object.assign(d.presentation.labels[2]!, { place_id:source.places[0]!.place_id })
  if (fault === 'interaction') Object.assign(d.presentation.labels[2]!, { interaction_id:'gifts_info' })
  if (fault === 'rect') d.presentation.labels[0]!.rect.x++
  if (fault === 'glyph') d.presentation.icons.wall[0]![0]![0]!++
  if (fault === 'annex') d.annexes[0]!.x++
  if (fault === 'food-frame') d.display_replacements.PARADE.frame.x++
  if (fault === 'food-foot') d.display_replacements.PARADE.depth_y++
  if (fault === 'world-authority') d.production_status = 'ADMITTED'
  pin.document_sha256 = hash(new TextEncoder().encode(JSON.stringify(d)))
  expect(() => validateSpatialV5MapService(d, pin)).toThrow()
  await expect(verifySpatialV5MapService(d, pin)).rejects.toThrow()
})
