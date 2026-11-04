import { isMap4Presentation, map4Labels, type Map4PresentationIdentity } from './annotations'

/** Readable local MAP scale on short screens; scroll instead of shrinking 44px actions. */
export function map4LocalZoom(slot: { width: number; height: number }, zoom: number) {
  const baseHeight = Math.max(slot.width * 1296 / 768, slot.height)
  return Math.max(448 / baseHeight, zoom)
}
export function map4ServiceTargets(identity: Map4PresentationIdentity, width: number, height: number) {
  if (!isMap4Presentation(identity)) return []
  return map4Labels.filter(l => l.intent !== null).map(label => {
    const w = Math.max(44, label.rect.width / 768 * width), h = Math.max(44, label.rect.height / 1296 * height)
    return { intent: label.intent!, label: label.label,
      left: Math.max(0, Math.min(width - w, (label.rect.x + label.rect.width / 2) / 768 * width - w / 2)),
      top: Math.max(0, Math.min(height - h, (label.rect.y + label.rect.height / 2) / 1296 * height - h / 2)), width: w, height: h }
  })
}

type Rect = { left: number; top: number; width: number; height: number }
/** Compact canonical markers stay separate from virtual feature hit regions. */
export function packMap4Markers(anchors: readonly { id: string; x: number; y: number }[], width: number, height: number, excluded: readonly Rect[]) {
  const placed: Rect[] = [], result = new Map<string, { x: number; y: number }>()
  const overlaps = (a: Rect, b: Rect) => a.left < b.left + b.width + 4 && a.left + a.width + 4 > b.left
    && a.top < b.top + b.height + 4 && a.top + a.height + 4 > b.top
  for (const anchor of anchors) {
    const candidates: { x: number; y: number }[] = []
    for (let y = 24; y <= height - 24; y += 48) for (let x = 24; x <= width - 24; x += 48) candidates.push({ x, y })
    candidates.sort((a, b) => (a.x - anchor.x) ** 2 + (a.y - anchor.y) ** 2 - (b.x - anchor.x) ** 2 - (b.y - anchor.y) ** 2)
    const point = candidates.find(p => {
      const r = { left: p.x - 22, top: p.y - 22, width: 44, height: 44 }
      return [...excluded, ...placed].every(e => !overlaps(r, e))
    })
    if (!point) throw Error('Map4 marker space unavailable')
    result.set(anchor.id, point); placed.push({ left: point.x - 22, top: point.y - 22, width: 44, height: 44 })
  }
  return result
}
