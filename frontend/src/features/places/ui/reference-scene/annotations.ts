import type { PlaqueAnchor } from '../plaque-layout'

export interface AnnotationRect { left: number; top: number; width: number; height: number }
/** Labels may occupy letterbox space. Exact source anchors and target pins stay
 * immutable; connector lines express any presentation-only displacement. */
export function referenceAnnotations(anchors: readonly PlaqueAnchor[], view: AnnotationRect, excluded: readonly AnnotationRect[] = []) {
  const clamp = (n: number, min: number, max: number) => Math.max(min, Math.min(max, n))
  const minX = view.left + 72, maxX = Math.max(minX, view.left + view.width - 72)
  const minY = view.top + 36, maxY = Math.max(minY, view.top + view.height - 36)
  const placed: { x: number; y: number }[] = [], result = new Map<string, { x: number; y: number }>()
  for (const anchor of anchors) {
    if (anchor.x < view.left || anchor.x > view.left + view.width || anchor.y < view.top || anchor.y > view.top + view.height) {
      result.set(anchor.id, { x: anchor.x, y: anchor.y }); continue
    }
    const x = clamp(anchor.x, minX, maxX), y = clamp(anchor.y, minY, maxY)
    const xs = maxX - minX >= 140 ? [minX, maxX] : [(minX + maxX) / 2]
    const ys: number[] = []
    for (let row = minY; row <= maxY; row += 72) ys.push(row)
    const candidates = xs.flatMap(cx => ys.map(cy => ({ x: cx, y: cy })))
      .sort((a, b) => (a.x - x) ** 2 + (a.y - y) ** 2 - ((b.x - x) ** 2 + (b.y - y) ** 2))
    const box = candidates.find(c => placed.every(p => Math.abs(p.x - c.x) >= 140 || Math.abs(p.y - c.y) >= 72)
      && excluded.every(r => c.x + 70 <= r.left || c.x - 70 >= r.left + r.width || c.y + 36 <= r.top || c.y - 36 >= r.top + r.height)) ?? { x, y }
    placed.push(box); result.set(anchor.id, box)
  }
  return result
}
