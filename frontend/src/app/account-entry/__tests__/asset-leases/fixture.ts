import original from '../../../../../../contracts/fixtures/assets/avatar-manifest.json'
import { contentHash, sha256 } from '../../../../world/map/json'
import type { ReviewedAvatarBinding } from '../../../../entities/avatar/runtime'
import { characterArtSources, characterSourcePack } from './catalog'

/** Synthetic bytes + synthetic catalog; never approved production character art. */
export function headerPng() {
  const bytes = new Uint8Array(33), view = new DataView(bytes.buffer)
  bytes.set([137, 80, 78, 71, 13, 10, 26, 10]); view.setUint32(8, 13)
  bytes.set(new TextEncoder().encode('IHDR'), 12); view.setUint32(16, 320); view.setUint32(20, 1280)
  return bytes
}
export async function fixture(png = headerPng()) {
  const document = structuredClone(original)
  Object.assign(document.provenance, { source_id: characterSourcePack.source_pack_id, source_kind: 'ORIGINAL_ART', review_state: 'REVIEWED',
    reviewed_by: 'SYNTHETIC_TEST_ONLY', reviewed_at: '2026-11-04T00:00:00Z' })
  Object.assign(document.atlas.texture, { image_state: 'PROVIDED', path: 'world/synthetic-leases/atlas.png', sha256: await sha256(png) })
  document.content_hash = await contentHash(document)
  const binding: ReviewedAvatarBinding = { sourcePackId: characterSourcePack.source_pack_id, sourcePackRevision: characterSourcePack.revision,
    manifestHash: document.content_hash, texturePath: document.atlas.texture.path!, textureHash: document.atlas.texture.sha256!,
    sources: characterArtSources.map(source => ({ sourceId: source.source_id, sha256: source.sha256 })),
    reviewedBy: 'SYNTHETIC_TEST_ONLY', reviewedAt: '2026-11-04T00:00:00Z', bakedSkinToneId: 'tone_fixture' }
  return { document, pin: document, binding, png }
}
