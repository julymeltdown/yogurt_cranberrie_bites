import input from './map-cozy-source.json'
import pins from './map-cozy-pins.json'
import base from './source-inputs.json'
import architecture from './architecture-source.json'
import { canonicalJson, requireMap, sha256 } from '../../map/json'
import type { Scenery } from '../../illustrated/model'

function freeze<T>(value: T): T {
  if (value && typeof value === 'object') { Object.values(value).forEach(freeze); Object.freeze(value) }
  return value
}
const document = freeze(structuredClone(input))
/** Source-only scenery replacement; it confers no map admission or place authority. */
export function validateSpatialV5Cozy(value = document, resourcePins = pins) {
  requireMap(value.format === 'MCR_SPATIAL5_MAP_COZY_V1_INPUTS' && value.version === 1
    && value.production_status === 'NOT_ADMITTED' && value.geometry_hash === base.geometry_hash
    && value.base_source_revision === base.source_revision
    && canonicalJson(value) === canonicalJson(document) && canonicalJson(resourcePins) === canonicalJson(pins)
    && canonicalJson(value.eras) === canonicalJson(['PARADE'])
    && canonicalJson(value.frame_rects) === canonicalJson(base.scenes.PARADE.frames)
    && value.ground.id === architecture.scenes.PARADE.scenery.ground.id
    && value.atlas.id === architecture.scenes.PARADE.scenery.atlases[0]!.id
    && value.ground.bytes + value.atlas.bytes <= 2097152)
}
export async function verifySpatialV5Cozy(value = document, resourcePins = pins) {
  validateSpatialV5Cozy(value, resourcePins)
  requireMap(await sha256(new TextEncoder().encode(JSON.stringify(value))) === resourcePins.document_sha256)
}
export function spatialV5CozyScenery(themeId: keyof typeof architecture.scenes, original: Scenery,
  value = document, resourcePins = pins): Scenery {
  validateSpatialV5Cozy(value, resourcePins)
  requireMap(Object.hasOwn(architecture.scenes, themeId)
    && canonicalJson(original) === canonicalJson(architecture.scenes[themeId].scenery))
  if (themeId !== 'PARADE') return original
  return freeze({ ...original, ground: value.ground, atlases: [value.atlas] })
}
