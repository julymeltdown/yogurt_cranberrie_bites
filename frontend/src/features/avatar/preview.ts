import { AvatarError, createReviewedAvatarCompositor, validateConfig, type AvatarCompositor } from '../../entities/avatar/runtime'
import { immutable, type AvatarCatalog, type AvatarConfig, type AvatarPreviewPort, type Layer } from './contract'

/** Internal adapter, also exercised with the explicitly synthetic compositor in tests.
 * Production callers must use createReviewedAvatarPreview, which enforces reviewed art.
 */
export function compositorPreview(compositor: AvatarCompositor, bakedSkinToneId: string, signal: AbortSignal): AvatarPreviewPort {
  let closed = false
  const manifest = compositor.manifest
  const catalog: AvatarCatalog = immutable({ presets: manifest.presets.map(p => ({ id: p.preset_id, config: p.config })),
    items: manifest.atlas.items.map(item => ({ id: item.item_id, layer: item.layer_id as Layer })), tones: manifest.skin_tones.map(t => t.skin_tone_id) })
  const supports = (config: AvatarConfig) => {
    if (closed || signal.aborted) return false
    try { return validateConfig(config, manifest).skin_tone_id === bakedSkinToneId } catch { return false }
  }
  const dispose = () => { if (closed) return; closed = true; signal.removeEventListener('abort', dispose); compositor.dispose() }
  signal.addEventListener('abort', dispose, { once: true })
  if (signal.aborted) dispose()
  return { catalog, supports,
    async render(config, signal) {
      if (!supports(config)) throw new AvatarError('unavailable')
      const lease = compositor.thumbnail(config, { direction: 'down', animation: 'idle', frame: 0 }, signal)
      if (lease.fallback) { lease.dispose(); throw new AvatarError('unavailable') }
      return { canvas: lease.canvas, dispose() { lease.canvas.remove(); lease.dispose() } }
    }, dispose }
}
/** Same committed C06 compositor entry as world/profile; no binding or artwork defaults. */
export async function createReviewedAvatarPreview(options: Parameters<typeof createReviewedAvatarCompositor>[0]): Promise<AvatarPreviewPort> {
  const compositor = await createReviewedAvatarCompositor(options)
  // The reviewed entry already rejects absent binding. Keep this guard explicit for future API changes.
  if (!options.binding) { compositor.dispose(); throw new AvatarError('unavailable') }
  return compositorPreview(compositor, options.binding.bakedSkinToneId, options.signal)
}
