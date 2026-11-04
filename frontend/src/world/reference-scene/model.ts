import source from './source-inputs.json'
import { canonicalJson, requireMap } from '../map/json'
import type { LoadedMap } from '../map/resources'
import { validateLoadedScenery, type Scenery, type SceneryObject, type LoadedScenery } from '../illustrated/model'
import { referenceSceneIdentity } from './metadata'
import { isSpatialReferenceClaim, validateSpatialMetadata, requireSpatialScene } from './spatial-v3/model'
import { isSpatialV4ReferenceClaim, validateSpatialV4Metadata, requireSpatialV4Scene } from './spatial-v4/model'

import { isSpatialV5ReferenceClaim, validateSpatialV5Metadata, requireSpatialV5Scene } from './spatial-v5/model'

/** Source and packed metadata are immutable public inputs, not release admission authority. */
function freeze<T>(value: T): T {
  if (value && typeof value === 'object') { for (const child of Object.values(value)) freeze(child); Object.freeze(value) }
  return value
}
export const referenceScenery: Scenery = freeze(structuredClone(source.scenery)) as Scenery
const expectedGeometry = canonicalJson(source.geometry), expectedScenery = canonicalJson(referenceScenery)
const replaced = new Set(source.overview_representations.flatMap(p => 'replaces_prop_id' in p ? [p.replaces_prop_id] : []))
const frames = source.frames as Readonly<Record<string, Readonly<{ x: number; y: number; width: number; height: number }>>>
const overview: readonly SceneryObject[] = freeze([
  ...referenceScenery.objects.filter(o => !replaced.has(o.id)),
  ...source.overview_representations.map(p => ({ id: p.id, atlas_id: referenceScenery.atlases[0]!.id,
    frame: frames[p.frame]!, x: p.x, y: p.y, width: p.width, height: p.height, depth_y: p.depth_y })),
])
const plaza = freeze(source.plaza_parcel.map(p => [p[0]!, p[1]!] as const))
const worldCamera = freeze({ ...source.world_camera })

export interface ReferenceSceneBinding {
  readonly loaded: LoadedScenery
  readonly overview: readonly SceneryObject[]
  readonly plaza: readonly (readonly [number, number])[]
  readonly worldCamera: Readonly<{ x: number; y: number; width: number; height: number }>
}

/** A matching hash label alone cannot pair these pixels with unrelated collision or visual metadata. */
export function validateReferenceMetadata(map: Pick<LoadedMap, 'map' | 'theme' | 'widthPx' | 'heightPx' | 'spawn'>) {
  if (isSpatialV5ReferenceClaim(map)) { validateSpatialV5Metadata(map); return }
  if (isSpatialV4ReferenceClaim(map)) { validateSpatialV4Metadata(map); return }
  if (isSpatialReferenceClaim(map)) { validateSpatialMetadata(map); return }
  const expected = referenceSceneIdentity
  requireMap(map.map.map_id === expected.mapId && map.map.map_revision === expected.mapRevision
    && map.map.geometry_hash === expected.geometryHash && canonicalJson(map.map.geometry) === expectedGeometry
    && map.widthPx === expected.width && map.heightPx === expected.height
    && map.theme.theme_id === expected.themeId && map.theme.map_id === map.map.map_id
    && map.theme.map_revision === map.map.map_revision && map.theme.map_content_hash === map.map.content_hash
    && map.theme.geometry_hash === expected.geometryHash && map.theme.collision_id === source.geometry.collision_id
    && map.theme.scenery && canonicalJson(map.theme.scenery) === expectedScenery
    && canonicalJson(map.spawn) === canonicalJson(source.geometry.spawns.find(s => s.spawn_id === source.geometry.default_spawn_id)))
  requireMap(map.map.places.length === source.places.length && source.places.every(p => map.map.places.some(actual => actual.place_id === p.place_id)))
}

export function requireReferenceScene(map: LoadedMap): ReferenceSceneBinding {
  if (isSpatialV5ReferenceClaim(map)) return requireSpatialV5Scene(map)
  if (isSpatialV4ReferenceClaim(map)) return requireSpatialV4Scene(map)
  if (isSpatialReferenceClaim(map)) return requireSpatialScene(map)
  validateReferenceMetadata(map); validateLoadedScenery(map)
  requireMap(map.scenery && map.image.naturalWidth === map.theme.texture.width_px && map.image.naturalHeight === map.theme.texture.height_px)
  return Object.freeze({ loaded: map.scenery, overview, plaza, worldCamera })
}

/** Unknown ordinary releases retain their incumbent path; malformed/stale reference candidates fail closed. */
export function referenceSceneFor(map: LoadedMap): ReferenceSceneBinding | null {
  const candidate = isSpatialV5ReferenceClaim(map) || isSpatialV4ReferenceClaim(map) || isSpatialReferenceClaim(map) || map.map.map_id === referenceSceneIdentity.mapId || map.map.geometry_hash === referenceSceneIdentity.geometryHash
    || [map.theme.scenery?.ground.path, ...map.theme.scenery?.atlases.map(a => a.path) ?? []].some(p => p?.startsWith('world/reference-scene-v2/runtime-v4/'))
  return candidate ? requireReferenceScene(map) : null
}
