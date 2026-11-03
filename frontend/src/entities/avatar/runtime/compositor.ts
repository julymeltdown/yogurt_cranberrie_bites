import { canonicalJson } from '../../../world/map/json'
import { AvatarError, ensure, frameIndex, validateConfig, type AvatarConfig, type AvatarManifest, type Layer, type Pose } from './contract'
import { schemaValid } from './schema'

export interface ComposedAvatar { readonly key: string; readonly canvas: HTMLCanvasElement; readonly fallback: boolean; release(): void }
/** Per owner/cache: never share private outfit keys across users/events or persist them. */
export function createAvatarCompositor(options: { manifest: AvatarManifest; image: CanvasImageSource; bakedSkinToneId: string;
  fallbackPresetId?: string | undefined; maxEntries?: number; disposeAtlas(): void }) {
  const { manifest } = options, maximum = options.maxEntries ?? 50
  ensure(Number.isInteger(maximum) && maximum > 0 && maximum <= 50)
  ensure(manifest.skin_tones.some(t => t.skin_tone_id === options.bakedSkinToneId))
  if (options.fallbackPresetId !== undefined) ensure(options.fallbackPresetId === manifest.fallback_preset_id)
  type Entry = { canvas: HTMLCanvasElement; refs: number }
  const cache = new Map<string, Entry>()
  const thumbnails = new Set<() => void>()
  let disposed = false, created = 0, removed = 0
  function choose(value: unknown) {
    let config: AvatarConfig, fallback = false
    try { config = validateConfig(value, manifest) }
    catch (error) {
      if (error instanceof AvatarError && error.reason === 'version') throw error
      if (options.fallbackPresetId === undefined || !schemaValid(value, 'config')) throw error
      const requested = value as AvatarConfig
      if (requested.skin_tone_id !== options.bakedSkinToneId) throw error
      config = validateConfig(manifest.presets.find(p => p.preset_id === options.fallbackPresetId)!.config, manifest)
      if (config.skin_tone_id !== options.bakedSkinToneId) throw new AvatarError('unavailable')
      fallback = true
    }
    if (config.skin_tone_id !== options.bakedSkinToneId) throw new AvatarError('unavailable')
    return { config, fallback }
  }
  function erase(key: string, entry: Entry) { entry.canvas.width = entry.canvas.height = 0; cache.delete(key); removed++ }
  function acquire(value: unknown): ComposedAvatar {
    if (disposed) throw new AvatarError('unavailable')
    const { config, fallback } = choose(value), key = `${manifest.content_hash}/${canonicalJson(config)}`
    let entry = cache.get(key)
    if (!entry) {
      if (cache.size >= maximum) {
        const victim = [...cache].find(([, e]) => e.refs === 0)
        if (!victim) throw new AvatarError('capacity')
        erase(...victim)
      }
      const canvas = document.createElement('canvas'); canvas.width = 320; canvas.height = 256
      const context = canvas.getContext('2d'); if (!context) throw new AvatarError('unavailable')
      context.imageSmoothingEnabled = false
      for (const layer of manifest.layer_order as Layer[]) {
        const id = config.item_ids[layer]; if (id === null) continue
        const item = manifest.atlas.items.find(item => item.item_id === id)!
        context.drawImage(options.image, item.region.x, item.region.y, 320, 256, 0, 0, 320, 256)
      }
      entry = { canvas, refs: 0 }; created++
    }
    cache.delete(key); cache.set(key, entry); entry.refs++
    let released = false
    return { key, canvas: entry.canvas, fallback, release() { if (released) return; released = true; entry.refs-- } }
  }
  function thumbnail(value: unknown, pose: Pose, signal?: AbortSignal) {
    if (signal?.aborted) throw new AvatarError('cancelled')
    const frame = frameIndex(pose), composed = acquire(value), canvas = document.createElement('canvas')
    canvas.width = canvas.height = 64
    let closed = false
    const dispose = () => { if (closed) return; closed = true; canvas.width = canvas.height = 0; thumbnails.delete(dispose); signal?.removeEventListener('abort', dispose) }
    try {
      const context = canvas.getContext('2d'); if (!context) throw new AvatarError('unavailable')
      context.imageSmoothingEnabled = false
      context.drawImage(composed.canvas, frame % 5 * 64, Math.floor(frame / 5) * 64, 64, 64, 0, 0, 64, 64)
      signal?.addEventListener('abort', dispose, { once: true })
      thumbnails.add(dispose)
      return { canvas, fallback: composed.fallback, dispose }
    } catch (error) { dispose(); throw error }
    finally { composed.release() }
  }
  return { manifest, acquire, thumbnail,
    read: () => ({ entries: cache.size, references: [...cache.values()].reduce((sum, e) => sum + e.refs, 0), created, removed, disposed }),
    dispose() { if (disposed) return; disposed = true; for (const remove of Array.from(thumbnails)) remove(); for (const [key, entry] of cache) erase(key, entry); options.disposeAtlas() },
  }
}
export type AvatarCompositor = ReturnType<typeof createAvatarCompositor>
