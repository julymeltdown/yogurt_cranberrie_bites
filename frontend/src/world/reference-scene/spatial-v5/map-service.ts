import serviceInput from './map-service-source.json'
import pins from './map-service-pins.json'
import base from './source-inputs.json'
import { canonicalJson, requireMap, sha256 } from '../../map/json'
import type { SceneryObject } from '../../illustrated/model'
import type { ReferenceOverviewIdentity } from '../metadata'

function freeze<T>(value: T): T {
  if (value && typeof value === 'object') { Object.values(value).forEach(freeze); Object.freeze(value) }
  return value
}
const service = freeze(structuredClone(serviceInput))

export const mapServiceRoutes = Object.freeze({ feed: '/feed', wall: '/wall', meet: '/meet', gifts: '/gifts' } as const)
export type MapServiceIntent = keyof typeof mapServiceRoutes

/** This independently pinned overlay changes presentation, never canonical place or World authority. */
export function validateSpatialV5MapService(document = service, resourcePins = pins) {
  requireMap(document.format === 'MCR_SPATIAL5_MAP_SERVICE_V1_INPUTS' && document.production_status === 'NOT_ADMITTED'
    && document.geometry_hash === base.geometry_hash && document.base_source_revision === base.source_revision
    && canonicalJson(resourcePins) === canonicalJson(pins) && canonicalJson(document) === canonicalJson(service))
  requireMap(document.presentation.labels.length === 6 && document.annexes.length === 2)
  const intents = new Set<string>()
  for (const label of document.presentation.labels) {
    requireMap(label.interaction_id === null && label.place_id === null)
    const intent = label.navigation_intent
    if (intent === null) requireMap(label.authority === 'VIRTUAL_SCENERY_ONLY')
    else {
      requireMap(Object.hasOwn(mapServiceRoutes, intent) && !intents.has(intent)
        && label.authority === 'VIRTUAL_FEATURE_NAVIGATION_ONLY' && label.label === intent.toUpperCase())
      intents.add(intent)
    }
  }
  requireMap(intents.size === 4)
}
export async function verifySpatialV5MapService(document = service, resourcePins = pins) {
  validateSpatialV5MapService(document, resourcePins)
  requireMap(await sha256(new TextEncoder().encode(JSON.stringify(document))) === resourcePins.document_sha256)
}
export function getSpatialV5ServicePresentation() { validateSpatialV5MapService(); return service.presentation }
export function spatialV5ServiceOverview(themeId: keyof typeof base.scenes, objects: readonly SceneryObject[], atlasId: string): readonly SceneryObject[] {
  validateSpatialV5MapService()
  const replacement = service.display_replacements[themeId]
  const original = objects.find(o => o.id === replacement.replaces_prop_id)
  requireMap(original && ['x', 'y', 'width', 'height', 'depth_y'].every(k => Reflect.get(original, k) === Reflect.get(replacement, k)))
  const convert = (row: { id: string; frame: { x: number; y: number; width: number; height: number }; x: number; y: number; width: number; height: number; depth_y: number }): SceneryObject => ({
    id: row.id, atlas_id: atlasId, frame: row.frame, x: row.x, y: row.y, width: row.width, height: row.height, depth_y: row.depth_y,
  })
  const frames = base.scenes[themeId].frames
  return [...objects.map(o => o === original ? convert(replacement) : o),
    ...service.annexes.map(row => convert({ ...row, frame: frames[row.frame as keyof typeof frames] }))]
}
export function getSpatialV5ServiceAnnotations(identity: ReferenceOverviewIdentity) {
  validateSpatialV5MapService()
  if (identity.geometryHash !== base.geometry_hash || identity.mapId !== base.map_id || !Number.isSafeInteger(identity.mapRevision)
    || identity.mapRevision! < base.minimum_map_revision || identity.width !== 768 || identity.height !== 1296
    || !Object.hasOwn(base.scenes, identity.themeId)) return []
  return service.presentation.labels.flatMap(label => {
    const intent = label.navigation_intent
    return intent !== null && Object.hasOwn(mapServiceRoutes, intent)
      ? [{ intent: intent as MapServiceIntent, path: mapServiceRoutes[intent as MapServiceIntent], label: label.label, rect: label.rect }] : []
  })
}
