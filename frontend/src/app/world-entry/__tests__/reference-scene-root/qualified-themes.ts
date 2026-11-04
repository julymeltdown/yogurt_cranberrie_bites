// TEST ONLY derived manifest packaging. Public source-review DATA is retained verbatim, not backend implementation.
import qualificationText from './source-inputs/scene-source-qualification.json?raw'
import bindingText from './source-inputs/source-binding.json?raw'
import type qualificationType from './source-inputs/scene-source-qualification.json'
import type { MapManifest, ThemeManifest, ReleasePin } from '../../../../world/map/manifest'
import { canonicalJson, contentHash, sha256, requireMap } from '../../../../world/map/json'
import { validateManifests } from '../../../../world/map/manifest'
import { preflightReferenceScene } from '../../reference-scene-overview'

export const qualificationCommit = '0a8c422645458c9397af9289e15089ee3f9cb0ad'
export const qualificationHash = 'sha256:70c3d1ac268c6e5da39d30a20e01406fc7eb27668d6f0e6a66e1d87de15d7d07'
/** Exact source provenance qualifies only supplied pixels/scenery. This producer separately checks every derived DTO binding. */
export async function deriveFixtureThemes(map: MapManifest, themes: readonly ThemeManifest[], avatar: ReleasePin['avatar']) {
  requireMap(await sha256(new TextEncoder().encode(qualificationText)) === qualificationHash)
  const q = JSON.parse(qualificationText) as typeof qualificationType
  requireMap(q.format === 'MCR_SPATIAL_SCENE_SOURCE_QUALIFICATION' && q.version === 1 && q.state === 'CONTRACT_READY'
    && q.admission === 'NOT_ADMITTED' && await sha256(new TextEncoder().encode(bindingText)) === q.source_binding_sha256
    && map.map_id === q.map.map_id && map.map_revision === q.map.map_revision && map.geometry_hash === q.map.geometry_hash
    && themes.length === 3 && new Set(themes.map(t => t.theme_id)).size === 3)
  return Promise.all(themes.map(async original => {
    const scene = q.scenes.find(s => s.theme_id === original.theme_id), scenery = original.scenery
    requireMap(scene && scenery && original.provenance.review_state === 'UNREVIEWED'
      && scene.source_provenance.review_state === 'REVIEWED')
    // The consumer's strict metadata validation checks the entire exact source body, not hash labels alone.
    const descriptors = [scenery.ground, ...scenery.atlases]
    requireMap(descriptors.length === scene.runtime_resources.length && descriptors.every(d => scene.runtime_resources.some(r =>
      r.path === d.path && r.sha256 === d.sha256 && r.width_px === d.width_px && r.height_px === d.height_px)))
    const derived: ThemeManifest = { ...structuredClone(original), provenance: structuredClone(scene.source_provenance) as ThemeManifest['provenance'] }
    derived.content_hash = await contentHash(derived)
    const metadata = await validateManifests(canonicalJson(map), canonicalJson(derived), { mapHash: map.content_hash, themeHash: derived.content_hash, avatar })
    preflightReferenceScene(metadata)
    requireMap(derived.map_content_hash === map.content_hash && derived.avatar_content_hash === avatar.content_hash)
    return derived
  }))
}
