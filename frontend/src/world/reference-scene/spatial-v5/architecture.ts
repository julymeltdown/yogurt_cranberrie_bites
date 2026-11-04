import architecture from './architecture-source.json'
import pins from './architecture-pin.json'
import { canonicalJson, requireMap } from '../../map/json'
import type { Scenery } from '../../illustrated/model'

/** Compiled, separately pinned art replacement. Geometry and the original source tuple stay immutable. */
export function spatialV5ArchitectureScenery(themeId: keyof typeof architecture.scenes, base: Scenery,
  document = architecture, resourcePins = pins): Scenery {
  requireMap(document.format === 'MCR_SPATIAL5_ARCHITECTURE_V2_INPUTS' && document.version === 1
    && document.production_status === 'NOT_ADMITTED' && document.geometry_hash === architecture.geometry_hash
    && document.base_source_revision === architecture.base_source_revision
    && resourcePins.format === pins.format && resourcePins.version === 1 && resourcePins.production_status === 'NOT_ADMITTED'
    && canonicalJson(resourcePins.handoff) === canonicalJson(pins.handoff)
    && canonicalJson(resourcePins.runtime_atlases) === canonicalJson(pins.runtime_atlases))
  const scene = document.scenes[themeId]?.scenery as Scenery | undefined
  requireMap(scene && scene.atlases.length === 1 && base.atlases.length === 1
    && canonicalJson(scene.atlases[0]) === canonicalJson(pins.runtime_atlases[themeId])
    && canonicalJson(scene.ground) === canonicalJson(base.ground))
  const atlasId = scene.atlases[0]!.id
  requireMap(scene.objects.length === 34 && scene.objects.every(o => o.atlas_id === atlasId))
  const normalized = { ...scene, atlases: base.atlases, objects: scene.objects.map(o => ({ ...o, atlas_id: base.atlases[0]!.id })) }
  requireMap(canonicalJson(normalized) === canonicalJson(base))
  return scene
}
