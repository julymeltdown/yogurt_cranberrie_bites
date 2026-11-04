import { spawnSync } from 'node:child_process'
import { expect, it, vi } from 'vitest'
import { spatialV5Fixture, syntheticMapRevision } from '../../../../world/reference-scene/__tests__/spatial-v5/fixture'
import { spatialV5SceneIdentity, spatialV5ThemeIds, getReferenceOverviewMetadata, loadReferenceScene } from '../../../../world/reference-scene'
import { contentHash } from '../../../../world/map/json'
import { validateManifests } from '../../../../world/map/manifest'
import { isReferenceSceneCandidate, preflightReferenceScene } from '../../reference-scene-overview'

it('passes the actual closed C06 bundle validator with the supported retained Python format environment', async () => {
  const fixtures = await Promise.all(spatialV5ThemeIds.map(id => spatialV5Fixture(id))), f = fixtures[0]!
  const documents = { map: f.map, avatar: f.avatar, config: f.avatar.presets[0]!.config,
    ...Object.fromEntries(fixtures.map(item => [item.theme.theme_id, item.theme])) }
  const program = "import runpy,json,sys;m=runpy.run_path('../scripts/mcr/validate-assets.py');print(json.dumps(m['validate_bundle'](json.loads(sys.stdin.read(),object_pairs_hook=m['reject_duplicates']))))"
  const result = spawnSync(process.env.MCR_FE14_PYTHON ?? 'python3', ['-c', program], { input: JSON.stringify(documents), encoding: 'utf8' })
  expect(result.status, result.stderr).toBe(0)
  expect(JSON.parse(result.stdout)).toMatchObject({ status: 'CONTRACT_READY', documents_validated: 6, themes: ['DANGER', 'PARADE', 'REVENGE'] })
  expect(f.map.map_revision).toBe(syntheticMapRevision)
  expect(f.map.provenance.review_state).toBe('UNREVIEWED')
})

it.each(spatialV5ThemeIds)('binds %s labels to the actual coherent map identity, never the source proposal number', async id => {
  const f = await spatialV5Fixture(id), metadata = await validateManifests(JSON.stringify(f.map), JSON.stringify(f.theme), f.pin)
  expect(isReferenceSceneCandidate(metadata)).toBe(true); expect(() => preflightReferenceScene(metadata)).not.toThrow()
  const identity = { geometryHash: f.map.geometry_hash, width: 768, height: 1296, themeId: id }
  expect(getReferenceOverviewMetadata(identity)).toBeNull()
  expect(getReferenceOverviewMetadata({ ...identity, mapId: f.map.map_id, mapRevision: 4 })).toBeNull()
  expect(getReferenceOverviewMetadata({ ...identity, mapId: 'foreign', mapRevision: syntheticMapRevision })).toBeNull()
  expect(getReferenceOverviewMetadata({ ...identity, mapId: f.map.map_id, mapRevision: f.map.map_revision }))
    .toMatchObject({ ...spatialV5SceneIdentity, mapRevision: syntheticMapRevision })
})

it.each(['foreign-map', 'old-map4', 'missing-root', 'reordered-depth', 'old-tile'] as const)('rejects %s at root preflight without resource/decode work', async fault => {
  const f = await spatialV5Fixture(), { map, theme } = f
  if (fault === 'foreign-map') { map.map_id = 'foreign'; theme.map_id = 'foreign' }
  if (fault === 'old-map4') { map.map_revision = 4; theme.map_revision = 4 }
  if (fault === 'missing-root') Object.assign(theme.scenery!, { objects: theme.scenery!.objects.slice(0, -1) })
  if (fault === 'reordered-depth') Object.assign(theme.scenery!.objects[33]!, { depth_y: theme.scenery!.objects[33]!.depth_y - 1 })
  if (fault === 'old-tile') map.tilemap.path = 'world/reference-scene-v2/runtime/spatial-v3/canonical-map-v4.json'
  map.content_hash = await contentHash(map); theme.map_content_hash = map.content_hash; theme.content_hash = await contentHash(theme)
  const pin = { ...f.pin, mapHash: map.content_hash, themeHash: theme.content_hash }
  const metadata = await validateManifests(JSON.stringify(map), JSON.stringify(theme), pin)
  expect(isReferenceSceneCandidate(metadata)).toBe(true); expect(() => preflightReferenceScene(metadata)).toThrow()
  const read = vi.fn(), decode = vi.fn()
  await expect(loadReferenceScene({ mapText: JSON.stringify(map), themeText: JSON.stringify(theme), pin, resources: { read }, decode }, new AbortController().signal)).rejects.toThrow()
  expect(read).not.toHaveBeenCalled(); expect(decode).not.toHaveBeenCalled()
})

it('projects only coherent v10 authored label obstacles; decorative labels remain unbound', async () => {
  const { spatialV5LabelObstacles } = await import('../../../../features/places/ui/reference-scene/spatial-v5-label-obstacles')
  const identity = { geometryHash: spatialV5SceneIdentity.geometryHash, width: 768, height: 1296, themeId: 'PARADE', mapId: spatialV5SceneIdentity.mapId, mapRevision: syntheticMapRevision }
  expect(spatialV5LabelObstacles({ ...identity, mapRevision: 4 }, 384, 648)).toEqual([])
  const boxes = spatialV5LabelObstacles(identity, 384, 648)
  expect(boxes).toHaveLength(6)
  expect(boxes.filter(b => b.interactionId !== null).map(b => b.interactionId)).toEqual([])
  expect(boxes[0]).toMatchObject({ left: 133, top: 81, width: 156, height: 39 })
})
