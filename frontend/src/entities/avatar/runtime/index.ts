import { canonicalJson } from '../../../world/map/json'
import type { AssetResources } from '../../../world/map/resources'
import { createAvatarCompositor } from './compositor'
import { AvatarError, validateManifest } from './contract'
import { loadReviewedAvatar, type ReviewedAvatarBinding } from './resources'
import type { ReviewedAtlasLeases } from './atlas-lease'

export type { AvatarConfig, AvatarManifest, Pose, Direction, Layer } from './contract'
export type { AvatarCompositor, ComposedAvatar } from './compositor'
export type { ReviewedAvatarBinding } from './resources'
export { createReviewedAtlasLeases, type ReviewedAtlasLeases } from './atlas-lease'
export { validateConfig, frameIndex, sourceRect, AvatarError } from './contract'

/** Shared product entry for world/editor/profile. Pin to the current authorized bootstrap;
 * caller owns one user/event/lease-scoped instance. No default art or implicit fallback.
 */
export async function createReviewedAvatarCompositor(options: { document: unknown;
  pin: { manifest_id: string; manifest_version: number; content_hash: string }; binding?: ReviewedAvatarBinding;
  resources: AssetResources; signal: AbortSignal; leases?: ReviewedAtlasLeases }) {
  const { signal } = options
  if (signal.aborted) throw new AvatarError('cancelled')
  const borrowed = options.leases ? await options.leases.acquire(options, signal) : undefined
  const manifest = borrowed?.manifest ?? await validateManifest(canonicalJson(options.document), options.pin)
  const atlas = borrowed ?? await loadReviewedAvatar(manifest, options.binding, options.resources, signal)
  if (signal.aborted) { atlas.dispose(); throw new AvatarError('cancelled') }
  const ownerSignal = options.leases?.signal
  if (ownerSignal?.aborted) { atlas.dispose(); throw new AvatarError('cancelled') }
  let compositor: ReturnType<typeof createAvatarCompositor>
  const dispose = () => compositor.dispose()
  try {
    compositor = createAvatarCompositor({ manifest, image: atlas.image, bakedSkinToneId: atlas.bakedSkinToneId,
      fallbackPresetId: atlas.fallbackPresetId, disposeAtlas() {
        signal.removeEventListener('abort', dispose); ownerSignal?.removeEventListener('abort', dispose); atlas.dispose()
      } })
  } catch (error) { atlas.dispose(); throw error }
  signal.addEventListener('abort', dispose, { once: true })
  ownerSignal?.addEventListener('abort', dispose, { once: true })
  return compositor
}
