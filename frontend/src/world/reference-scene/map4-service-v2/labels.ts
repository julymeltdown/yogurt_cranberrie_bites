import { compositionStyle as style } from './style'
import { abortable } from '../../map/resources'
import { requireMap, sha256 } from '../../map/json'
import type { CompositionModel } from './model'

type Polygon = readonly (readonly number[])[]
const icons = style.icons as Record<string, readonly Polygon[]>, cutouts = style.icon_cutouts as Record<string, readonly Polygon[]>
const family = 'MCR Map4 Composition Labels'

type LabelMetrics = Pick<TextMetrics, 'width'> & Partial<Pick<TextMetrics,
  'actualBoundingBoxLeft' | 'actualBoundingBoxRight' | 'actualBoundingBoxAscent' | 'actualBoundingBoxDescent'>>
/** Paint occupies measured ink space; source hit/exclusion rectangles remain independent. */
export function compositionLabelPaintRect(label: CompositionModel['labels'][number], metrics: LabelMetrics) {
  const s = style.style
  requireMap(s.paint_mode === 'MEASURED_CENTERED_V1' && Number.isFinite(metrics.width) && metrics.width > 0)
  const metric = (value: number | undefined, fallback: number) => {
    const result = value ?? fallback; requireMap(Number.isFinite(result)); return result
  }
  const left = Math.max(0, metric(metrics.actualBoundingBoxLeft, 0))
  const right = Math.max(metrics.width, metric(metrics.actualBoundingBoxRight, metrics.width))
  const ascent = metric(metrics.actualBoundingBoxAscent, s.font_size * .8)
  const descent = metric(metrics.actualBoundingBoxDescent, s.font_size * .2)
  const width = Math.ceil(left + right + s.padding * 2 + (label.icon ? s.icon_cell : 0) + s.text_metric_allowance)
  const height = s.single_height
  requireMap(width <= label.rect.width && height <= label.rect.height
    && ascent + descent > 0 && ascent + descent <= height - s.padding * 2 && s.icon_size + 6 <= height - s.border_width * 2)
  const x = label.x - width / 2, y = label.y - height / 2
  return Object.freeze({ x, y, width, height, textX: x + s.padding + (label.icon ? s.icon_cell : 0) + left,
    textBaseline: y + (height + ascent - descent) / 2 })
}

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
          const r = compositionLabelPaintRect(label, context.measureText(label.label)), b = s.border_width
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
          context.fillText(label.label, r.textX, r.textBaseline)
        }
      } finally { context.restore() }
    } }
  } catch (error) { dispose(); throw error }
}
