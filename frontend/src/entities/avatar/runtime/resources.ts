import { characterSourcePack, characterArtSources } from '../source-catalog'
import { abortable, decodeImage, pngDimensions, type AssetResources } from '../../../world/map/resources'
import { sha256 } from '../../../world/map/json'
import { validDateTime } from '../../../world/map/formats'
import { AvatarError, ensure, type AvatarConfig, type AvatarManifest } from './contract'
import { schemaValid } from './schema'

/** A whole-character config cannot enter the layered compositor through a display boundary. */
export function isLayeredAvatarConfig(value: unknown): value is AvatarConfig {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
    && 'contract_version' in value && value.contract_version === '1.0.0'
    && 'kind' in value && value.kind === 'avatar_config' && schemaValid(value, 'config')
}

/** Local review receipt, not a new C06 wire DTO. No default binding or approved art ships here. */
export interface ReviewedAvatarBinding {
  readonly sourcePackId: string
  readonly sourcePackRevision: number
  readonly manifestHash: string
  readonly texturePath: string
  readonly textureHash: string
  readonly sources: readonly { readonly sourceId: string; readonly sha256: string }[]
  readonly reviewedBy: string
  readonly reviewedAt: string
  /** Only this baked tone is supported; C06 v1 defines no tint mask. */
  readonly bakedSkinToneId: string
  /** Optional approval to use this manifest's complete same-pack fallback preset. */
  readonly fallbackPresetId?: string
}
export function validateBinding(manifest: AvatarManifest, binding: ReviewedAvatarBinding | undefined) {
  if (!binding) throw new AvatarError('unavailable')
  const texture = manifest.atlas.texture, provenance = manifest.provenance
  ensure(binding.sourcePackId === characterSourcePack.source_pack_id && binding.sourcePackRevision === characterSourcePack.revision
    && binding.manifestHash === manifest.content_hash && binding.texturePath === texture.path && binding.textureHash === texture.sha256
    && binding.reviewedBy.trim().length > 0 && validDateTime(binding.reviewedAt)
    && provenance.source_kind === 'ORIGINAL_ART' && provenance.source_id === characterSourcePack.source_pack_id
    && provenance.review_state === 'REVIEWED' && binding.sources.length > 0 && binding.sources.length <= characterArtSources.length
    && new Set(binding.sources.map(s => s.sourceId)).size === binding.sources.length)
  for (const source of binding.sources) ensure(characterArtSources.some(s => s.source_id === source.sourceId && s.sha256 === source.sha256))
  ensure(manifest.skin_tones.some(t => t.skin_tone_id === binding.bakedSkinToneId))
  if (binding.fallbackPresetId !== undefined) ensure(binding.fallbackPresetId === manifest.fallback_preset_id
    && manifest.presets.some(p => p.preset_id === binding.fallbackPresetId && p.config.skin_tone_id === binding.bakedSkinToneId))
  return Object.freeze({ bakedSkinToneId: binding.bakedSkinToneId, fallbackPresetId: binding.fallbackPresetId })
}
/** Low-level C06 decoder. Product entry is loadReviewedAvatar; fixtures call this with synthetic bytes. */
export async function loadAtlas(manifest: AvatarManifest, resources: AssetResources, signal: AbortSignal) {
  const t = manifest.atlas.texture
  if (t.image_state !== 'PROVIDED' || !t.path || !t.sha256) throw new AvatarError('unavailable')
  const bytes = await abortable(resources.read({ path: t.path, sha256: t.sha256, kind: 'png' }, signal), signal)
  pngDimensions(bytes, t.width_px, t.height_px); ensure(await sha256(bytes) === t.sha256)
  const decoded = await decodeImage(bytes, signal)
  try {
    ensure(decoded.image.naturalWidth === t.width_px && decoded.image.naturalHeight === t.height_px)
    if (signal.aborted) throw new AvatarError('cancelled')
    return decoded
  } catch (error) { decoded.dispose(); throw error }
}
export async function loadReviewedAvatar(manifest: AvatarManifest, binding: ReviewedAvatarBinding | undefined, resources: AssetResources, signal: AbortSignal) {
  const policy = validateBinding(manifest, binding)
  const atlas = await loadAtlas(manifest, resources, signal)
  return { ...atlas, ...policy }
}
