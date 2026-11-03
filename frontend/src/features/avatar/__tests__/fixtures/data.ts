// SYNTHETIC C06 TEST METADATA ONLY. No product artwork or review receipt.
import type { AvatarManifest } from '../../../../entities/avatar/runtime'
import source from '../../../../../../contracts/fixtures/assets/avatar-manifest.json'
import { contentHash, sha256 } from '../../../../world/map/json'
export async function fixtureManifest(png: Uint8Array<ArrayBuffer>) {
  const manifest = structuredClone(source) as AvatarManifest, config = manifest.presets[0]!.config
  manifest.skin_tones.push({ skin_tone_id: 'tone_unsupported', color: '#DDAA88' })
  // Fixture metadata adds choices using the same synthetic atlas, never production inventory.
  manifest.presets.push({ preset_id: 'fixture_no_accessory', config: { ...config, item_ids: { ...config.item_ids, accessory: null } } },
    { preset_id: 'fixture_unsupported', config: { ...config, skin_tone_id: 'tone_unsupported' } })
  Object.assign(manifest.atlas.texture, { image_state: 'PROVIDED', path: 'world/fe13-test/avatar.png', sha256: await sha256(png) })
  manifest.content_hash = await contentHash(manifest)
  return manifest
}
