import source from './style-source.json'
import { map4Labels, presentationHash } from './annotations'
import { map4PaintSource } from './paint-source'
import { abortable } from '../../map/resources'
import { requireMap, sha256 } from '../../map/json'

type Polygon = readonly (readonly number[])[]
const icons = source.icons as Record<string, readonly Polygon[]>, cutouts = source.icon_cutouts as Record<string, readonly Polygon[]>
const family = 'MCR Map4 Service Labels'

/** Map4 owns an independent font lifetime; no v5 scene/admission/coordinate dependency. */
export async function prepareMap4Labels(signal: AbortSignal) {
  const controller = new AbortController(), combined = AbortSignal.any([signal, controller.signal])
  let face: FontFace | null = null, disposed = false
  const dispose = () => { if (disposed) return; disposed = true; controller.abort(); if (face) document.fonts.delete(face); face = null }
  try {
    requireMap(await sha256(new TextEncoder().encode(JSON.stringify({ labels: map4Labels, style: source, ground: map4PaintSource }))) === presentationHash)
    const operation = async () => {
      const url = new URL('/world/reference-scene-v2/map-labels-v1/map-labels-bold.ttf', location.origin).href
      const response = await fetch(url, { signal: combined, credentials: 'omit', redirect: 'error', mode: 'same-origin', cache: 'force-cache', referrerPolicy: 'no-referrer' })
      requireMap(response.ok && !response.redirected && response.url === url && response.body)
      const reader = response.body.getReader(), chunks: Uint8Array[] = []; let size = 0
      try {
        for (;;) {
          const value = await reader.read(); if (value.done) break
          size += value.value.byteLength; requireMap(size <= source.font.bytes); chunks.push(value.value)
        }
      } finally { await reader.cancel().catch(() => {}); reader.releaseLock() }
      const bytes = new Uint8Array(size); let offset = 0
      for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.byteLength }
      requireMap(size === source.font.bytes && await sha256(bytes) === source.font.sha256)
      requireMap(!combined.aborted && !disposed)
      const candidate = await new FontFace(family, bytes).load()
      requireMap(!combined.aborted && !disposed)
      face = candidate; document.fonts.add(candidate)
    }
    await abortable(operation(), combined, 5000)
    return { dispose, draw(context: CanvasRenderingContext2D) {
      requireMap(!disposed && !combined.aborted && face && document.fonts.has(face))
      const s = source.style
      function polygon(points: Polygon, color: string) {
        context.beginPath(); points.forEach(([x, y], i) => i ? context.lineTo(x!, y!) : context.moveTo(x!, y!))
        context.closePath(); context.fillStyle = color; context.fill()
      }
      context.save()
      try {
        for (const label of map4Labels) {
          const r = label.rect, b = s.border_width
          context.fillStyle = s.border_fill; context.fillRect(r.x, r.y, r.width, r.height)
          context.fillStyle = s.paper_fill; context.fillRect(r.x + b, r.y + b, r.width - b * 2, r.height - b * 2)
          if (label.icon) {
            const x = r.x + s.padding, y = r.y + (r.height - s.icon_size) / 2, scale = s.icon_size / 24
            context.fillStyle = s.icon_paper; context.fillRect(x - 3, y - 3, s.icon_size + 6, s.icon_size + 6)
            const transform = (shape: Polygon) => shape.map(([px, py]) => [x + px! * scale, y + py! * scale])
            for (const shape of icons[label.icon]!) polygon(transform(shape), s.icon_fill)
            for (const shape of cutouts[label.icon] ?? []) polygon(transform(shape), s.icon_paper)
          }
          context.font = `${s.font_size}px "${family}"`; context.textAlign = 'left'; context.textBaseline = 'alphabetic'; context.fillStyle = s.text_fill
          context.fillText(label.label, r.x + s.padding + (label.icon ? s.icon_cell : 0), r.y + (r.height - s.line_height) / 2 + 32)
        }
      } finally { context.restore() }
    } }
  } catch (error) { dispose(); throw error }
}
