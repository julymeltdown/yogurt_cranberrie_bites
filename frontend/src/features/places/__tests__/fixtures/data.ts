import fixture from '../../../../../../contracts/fixtures/assets/map-manifest.json'
import { contentHash, digest } from '../../../../world/map/json'
import type { MapManifest } from '../../../../world/map/manifest'
import type { PlaceMapRelease } from '../../model/controller'
import type { Place, PlaceList } from '../../model/contract'
export { deferred } from '../../../../test-support/fixtures'

export const eventId = '00000000-0000-4000-8000-000000000201'
export const userId = '00000000-0000-4000-8000-000000000202'
export const otherUser = '00000000-0000-4000-8000-000000000203'
const id = (n: number) => `00000000-0000-4000-8000-${String(n).padStart(12, '0')}`
export function places(): Place[] {
  const concept: Place = { ...structuredClone(fixture.places[0]!), place_id: id(1), label: 'Synthetic virtual stage',
    place_kind: 'VIRTUAL_CONCEPT', verification_state: 'CONCEPT_ONLY', closure_state: 'OPEN',
    provenance: { ...fixture.places[0]!.provenance, source_kind: 'ORIGINAL_FIXTURE', review_state: 'UNREVIEWED', photo_permission: 'NOT_APPLICABLE' } }
  const physical: Place = { ...structuredClone(concept), place_id: id(2), label: 'Synthetic reviewed landmark',
    place_kind: 'PHYSICAL', verification_state: 'VERIFIED', accessibility_notes: 'Synthetic notes only. No real venue access claim.',
    provenance: { ...concept.provenance, source_id: 'synthetic_reference', source_kind: 'PLACE_REFERENCE', source_revision: 'fixture-reference-1',
      source_uri: 'https://venue.example.invalid/reference', review_state: 'REVIEWED', reviewed_by: 'Synthetic reviewer',
      reviewed_at: '2026-11-01T00:00:00Z', photo_permission: 'DENIED' } }
  return [concept, physical,
    { ...structuredClone(physical), place_id: id(3), label: 'Synthetic unverified gate', verification_state: 'UNVERIFIED',
      provenance: { ...physical.provenance, review_state: 'UNREVIEWED', reviewed_by: null, reviewed_at: null } },
    { ...structuredClone(physical), place_id: id(4), label: 'Synthetic closed landmark', closure_state: 'CLOSED' },
    { ...structuredClone(physical), place_id: id(5), label: 'Synthetic unavailable source', provenance: { ...physical.provenance, source_uri: 'javascript:alert(1)' } },
  ]
}
export function catalog(items = places(), revision = 7): PlaceList { return { event_id: eventId, revision, items } }
export async function release(items = places(), mapRevision = 1): Promise<PlaceMapRelease> {
  const map: MapManifest = structuredClone(fixture), cells = [[1, 1], [2, 1], [3, 1], [3, 2], [3, 3]] as const
  map.map_revision = mapRevision
  map.places = structuredClone(items)
  map.geometry.interactions = items.map((place, i) => ({ interaction_id: `inspect_${i}`, place_id: place.place_id,
    cell_x: cells[i % cells.length]![0], cell_y: cells[i % cells.length]![1] }))
  map.geometry_hash = await digest(map.geometry)
  map.content_hash = await contentHash(map)
  return { scope: { generation: 1, userId, eventId }, manifest: map, expected: { mapId: map.map_id, revision: map.map_revision, contentHash: map.content_hash, geometryHash: map.geometry_hash } }
}
