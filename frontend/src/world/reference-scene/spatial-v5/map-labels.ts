import { getSpatialV5ServicePresentation } from './map-service'
import { sha256 } from '../../map/json'
import { validateSpatialV5MapLayoutPair, verifySpatialV5MapLayoutPair } from './model'
const presentation = getSpatialV5ServicePresentation()

type Polygon = readonly (readonly number[])[]
const icons = presentation.icons as Record<string, readonly Polygon[]>
const cutouts = presentation.icon_cutouts as Record<string, readonly Polygon[]>
const family = presentation.style.font_family
const fontPath = '/world/reference-scene-v2/map-labels-v1/map-labels-bold.ttf'

/** One public font lease per overview owner; never part of C06's two scene PNGs or private data. */
export function createSpatialV5Labels() {
  validateSpatialV5MapLayoutPair()
  const lifetime = new AbortController()
  let disposed = false, face: FontFace | null = null, pending: Promise<void> | undefined
  return {
    prepare(): Promise<void> {
      if (disposed) return Promise.reject(Error('Map labels disposed'))
      if (pending) return pending
      pending = (async () => {
        await verifySpatialV5MapLayoutPair()
        if (disposed || lifetime.signal.aborted) throw Error('Map labels disposed')
        const timeout = setTimeout(() => lifetime.abort(), 5000)
        try {
          const response = await fetch(new URL(fontPath, location.origin), { signal: lifetime.signal, credentials: 'omit',
            redirect: 'error', cache: 'no-store', referrerPolicy: 'no-referrer' })
          if (!response.ok || response.redirected || !response.body) throw Error('Map label font unavailable')
          const reader = response.body.getReader(), chunks: Uint8Array[] = []
          let size = 0
          try {
            for (;;) {
              const { done, value } = await reader.read(); if (done) break
              size += value.length; if (size > presentation.font.bytes) throw Error('Map label font exceeds pin')
              chunks.push(value)
            }
          } finally { void reader.cancel().catch(() => undefined); reader.releaseLock() }
          const bytes = new Uint8Array(size); let offset = 0
          for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.length }
          if (size !== presentation.font.bytes || await sha256(bytes) !== presentation.font.sha256) throw Error('Map label font pin mismatch')
          if (disposed || lifetime.signal.aborted) throw Error('Map labels disposed')
          const candidate = await new FontFace(family, bytes).load()
          if (disposed || lifetime.signal.aborted) throw Error('Map labels disposed')
          face = candidate; document.fonts.add(candidate)
        } finally { clearTimeout(timeout) }
      })()
      return pending
    },
    draw(context: CanvasRenderingContext2D) {
      if (disposed || !face || !document.fonts.has(face)) throw Error('Pinned map label font not ready')
      const s = presentation.style
      function polygon(points: Polygon, color: string) {
        context.beginPath(); points.forEach(([x, y], i) => i ? context.lineTo(x!, y!) : context.moveTo(x!, y!))
        context.closePath(); context.fillStyle = color; context.fill()
      }
      context.save()
      try {
        for (const label of presentation.labels) {
          const r = label.rect, b = s.border_width
          context.fillStyle = s.border_fill; context.fillRect(r.x, r.y, r.width, r.height)
          context.fillStyle = s.paper_fill; context.fillRect(r.x + b, r.y + b, r.width - 2 * b, r.height - 2 * b)
          if (label.icon) {
            const x = r.x + s.padding, y = r.y + (r.height - s.icon_size) / 2, scale = s.icon_size / 24
            context.fillStyle = s.icon_paper; context.fillRect(x - 3, y - 3, s.icon_size + 6, s.icon_size + 6)
            const transformed = (shape: Polygon) => shape.map(([px, py]) => [x + px! * scale, y + py! * scale])
            for (const shape of icons[label.icon]!) polygon(transformed(shape), s.icon_fill)
            for (const shape of cutouts[label.icon] ?? []) polygon(transformed(shape), s.icon_paper)
          }
          context.font = `${s.font_size}px "${family}"`; context.textAlign = 'left'; context.textBaseline = 'alphabetic'
          context.fillStyle = s.text_fill
          const x = r.x + s.padding + (label.icon ? s.icon_cell : 0)
          const first = r.y + (r.height - label.lines.length * s.line_height) / 2 + 32
          label.lines.forEach((line, i) => context.fillText(line, x, first + i * s.line_height))
        }
      } finally { context.restore() }
    },
    dispose() { if (disposed) return; disposed = true; lifetime.abort(); if (face) document.fonts.delete(face); face = null },
  }
}
