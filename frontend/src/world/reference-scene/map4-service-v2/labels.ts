import { compositionStyle as style } from './style'
import { abortable } from '../../map/resources'
import { requireMap, sha256 } from '../../map/json'
import type { CompositionModel } from './model'

type Polygon = readonly (readonly number[])[]
const icons = style.icons as Record<string, readonly Polygon[]>, cutouts = style.icon_cutouts as Record<string, readonly Polygon[]>
const family = 'MCR Map4 Composition Labels'

/** Independent lease over the retained original font/icons; no old layout or World asset reader. */
export async function prepareCompositionLabels(model: CompositionModel, signal: AbortSignal, options: { origin: string; fetch?: typeof fetch }) {
  requireMap(new URL(options.origin).origin === options.origin)
  const controller = new AbortController(), combined = AbortSignal.any([signal, controller.signal])
  let face: FontFace | null = null, disposed = false
  const dispose = () => { if (disposed) return; disposed = true; controller.abort(); if (face) document.fonts.delete(face); face = null }
  try {
    const operation = async () => {
      const url = options.origin + '/world/reference-scene-v2/map-labels-v1/map-labels-bold.ttf'
      const response = await (options.fetch ?? fetch)(url, { signal: combined, credentials: 'omit', redirect: 'error',
        mode: 'same-origin', cache: 'force-cache', referrerPolicy: 'no-referrer' })
      requireMap(response.ok && !response.redirected && response.url === url && response.body)
      const reader = response.body.getReader(), chunks: Uint8Array[] = []; let size = 0
      try {
        for (;;) {
          const value = await reader.read(); if (value.done) break
          size += value.value.byteLength; requireMap(size <= style.font.bytes); chunks.push(value.value)
        }
      } finally { await reader.cancel().catch(() => {}); reader.releaseLock() }
      const bytes = new Uint8Array(size); let offset = 0
      for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.byteLength }
      requireMap(size === style.font.bytes && await sha256(bytes) === style.font.sha256 && !combined.aborted && !disposed)
      const candidate = await new FontFace(family, bytes).load()
      requireMap(!combined.aborted && !disposed)
      face = candidate; document.fonts.add(candidate)
    }
    await abortable(operation(), combined, 5000)
    return { dispose, draw(context: CanvasRenderingContext2D) {
      requireMap(!disposed && !combined.aborted && face && document.fonts.has(face))
      const s = style.style
      function polygon(points: Polygon, color: string) {
        context.beginPath(); points.forEach(([x, y], i) => i ? context.lineTo(x!, y!) : context.moveTo(x!, y!))
        context.closePath(); context.fillStyle = color; context.fill()
      }
      context.save()
      try {
        context.font = `${s.font_size}px "${family}"`; context.textAlign = 'left'; context.textBaseline = 'alphabetic'
        for (const label of model.labels) {
          const r = label.rect, b = s.border_width
          requireMap(context.measureText(label.label).width <= r.width - s.padding * 2 - (label.icon ? s.icon_cell : 0) + s.text_metric_allowance)
          context.fillStyle = s.border_fill; context.fillRect(r.x, r.y, r.width, r.height)
          context.fillStyle = s.paper_fill; context.fillRect(r.x + b, r.y + b, r.width - b * 2, r.height - b * 2)
          if (label.icon) {
            const x = r.x + s.padding, y = r.y + (r.height - s.icon_size) / 2, scale = s.icon_size / 24
            context.fillStyle = s.icon_paper; context.fillRect(x - 3, y - 3, s.icon_size + 6, s.icon_size + 6)
            const transform = (shape: Polygon) => shape.map(([px, py]) => [x + px! * scale, y + py! * scale])
            for (const shape of icons[label.icon]!) polygon(transform(shape), s.icon_fill)
            for (const shape of cutouts[label.icon] ?? []) polygon(transform(shape), s.icon_paper)
          }
          context.fillStyle = s.text_fill
          context.fillText(label.label, r.x + s.padding + (label.icon ? s.icon_cell : 0), r.y + (r.height - s.line_height) / 2 + 32)
        }
      } finally { context.restore() }
    } }
  } catch (error) { dispose(); throw error }
}
