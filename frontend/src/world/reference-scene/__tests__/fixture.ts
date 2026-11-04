// TEST ONLY coherent C06 fixture: genuine source geometry/pixels; no bootstrap or production admission.
import mapBase from '../../../../../contracts/fixtures/assets/map-manifest.json'
import themeBase from '../../../../../contracts/fixtures/assets/theme-revenge.json'
import avatar from '../../../../../contracts/fixtures/assets/selected-static/avatar-manifest.json'
import proposal from '../source-inputs.json'
import type { MapManifest, ThemeManifest } from '../../map/manifest'
import type { AssetResources, Resource } from '../../map/resources'
import { createAssetResources } from '../../map/resources'
import { createResourceByteCache } from '../../map/resource-cache'
import { contentHash, sha256 } from '../../map/json'
import { THEME_IMAGE_MARKER } from '../../map/tiled'
import { referenceScenery } from '../model'
import { referenceSceneIdentity } from '../metadata'

export async function referenceFixture() {
  const g = structuredClone(proposal.geometry), texture = referenceScenery.ground
  const tiled = new TextEncoder().encode(JSON.stringify({ type: 'map', version: '1.10', tiledversion: '1.10.2',
    orientation: 'orthogonal', renderorder: 'right-down', infinite: false, width: g.width_cells, height: g.height_cells,
    tilewidth: g.tile_width_px, tileheight: g.tile_height_px,
    layers: [['terrain', g.terrain_layer], ['collision', g.collision_grid], ['overdraw', g.overdraw_layer]].map(([name, grid], i) => ({
      id: i + 1, name, type: 'tilelayer', width: g.width_cells, height: g.height_cells, x: 0, y: 0, opacity: 1, visible: true, data: (grid as number[][]).flat(),
    })),
    tilesets: [{ firstgid: 1, name: 'theme', image: THEME_IMAGE_MARKER, imagewidth: texture.width_px, imageheight: texture.height_px,
      tilewidth: 16, tileheight: 16, tilecount: texture.width_px * texture.height_px / 256, columns: texture.width_px / 16, margin: 0, spacing: 0 }],
  })+'\n')
  const tileResource: Resource = { path: 'world/reference-scene-v2/runtime/canonical-map-v2.json', sha256: await sha256(tiled), kind: 'json' }
  const map: MapManifest = { ...structuredClone(mapBase), map_id: referenceSceneIdentity.mapId, map_revision: referenceSceneIdentity.mapRevision,
    geometry: g, geometry_hash: proposal.geometry_hash, places: structuredClone(proposal.places), provenance: structuredClone(proposal.places[0]!.provenance),
    tilemap: { format: 'TILED_JSON', resource_state: 'PROVIDED', path: tileResource.path, sha256: tileResource.sha256 } }
  map.content_hash = await contentHash(map)
  const theme: ThemeManifest = { ...structuredClone(themeBase), theme_id: 'PARADE', theme_revision: 1,
    map_id: map.map_id, map_revision: referenceSceneIdentity.mapRevision, map_content_hash: map.content_hash, geometry_hash: map.geometry_hash,
    collision_id: g.collision_id, place_ids: map.places.map(p => p.place_id), interaction_ids: g.interactions.map(i => i.interaction_id),
    avatar_manifest_id: avatar.manifest_id, avatar_manifest_version: 1, avatar_content_hash: avatar.content_hash,
    texture: { image_state: 'PROVIDED', path: texture.path, sha256: texture.sha256, width_px: texture.width_px, height_px: texture.height_px },
    scenery: structuredClone(referenceScenery), provenance: structuredClone(map.provenance) }
  theme.content_hash = await contentHash(theme)
  return { map, theme, tiled, tileResource, avatar,
    pin: { mapHash: map.content_hash, themeHash: theme.content_hash, avatar } }
}

export function referenceFixtureResources(fixture: Awaited<ReturnType<typeof referenceFixture>>, origin: string,
  options: { fetch?: typeof fetch; signal?: AbortSignal } = {}) {
  const cache = createResourceByteCache(options.signal ? { signal: options.signal } : {})
  const pngs = [referenceScenery.ground, ...referenceScenery.atlases].map(t => ({ path: t.path, sha256: t.sha256, kind: 'png' as const }))
  const files = createAssetResources({ origin, allow: [fixture.tileResource, ...pngs], cache, ...(options.fetch ? { fetch: options.fetch } : {}) })
  const resources: AssetResources = { async read(resource, signal) {
    if (resource.path !== fixture.tileResource.path) return files.read(resource, signal)
    if (signal.aborted || resource.kind !== 'json' || resource.sha256 !== fixture.tileResource.sha256) throw Error('Fixture resource unavailable')
    return files.read(resource, signal)
  } }
  return { resources, dispose: cache.dispose, diagnostics: cache.diagnostics }
}
