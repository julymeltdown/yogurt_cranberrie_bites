import { canonicalJson, requireMap } from '../map/json'
import { requireProvided, validateManifests } from '../map/manifest'
import type { LoadedMap } from '../map/resources'
import { validateLoadedScenery } from '../illustrated/model'

/** A skin cannot revise the active world, its interaction semantics, or avatar release. */
export function assertSameWorld(current: LoadedMap, next: LoadedMap) {
  // C06 requires the same immutable map release across all three themes.
  requireMap(canonicalJson(current.map) === canonicalJson(next.map)
    && current.widthPx === next.widthPx && current.heightPx === next.heightPx
    && canonicalJson(current.spawn) === canonicalJson(next.spawn))
  for (const key of ['avatar_manifest_id', 'avatar_manifest_version', 'avatar_content_hash'] as const) {
    requireMap(current.theme[key] === next.theme[key])
  }
  if (current.theme.scenery && next.theme.scenery) {
    // Atlas identifiers and packed source rectangles belong to each era's art.
    // Preserve ordered landmark identity, destination geometry and foot ordering.
    const placement = (objects: NonNullable<LoadedMap['theme']['scenery']>['objects']) =>
      objects.map(({ id, x, y, width, height, depth_y }) => ({ id, x, y, width, height, depth_y }))
    requireMap(canonicalJson(placement(current.theme.scenery.objects)) === canonicalJson(placement(next.theme.scenery.objects)))
  }
}

export async function validateReplacement(current: LoadedMap, next: LoadedMap) {
  assertSameWorld(current, next)
  requireProvided(next.map, next.theme)
  await validateManifests(JSON.stringify(next.map), JSON.stringify(next.theme), {
    mapHash: current.map.content_hash, themeHash: next.theme.content_hash,
    avatar: { manifest_id: current.theme.avatar_manifest_id, manifest_version: current.theme.avatar_manifest_version,
      content_hash: current.theme.avatar_content_hash },
  })
  requireMap(next.image.naturalWidth === next.theme.texture.width_px && next.image.naturalHeight === next.theme.texture.height_px)
  validateLoadedScenery(next)
}
