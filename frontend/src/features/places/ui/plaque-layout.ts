export interface PlaqueAnchor { readonly id: string; readonly x: number; readonly y: number }
/** Move annotation boxes, never their source interaction points. Leaders retain
 * the exact target. Bounded best effort; the complete DOM list remains available.
 */
export function layoutPlaques(anchors: readonly PlaqueAnchor[], view: { left: number; top: number; width: number; height: number }, worldHeight: number) {
  const clamp = (n: number, min: number, max: number) => Math.max(min, Math.min(max, n))
  const result = new Map<string, { x: number; y: number }>()
  const placed: { x: number; y: number }[] = []
  for (const point of [...anchors].sort((a, b) => a.y - b.y || a.x - b.x)) {
    if (point.x < view.left || point.x > view.left + view.width || point.y < view.top || point.y > view.top + view.height) { result.set(point.id, { x: point.x, y: point.y }); continue }
    const x = clamp(point.x, view.left + 72, view.left + view.width - 72)
    const minY = view.top + 32, maxY = Math.max(minY, Math.min(worldHeight, view.top + view.height) - 32)
    let y = clamp(point.y, minY, maxY)
    for (let pass = 0; pass <= placed.length; pass++) {
      const collision = placed.find(p => Math.abs(p.x - x) < 140 && Math.abs(p.y - y) < 64)
      if (!collision) break
      const below = collision.y + 64
      y = below <= maxY ? below : clamp(collision.y - 64, minY, maxY)
    }
    const box = { x, y }; placed.push(box); result.set(point.id, box)
  }
  return result
}
