// TEST ONLY coherent geometry/catalog. No release admission or review is granted.
import proposal from '../../../../world/reference-scene/source-inputs.json'
import template from '../../../../../../contracts/fixtures/assets/map-manifest.json'
import { contentHash, digest } from '../../../../world/map/json'
import type { MapManifest } from '../../../../world/map/manifest'
import type { PlaceMapRelease } from '../../model/controller'
import type { SessionScope } from '../../../../shared/api/http'
import { referenceSceneIdentity } from '../../../../world/reference-scene'

export async function referenceMapRelease(scope: SessionScope): Promise<PlaceMapRelease> {
  const manifest = structuredClone(template) as MapManifest
  manifest.map_id = referenceSceneIdentity.mapId
  manifest.map_revision = referenceSceneIdentity.mapRevision
  manifest.geometry = structuredClone(proposal.geometry)
  manifest.geometry_hash = await digest(manifest.geometry)
  if (manifest.geometry_hash !== proposal.geometry_hash) throw new Error('Reference geometry changed')
  manifest.places = structuredClone(proposal.places)
  manifest.tilemap = { format: 'TILED_JSON', resource_state: 'NOT_PROVIDED', path: null, sha256: null }
  // This can be inspected by Places, but production admission must reject it.
  manifest.provenance = { ...manifest.provenance, source_kind: 'ORIGINAL_ART', source_revision: '8f89d1905a86143d34ff77d2951fc60651ab2497',
    review_state: 'UNREVIEWED', reviewed_by: null, reviewed_at: null }
  manifest.content_hash = await contentHash(manifest)
  return { scope: { generation: scope.generation, userId: scope.userId, eventId: scope.eventId }, manifest,
    expected: { mapId: manifest.map_id, revision: manifest.map_revision, contentHash: manifest.content_hash, geometryHash: manifest.geometry_hash } }
}
