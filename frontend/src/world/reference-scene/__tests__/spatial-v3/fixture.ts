// TEST ONLY: proposed map4 and original unreviewed virtual records; no product admission.
import { referenceFixture } from '../fixture'
import source from '../../spatial-v3/source-inputs.json'
import tile from '../../spatial-v3/tile-pin.json'
import { spatialSceneIdentity, spatialThemeIds, getSpatialScenery, type SpatialThemeId } from '../../spatial-v3/model'
import { contentHash } from '../../../map/json'
import { createAssetResources } from '../../../map/resources'
import { createResourceByteCache } from '../../../map/resource-cache'

export async function spatialFixture(themeId: SpatialThemeId = 'PARADE') {
  const f = await referenceFixture(), scenery = getSpatialScenery(themeId)
  f.map.map_revision = spatialSceneIdentity.mapRevision; f.map.geometry = structuredClone(source.geometry)
  f.map.geometry_hash = source.geometry_hash; f.map.places = structuredClone(source.places)
  f.map.tilemap = { format: 'TILED_JSON', resource_state: 'PROVIDED', path: tile.path, sha256: tile.sha256 }
  f.map.content_hash = await contentHash(f.map)
  f.theme.theme_id = themeId; f.theme.theme_revision = 6; f.theme.map_revision = f.map.map_revision
  f.theme.map_content_hash = f.map.content_hash; f.theme.geometry_hash = f.map.geometry_hash; f.theme.collision_id = f.map.geometry.collision_id
  f.theme.texture = { image_state: 'PROVIDED', path: scenery.ground.path, sha256: scenery.ground.sha256, width_px: 768, height_px: 1296 }
  f.theme.scenery = structuredClone(scenery); f.theme.content_hash = await contentHash(f.theme)
  const t = JSON.parse(new TextDecoder().decode(f.tiled))
  t.layers.forEach((layer: { data: number[] }, i: number) => { layer.data = [f.map.geometry.terrain_layer, f.map.geometry.collision_grid, f.map.geometry.overdraw_layer][i]!.flat() })
  return { ...f, tiled: new TextEncoder().encode(JSON.stringify(t)+'\n'), tileResource: { path: tile.path, sha256: tile.sha256, kind: 'json' as const },
    pin: { mapHash: f.map.content_hash, themeHash: f.theme.content_hash, avatar: f.avatar } }
}
export function spatialFixtureResources(origin: string, options: { signal?: AbortSignal; fetch?: typeof fetch } = {}) {
  const cache = createResourceByteCache(options.signal ? { signal: options.signal } : {})
  const images = spatialThemeIds.flatMap(id => { const s = getSpatialScenery(id); return [s.ground, ...s.atlases] })
  const unique = [...new Map(images.map(i => [i.path, { path: i.path, sha256: i.sha256, kind: 'png' as const }])).values()]
  const resources = createAssetResources({ origin, allow: [{ path: tile.path, sha256: tile.sha256, kind: 'json' }, ...unique], cache,
    ...(options.fetch ? { fetch: options.fetch } : {}) })
  return { resources, dispose: cache.dispose, diagnostics: cache.diagnostics }
}
