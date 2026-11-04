import input from './source-inputs.json'
import tile from './tile-pin.json'
import { canonicalJson, requireMap } from '../../map/json'
import type { LoadedMap } from '../../map/resources'
import { validateLoadedScenery, type Scenery, type SceneryObject } from '../../illustrated/model'
import type { ReferenceSceneBinding } from '../model'
import type { ReferenceOverviewIdentity, ReferenceOverviewLabel } from '../metadata'

export const spatialSceneIdentity = Object.freeze({ sourceRevision: input.source_revision,
  mapId: 'mcr_reference_scene_r3', mapRevision: 4, geometryHash: input.geometry_hash, width: 768, height: 1296 })
export const spatialThemeIds = Object.freeze(['REVENGE', 'PARADE', 'DANGER'] as const)
export type SpatialThemeId = typeof spatialThemeIds[number]
export type SceneMetadata = Pick<LoadedMap, 'map' | 'theme' | 'widthPx' | 'heightPx' | 'spawn'>
function freeze<T>(value: T): T {
  if (value && typeof value === 'object') { for (const child of Object.values(value)) freeze(child); Object.freeze(value) }
  return value
}
const source = freeze(structuredClone(input))
const scenes = new Map(spatialThemeIds.map(themeId => {
  const data = source.scenes[themeId], scenery = data.scenery as Scenery
  const replacements = new Set(source.overview_representations.flatMap(p => 'replaces_prop_id' in p ? [p.replaces_prop_id] : []))
  const frames = data.frames as Readonly<Record<string, Readonly<{ x: number; y: number; width: number; height: number }>>>
  const overview: readonly SceneryObject[] = freeze([...scenery.objects.filter(o => !replacements.has(o.id)),
    ...source.overview_representations.map(p => ({ id: p.id, atlas_id: scenery.atlases[0]!.id, frame: frames[p.frame]!,
      x: p.x, y: p.y, width: p.width, height: p.height, depth_y: p.depth_y }))])
  const mapLabels: readonly ReferenceOverviewLabel[] = freeze(source.map_labels.map(p => ({ label: p.label, x: p.x, y: p.y, interactionId: p.interaction_id })))
  return [themeId, Object.freeze({ scenery, overview, expected: canonicalJson(scenery),
    metadata: Object.freeze({ ...spatialSceneIdentity, themeId, mapLabels }) })] as const
}))
const geometry = canonicalJson(source.geometry), places = canonicalJson(source.places)
const plaza = freeze(source.plaza_parcel.map(p => [p[0]!, p[1]!] as const)), worldCamera = freeze({ ...source.world_camera })

export function getSpatialOverviewMetadata(identity: ReferenceOverviewIdentity) {
  const scene = scenes.get(identity.themeId as SpatialThemeId)
  return scene && identity.geometryHash === spatialSceneIdentity.geometryHash
    && identity.width === 768 && identity.height === 1296 ? scene.metadata : null
}
export function getSpatialScenery(themeId: SpatialThemeId): Scenery { return scenes.get(themeId)!.scenery }
/** Any new source claim is fenced; an old or incomplete scene is never an allowed fallback. */
export function isSpatialReferenceClaim(value: Pick<SceneMetadata, 'map' | 'theme'>) {
  return value.map.map_id === spatialSceneIdentity.mapId && value.map.map_revision === 4
    || value.map.geometry_hash === spatialSceneIdentity.geometryHash
    || value.map.tilemap.path?.startsWith('world/reference-scene-v2/runtime/spatial-v3/') === true
    || [value.theme.scenery?.ground.path, ...value.theme.scenery?.atlases.map(a => a.path) ?? []]
      .some(path => path?.startsWith('world/reference-scene-v2/quality-v6/'))
}
export function validateSpatialMetadata(value: SceneMetadata) {
  const { map, theme } = value, scene = scenes.get(theme.theme_id as SpatialThemeId), expected = spatialSceneIdentity
  requireMap(scene && map.map_id === expected.mapId && map.map_revision === expected.mapRevision
    && map.geometry_hash === expected.geometryHash && canonicalJson(map.geometry) === geometry
    && canonicalJson(map.places) === places && value.widthPx === expected.width && value.heightPx === expected.height
    && map.tilemap.resource_state === 'PROVIDED' && map.tilemap.path === tile.path && map.tilemap.sha256 === tile.sha256
    && theme.map_id === map.map_id && theme.map_revision === map.map_revision && theme.map_content_hash === map.content_hash
    && theme.geometry_hash === expected.geometryHash && theme.collision_id === source.geometry.collision_id
    && theme.texture.image_state === 'PROVIDED' && theme.texture.path === scene.scenery.ground.path
    && theme.texture.sha256 === scene.scenery.ground.sha256 && theme.texture.width_px === 768 && theme.texture.height_px === 1296
    && canonicalJson(theme.scenery) === scene.expected
    && canonicalJson(value.spawn) === canonicalJson(source.geometry.spawns.find(s => s.spawn_id === source.geometry.default_spawn_id)))
}
export function requireSpatialScene(map: LoadedMap): ReferenceSceneBinding {
  validateSpatialMetadata(map); validateLoadedScenery(map)
  requireMap(map.scenery && map.image.naturalWidth === map.theme.texture.width_px && map.image.naturalHeight === map.theme.texture.height_px)
  return Object.freeze({ loaded: map.scenery, overview: scenes.get(map.theme.theme_id as SpatialThemeId)!.overview, plaza, worldCamera })
}
