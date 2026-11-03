import type { MuralBinding } from '../../features/mural/sync/contract'
export interface MuralFrame {
  readonly binding: MuralBinding
  readonly revision: number
  readonly state: 'EDITABLE' | 'PAUSED' | 'FROZEN'
  readonly palette: readonly string[]
  readonly pixels: readonly number[]
}
export interface MuralSink { replace(frame: MuralFrame | null): void; dispose(): void; subscribeFailure?(listener: () => void): () => void }
export interface MuralSceneTexture { replace(rgba: Uint8ClampedArray): void; dispose(): void }
export interface MuralSceneObject { dispose(): void }
/** The incumbent scene supplies these leases and its existing frame loop. */
export interface MuralScenePort {
  texture(): MuralSceneTexture
  object(texture: MuralSceneTexture): MuralSceneObject
  onFrame(listener: () => void): () => void
  onClose?(listener: () => void): () => void
}
export function createMuralTextureSink(scene: MuralScenePort): MuralSink & { read(): Readonly<{ revision: number | null; queued: number; textures: number; objects: number; listeners: number }> } {
  let disposed = false, pending: MuralFrame | null = null, texture: MuralSceneTexture | null = null, object: MuralSceneObject | null = null
  const failures = new Set<() => void>()
  let removeFrame: (() => void) | null = null, removeClose: (() => void) | null = null, revision: number | null = null, binding = ''
  const clear = () => {
    const remove = removeFrame, close = removeClose, image = object, raster = texture
    removeFrame = removeClose = null; object = null; texture = null; pending = null; revision = null; binding = ''
    try { remove?.() } finally { try { close?.() } finally { try { image?.dispose() } finally { raster?.dispose() } } }
  }
  const failed = () => {
    try { clear() } catch { /* A failed disposal cannot strand later failure observers. */ }
    for (const listener of Array.from(failures)) { try { listener() } catch { /* Independent failure observers. */ } }
  }
  const flush = () => {
    const frame = pending; pending = null
    if (disposed || !frame) return
    try {
      texture ??= scene.texture(); object ??= scene.object(texture)
      const colors = frame.palette.map(hex => [Number.parseInt(hex.slice(1, 3), 16), Number.parseInt(hex.slice(3, 5), 16), Number.parseInt(hex.slice(5, 7), 16)])
      const rgba = new Uint8ClampedArray(128 * 128 * 4)
      for (let i = 0; i < 16384; i++) { const color = colors[frame.pixels[i]!]!; rgba.set([color[0]!, color[1]!, color[2]!, 255], i * 4) }
      texture.replace(rgba); revision = frame.revision
    } catch { failed() }
  }
  return {
    replace(frame) {
      if (disposed) return
      if (!frame) { clear(); return }
      if (!Number.isSafeInteger(frame.revision) || frame.revision < 1 || frame.palette.length !== 16 || frame.pixels.length !== 16384
        || frame.palette.some(c => !/^#[0-9A-F]{6}$/.test(c)) || frame.pixels.some(c => !Number.isInteger(c) || c < 0 || c > 15)) { clear(); throw new Error('Invalid mural frame') }
      const nextBinding = JSON.stringify(frame.binding)
      if (binding && binding !== nextBinding) clear()
      if (frame.revision < Math.max(revision ?? 0, pending?.revision ?? 0)) return
      binding = nextBinding
      pending = Object.freeze({ ...frame, binding: Object.freeze({ ...frame.binding }), palette: Object.freeze([...frame.palette]), pixels: Object.freeze([...frame.pixels]) })
      removeFrame ??= scene.onFrame(flush)
      removeClose ??= scene.onClose?.(failed) ?? null
    },
    read: () => Object.freeze({ revision, queued: pending ? 1 : 0, textures: texture ? 1 : 0, objects: object ? 1 : 0, listeners: (removeFrame ? 1 : 0) + (removeClose ? 1 : 0) }),
    subscribeFailure(listener) { failures.add(listener); return () => { failures.delete(listener) } },
    dispose() { if (!disposed) { disposed = true; try { clear() } finally { failures.clear() } } },
  }
}
