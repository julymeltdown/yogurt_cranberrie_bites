import { assertMotionArt, motionFrame, requireMotion, validateMotionSelection, type MotionArt, type MotionDirection, type MotionItem, type MotionSelection } from './art'

export interface MotionComposition { readonly key: string; readonly canvas: HTMLCanvasElement; release(): void }
/** One scoped owner; 20 output cells fit the incumbent Phaser texture() capability. No animation loop. */
export function createMotionCompositor(options: { art: MotionArt; image: CanvasImageSource; isCurrent(): boolean; maxEntries?: number }) {
  assertMotionArt(options.art)
  const { descriptor } = options.art, maximum = options.maxEntries ?? 50
  requireMotion(Number.isInteger(maximum) && maximum > 0 && maximum <= 50)
  type Entry = { canvas: HTMLCanvasElement; refs: number }
  const cache = new Map<string, Entry>(), invalidations = new Set<() => void>()
  let disposed = false, created = 0, removed = 0
  const current = () => !disposed && options.isCurrent()
  function erase(key: string, entry: Entry) { cache.delete(key); entry.canvas.width = entry.canvas.height = 0; removed++ }
  function item(key: string, kind: MotionItem['kind']) { const result = descriptor.wardrobe.items.find(i => i.key === key && i.kind === kind); requireMotion(result); return result }
  function drawCell(context: CanvasRenderingContext2D, index: number, x = 0, y = 0) {
    requireMotion(Number.isInteger(index) && index >= 0 && index < descriptor.physicalFrames)
    context.drawImage(options.image, index % 20 * 64, Math.floor(index / 20) * 64, 64, 64, x, y, 64, 64)
  }
  function compose(selection: MotionSelection) {
    const canvas = document.createElement('canvas'); canvas.width = 320; canvas.height = 256
    const context = canvas.getContext('2d'); requireMotion(context); context.imageSmoothingEnabled = false
    const cuff = document.createElement('canvas'); cuff.width = cuff.height = 64
    try {
      const mask = cuff.getContext('2d'); requireMotion(mask); mask.imageSmoothingEnabled = false
      for (let direction = 0; direction < 4; direction++) for (let phase = 0; phase < 5; phase++) {
        const x = phase * 64, y = direction * 64
        if (selection.mode === 'preset') {
          const source = descriptor.presets.frames.find(f => f.preset === selection.preset && f.phase === phase
            && f.direction === ['front', 'left', 'right', 'rear'][direction]); requireMotion(source)
          drawCell(context, source.atlasIndex, x, y); continue
        }
        const body = item(selection.body, 'body'), outfit = selection.outfit === null ? null : item(selection.outfit, 'outfit')
        const frame = (part: MotionItem) => { const index = part.directions[direction]?.frames[phase]; requireMotion(index !== undefined); return index }
        if (outfit) {
          context.save(); context.beginPath(); context.rect(x, y, 64, descriptor.wardrobe.composition.outfitBodyClipY); context.clip()
          drawCell(context, frame(body), x, y); context.restore()
          const hands = outfit.directions[direction]?.handFrames?.[phase], color = body.skinPalette?.colors[2]
          requireMotion(hands !== undefined && color?.length === 3)
          mask.clearRect(0, 0, 64, 64); drawCell(mask, hands)
          mask.globalCompositeOperation = 'source-in'; mask.fillStyle = `rgb(${color.join(',')})`; mask.fillRect(0, 0, 64, 64)
          mask.globalCompositeOperation = 'source-over'; context.drawImage(cuff, x, y)
          drawCell(context, frame(outfit), x, y)
        } else drawCell(context, frame(body), x, y)
        for (const kind of ['eyes', 'hair', 'accessory'] as const) {
          const key = selection[kind]
          // Rear eyes use explicitly transparent UVs in the pinned descriptor, not a frontal overlay.
          if (key !== null) drawCell(context, frame(item(key, kind)), x, y)
        }
      }
      return canvas
    } catch (error) { canvas.width = canvas.height = 0; throw error }
    finally { cuff.width = cuff.height = 0 }
  }
  function acquire(input: unknown): MotionComposition {
    requireMotion(current())
    const selection = validateMotionSelection(input, options.art), key = JSON.stringify(selection)
    let entry = cache.get(key)
    if (!entry) {
      if (cache.size >= maximum) { const victim = [...cache].find(([, e]) => e.refs === 0); requireMotion(victim); erase(...victim) }
      entry = { canvas: compose(selection), refs: 0 }; cache.set(key, entry); created++
    }
    cache.delete(key); cache.set(key, entry); entry.refs++
    let released = false
    return { key, canvas: entry.canvas, release() { if (released) return; released = true; entry.refs-- } }
  }
  function thumbnail(input: unknown, direction: MotionDirection = 'down', phase = 0) {
    requireMotion(Number.isInteger(phase) && phase >= 0 && phase <= 4)
    const acquired = acquire(input), canvas = document.createElement('canvas'); canvas.width = canvas.height = 64
    try {
      const context = canvas.getContext('2d'); requireMotion(context); context.imageSmoothingEnabled = false
      const index = motionFrame(direction, phase !== 0, Math.max(0, phase - 1) * 140)
      context.drawImage(acquired.canvas, index % 5 * 64, Math.floor(index / 5) * 64, 64, 64, 0, 0, 64, 64)
      let closed = false
      const dispose = () => { if (closed) return; closed = true; invalidations.delete(dispose); canvas.width = canvas.height = 0 }
      invalidations.add(dispose); return { canvas, dispose }
    } catch (error) { canvas.width = canvas.height = 0; throw error }
    finally { acquired.release() }
  }
  return { acquire, thumbnail, validateSelection: (input: unknown) => validateMotionSelection(input, options.art),
    read: () => ({ entries: cache.size, references: [...cache.values()].reduce((n, e) => n + e.refs, 0), created, removed, thumbnails: invalidations.size, disposed }),
    dispose() { if (disposed) return; disposed = true; for (const clear of Array.from(invalidations)) clear(); for (const [key, entry] of Array.from(cache)) erase(key, entry) },
  }
}
export type MotionCompositor = ReturnType<typeof createMotionCompositor>
