import { beforeEach, expect, it, vi } from 'vitest'
import { webcrypto } from 'node:crypto'
import map from '../../../../../contracts/fixtures/assets/map-manifest.json'
import valid from '../../../../../contracts/fixtures/assets/illustrated/valid-theme.json'
import unknown from '../../../../../contracts/fixtures/assets/illustrated/invalid-unknown-field.json'
import oversized from '../../../../../contracts/fixtures/assets/illustrated/invalid-texture-limit.json'
import { schemaValid } from '../../map/schema'
import { validateManifests } from '../../map/manifest'
import { contentHash } from '../../map/json'
import { sceneryDepth, sceneryResources } from '../../illustrated/model'
beforeEach(() => vi.stubGlobal('crypto', webcrypto))
const pin = { mapHash: map.content_hash, themeHash: valid.content_hash, avatar: {
  manifest_id: valid.avatar_manifest_id, manifest_version: valid.avatar_manifest_version, content_hash: valid.avatar_content_hash } }
it('accepts an optional closed scenery fixture without changing required v1 bindings', async () => {
  expect(schemaValid(valid, 'theme')).toBe(true)
  const result = await validateManifests(JSON.stringify(map), JSON.stringify(valid), pin)
  expect(sceneryResources(result.theme)).toHaveLength(2)
  expect(Object.isFrozen(result.theme.scenery?.objects)).toBe(true)
  expect(sceneryDepth(32, 160)).toBe(1 + 32 / 161 * .9)
  expect(sceneryDepth(-1, 160)).toBe(.5)
})
it.each([unknown, oversized])('rejects a rehashed closed-schema/maximum violation', value => {
  expect(schemaValid(value, 'theme')).toBe(false)
})
it('enforces maxItems in the checked-in schema evaluator', () => {
  const t = structuredClone(valid); t.scenery.objects = Array.from({ length: 97 }, () => t.scenery.objects[0]!)
  expect(schemaValid(t, 'theme')).toBe(false)
})
it.each(['aggregate-pixels', 'aggregate-bytes', 'frame', 'duplicate-id', 'duplicate-path', 'world-bound', 'ground-size', 'missing-atlas'])(
  'rejects semantically invalid %s before resources can be requested', async kind => {
    const t = structuredClone(valid), s = t.scenery
    if (kind === 'aggregate-pixels') s.atlases[0]!.width_px = s.atlases[0]!.height_px = 2048
    if (kind === 'aggregate-bytes') { s.ground.bytes = 2_000_000; s.atlases[0]!.bytes = 2_000_000 }
    if (kind === 'frame') s.objects[0]!.frame.x = 1
    if (kind === 'duplicate-id') s.objects.push(structuredClone(s.objects[0]!))
    if (kind === 'duplicate-path') s.atlases[0]!.path = s.ground.path
    if (kind === 'world-bound') s.objects[0]!.x = 200
    if (kind === 'ground-size') s.ground.width_px++
    if (kind === 'missing-atlas') s.objects[0]!.atlas_id = 'absent'
    t.content_hash = await contentHash(t)
    await expect(validateManifests(JSON.stringify(map), JSON.stringify(t), { ...pin, themeHash: t.content_hash })).rejects.toMatchObject({ reason: 'invalid' })
  })
