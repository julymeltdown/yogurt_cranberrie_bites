// @vitest-environment jsdom
import { expect, it } from 'vitest'
import { sceneData } from './data'
import { deriveFixtureThemes } from './qualified-themes'
import { validateManifests } from '../../../../world/map/manifest'
import { canonicalJson, contentHash } from '../../../../world/map/json'
import { spatialFixture } from '../../../../world/reference-scene/__tests__/spatial-v3/fixture'
import { spatialThemeIds } from '../../../../world/reference-scene'
import { preflightReferenceScene } from '../../reference-scene-overview'

it('packages new fixture descriptors from actual source qualification, validating full map/avatar/C06/hash bindings without altering originals', async () => {
  const original = await sceneData(), derived = await sceneData('spatial', 'PARADE', true)
  expect(derived.map).toEqual(original.map); expect(derived.theme).toEqual(original.theme)
  expect(derived.originalThemeProvenance.every(p => p.review_state === 'UNREVIEWED')).toBe(true)
  expect(derived.map.places).toEqual(original.map.places)
  for (const theme of derived.bootstrap.theme_manifests) {
    expect(theme.provenance).toMatchObject({ review_state: 'REVIEWED', reviewed_by: 'MCR backend spatial source qualification' })
    expect(theme.content_hash).toBe(await contentHash(theme))
    const metadata = await validateManifests(canonicalJson(derived.map), canonicalJson(theme), { ...derived.pin, themeHash: theme.content_hash })
    expect(() => preflightReferenceScene(metadata)).not.toThrow()
    expect(theme.map_content_hash).toBe(derived.map.content_hash); expect(theme.avatar_content_hash).toBe(derived.avatar.content_hash)
  }
})
it.each(['resource', 'geometry', 'avatar', 'scenery', 'missingEra'] as const)('rejects derived %s changes rather than borrowing source review for unrelated fields', async kind => {
  const f = await spatialFixture(), themes = await Promise.all(spatialThemeIds.map(async id => (await spatialFixture(id)).theme))
  if (kind === 'resource') themes[0]!.texture.sha256 = 'sha256:' + '0'.repeat(64)
  if (kind === 'geometry') f.map.geometry_hash = 'sha256:' + '0'.repeat(64)
  if (kind === 'avatar') themes[0]!.avatar_content_hash = 'sha256:' + '0'.repeat(64)
  if (kind === 'scenery') themes[0]!.scenery = { ...themes[0]!.scenery!, objects: themes[0]!.scenery!.objects.map((o, i) => i ? o : { ...o, x: o.x + 1 }) }
  if (kind === 'missingEra') themes.pop()
  await expect(deriveFixtureThemes(f.map, themes, f.avatar)).rejects.toThrow()
})
