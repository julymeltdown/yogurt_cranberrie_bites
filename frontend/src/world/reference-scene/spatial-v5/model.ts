import input from './source-inputs.json'
import landscapeInput from './map-landscape-source.json'
import labelInput from './map-labels-source.json'
import labelPin from './map-labels-pins.json'
import landscapePin from './map-landscape-pin.json'
import tile from './tile-pin.json'
import { spatialV5ArchitectureScenery } from './architecture'
import { canonicalJson, requireMap, sha256 } from '../../map/json'
import type { LoadedMap } from '../../map/resources'
import { validateLoadedScenery, type Scenery, type SceneryObject } from '../../illustrated/model'
import type { ReferenceSceneBinding } from '../model'
import type { ReferenceOverviewIdentity, ReferenceOverviewLabel } from '../metadata'

export const spatialV5SceneIdentity = Object.freeze({ sourceRevision: input.source_revision,
  mapId: input.map_id, minimumMapRevision: input.minimum_map_revision, geometryHash: input.geometry_hash, width: 768, height: 1296 })
export const spatialV5ThemeIds = Object.freeze(['REVENGE', 'PARADE', 'DANGER'] as const)
export type SpatialV5ThemeId = typeof spatialV5ThemeIds[number]
export type SceneMetadata = Pick<LoadedMap, 'map' | 'theme' | 'widthPx' | 'heightPx' | 'spawn'>
function freeze<T>(value: T): T {
  if (value && typeof value === 'object') { for (const child of Object.values(value)) freeze(child); Object.freeze(value) }
  return value
}
const source = freeze(structuredClone(input)), landscape = freeze(landscapeInput), labels = freeze(labelInput)

/** Display-only sidecars must form one pinned pair before metadata or rendering is exposed. */
export function validateSpatialV5MapLayoutPair(label = labels, plants = landscape, labelPins = labelPin, plantPins = landscapePin) {
  requireMap(label?.source_layout_version === 'map-layout-v2' && plants?.source_layout_version === 'map-layout-v2'
    && label.geometry_hash === source.geometry_hash && plants.geometry_hash === source.geometry_hash
    && label.view === 'MAP_ONLY' && plants.view === 'MAP_ONLY'
    && label.dimensions.width === 768 && label.dimensions.height === 1296
    && plants.dimensions.width === 768 && plants.dimensions.height === 1296
    && labelPins.source_revision === plantPins.source_revision && labelPins.source_revision === labelPin.source_revision
    && labelPins.document_sha256 === labelPin.document_sha256 && plantPins.document_sha256 === landscapePin.document_sha256
    && canonicalJson(labelPins.layout_pair) === canonicalJson(labelPin.layout_pair)
    && canonicalJson(labelPins.layout_pair) === canonicalJson(plantPins.layout_pair)
    && canonicalJson(plants.required_label_presentation) === canonicalJson(labelPins.layout_pair.labels)
    && labelPins.presentation.path === labelPins.layout_pair.labels.path.replace('frontend/public/', '')
    && labelPins.presentation.sha256 === labelPins.layout_pair.labels.sha256
    && labelPins.presentation.bytes === labelPins.layout_pair.labels.bytes
    && plantPins.path === plantPins.layout_pair.landscape.path.replace('frontend/public/', '')
    && plantPins.sha256 === plantPins.layout_pair.landscape.sha256 && plantPins.bytes === plantPins.layout_pair.landscape.bytes
    && label.labels.length === source.map_labels.length && plants.decorations.length === 10)
  label.labels.forEach((item, index) => {
    const original = source.map_labels[index]!, r = item.rect
    requireMap(item.label === original.label && item.interaction_id === original.interaction_id
      && [item.x, item.y, r.x, r.y, r.width, r.height].every(Number.isSafeInteger)
      && r.width > 0 && r.height > 0 && r.x >= 0 && r.y >= 0 && r.x + r.width <= 768 && r.y + r.height <= 1296
      && Math.abs(item.x - r.x - r.width / 2) <= .5 && Math.abs(item.y - r.y - r.height / 2) <= .5)
  })
  const ids = new Set<string>()
  for (const item of plants.decorations) {
    requireMap(item.view === 'MAP_ONLY' && item.interaction_id === null && item.frame === 'shrub_bed'
      && item.id.startsWith('map_garden_') && !ids.has(item.id)
      && [item.x, item.y, item.width, item.height, item.depth_y].every(Number.isSafeInteger)
      && item.x >= 0 && item.y >= 0 && item.width > 0 && item.height > 0
      && item.x + item.width <= 768 && item.y + item.height <= 1296 && item.depth_y === item.y + item.height)
    ids.add(item.id)
  }
}
export async function verifySpatialV5MapLayoutPair(label = labels, plants = landscape, labelPins = labelPin, plantPins = landscapePin) {
  validateSpatialV5MapLayoutPair(label, plants, labelPins, plantPins)
  const hash = (value: unknown) => sha256(new TextEncoder().encode(JSON.stringify(value)))
  requireMap(await hash(label) === labelPins.document_sha256 && await hash(plants) === plantPins.document_sha256)
}
validateSpatialV5MapLayoutPair()
const scenes = new Map(spatialV5ThemeIds.map(themeId => {
  const data = source.scenes[themeId], baseScenery = data.scenery as Scenery
  requireMap(canonicalJson(landscape.themes.find(t => t.theme_id === themeId)?.atlas) === canonicalJson(baseScenery.atlases[0]))
  const scenery = freeze(spatialV5ArchitectureScenery(themeId, baseScenery))
  const replacements = new Set(source.overview_representations.flatMap(p => 'replaces_prop_id' in p ? [p.replaces_prop_id] : []))
  const frames = data.frames as Readonly<Record<string, Readonly<{ x: number; y: number; width: number; height: number }>>>
  const overview: readonly SceneryObject[] = freeze([...scenery.objects.filter(o => !replacements.has(o.id)),
    ...source.overview_representations.map(p => ({ id: p.id, atlas_id: scenery.atlases[0]!.id, frame: frames[p.frame]!,
      x: p.x, y: p.y, width: p.width, height: p.height, depth_y: p.depth_y })),
    ...landscape.decorations.map(p => ({ id: p.id, atlas_id: scenery.atlases[0]!.id, frame: frames[p.frame]!,
      x: p.x, y: p.y, width: p.width, height: p.height, depth_y: p.depth_y }))])
  const mapLabels: readonly ReferenceOverviewLabel[] = freeze(labels.labels.map(p => ({ label: p.label, x: p.x, y: p.y, interactionId: p.interaction_id })))
  return [themeId, Object.freeze({ scenery, overview, expected: canonicalJson(scenery),
    metadata: Object.freeze({ ...spatialV5SceneIdentity, themeId, mapLabels }) })] as const
}))
const geometry = canonicalJson(source.geometry), places = canonicalJson(source.places)
const plaza = freeze(source.plaza_parcel.map(p => [p[0]!, p[1]!] as const)), worldCamera = freeze({ ...source.world_camera })

export function getSpatialV5OverviewMetadata(identity: ReferenceOverviewIdentity) {
  const scene = scenes.get(identity.themeId as SpatialV5ThemeId)
  return scene && identity.geometryHash === spatialV5SceneIdentity.geometryHash
    && identity.width === 768 && identity.height === 1296
    && identity.mapId === spatialV5SceneIdentity.mapId && Number.isSafeInteger(identity.mapRevision)
    && identity.mapRevision! >= spatialV5SceneIdentity.minimumMapRevision
    ? Object.freeze({ ...scene.metadata, mapRevision: identity.mapRevision! }) : null
}
export function getSpatialV5Scenery(themeId: SpatialV5ThemeId): Scenery { return scenes.get(themeId)!.scenery }
/** Any new source claim is fenced; an old or incomplete scene is never an allowed fallback. */
export function isSpatialV5ReferenceClaim(value: Pick<SceneMetadata, 'map' | 'theme'>) {
  return value.map.geometry_hash === spatialV5SceneIdentity.geometryHash
    || value.map.tilemap.path?.startsWith('world/reference-scene-v2/runtime/spatial-v5/') === true
    || [value.theme.scenery?.ground.path, ...value.theme.scenery?.atlases.map(a => a.path) ?? []]
      .some(path => path?.startsWith('world/reference-scene-v2/quality-v10/') || path?.startsWith('world/reference-scene-v2/architecture-v2/'))
}
export function validateSpatialV5Metadata(value: SceneMetadata) {
  const { map, theme } = value, scene = scenes.get(theme.theme_id as SpatialV5ThemeId), expected = spatialV5SceneIdentity
  requireMap(scene && map.map_id === expected.mapId && Number.isSafeInteger(map.map_revision) && map.map_revision >= expected.minimumMapRevision
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
export function requireSpatialV5Scene(map: LoadedMap): ReferenceSceneBinding {
  validateSpatialV5Metadata(map); validateLoadedScenery(map)
  requireMap(map.scenery && map.image.naturalWidth === map.theme.texture.width_px && map.image.naturalHeight === map.theme.texture.height_px)
  return Object.freeze({ loaded: map.scenery, overview: scenes.get(map.theme.theme_id as SpatialV5ThemeId)!.overview, plaza, worldCamera })
}
