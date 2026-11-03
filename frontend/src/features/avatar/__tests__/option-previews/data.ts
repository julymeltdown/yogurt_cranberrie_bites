// SYNTHETIC METADATA ONLY. Preset combinations test choice/lifetime behavior, not approved art.
import source from '../../../../../../contracts/fixtures/assets/avatar-manifest.json'
import type { AvatarManifest } from '../../../../entities/avatar/runtime'
import { contentHash } from '../../../../world/map/json'
export async function optionManifest(pngSha256: string) {
  const manifest = structuredClone(source) as AvatarManifest, original = manifest.presets[0]!.config
  manifest.skin_tones.push({ skin_tone_id: 'tone_unsupported', color: '#DDAA88' })
  manifest.presets = Array.from({ length: 7 }, (_, index) => ({ preset_id: `look_${index + 1}`, config: {
    ...original, item_ids: { ...original.item_ids, accessory: index % 2 ? null : original.item_ids.accessory },
  } }))
  manifest.presets.push({ preset_id: 'look_unsupported', config: { ...original, skin_tone_id: 'tone_unsupported' } })
  manifest.fallback_preset_id = 'look_1'
  Object.assign(manifest.atlas.texture, { image_state: 'PROVIDED', path: 'world/fe13-options-test/avatar.png', sha256: pngSha256 })
  manifest.content_hash = await contentHash(manifest)
  return manifest
}
