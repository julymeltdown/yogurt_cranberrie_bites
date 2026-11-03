export interface Point { x: number; y: number }
const clamp = (v: number, min: number, max: number) => Math.min(max, Math.max(min, v))
export interface View { zoom: number; x: number; y: number }
export const fitView = (): View => ({ zoom: 1, x: 0, y: 0 })
export function boundView(view: View, size: number): View {
  const zoom = clamp(view.zoom, 1, 8), limit = Math.max(0, size * (zoom - 1))
  return { zoom, x: clamp(view.x, -limit, 0), y: clamp(view.y, -limit, 0) }
}
export function zoomView(view: View, zoom: number, size: number): View {
  const factor = clamp(zoom, 1, 8) / view.zoom
  return boundView({ zoom: view.zoom * factor, x: size / 2 - (size / 2 - view.x) * factor, y: size / 2 - (size / 2 - view.y) * factor }, size)
}
export function pixelAt(point: Point, view: View, size: number): Point | null {
  if (size <= 0 || !Number.isFinite(size)) return null
  const x = Math.floor((point.x - view.x) / (size * view.zoom) * 128), y = Math.floor((point.y - view.y) / (size * view.zoom) * 128)
  return x >= 0 && x < 128 && y >= 0 && y < 128 ? { x, y } : null
}
/** A gesture can preview OR pan. Multitouch/long press/cancel never preview and never mutate. */
export function createPointerInput({ select, pan, cancel, now = () => performance.now() }: {
  select: (p: Point) => void; pan: (delta: Point) => void; cancel: () => void; now?: () => number
}) {
  const pointers = new Set<number>()
  let first: { id: number; origin: Point; last: Point; at: number; moved: boolean; ambiguous: boolean } | null = null
  return {
    down(id: number, point: Point) {
      if (pointers.size >= 2) { if (first) first.ambiguous = true; return }
      pointers.add(id); cancel()
      if (pointers.size > 1) { if (first) first.ambiguous = true; return }
      first = { id, origin: point, last: point, at: now(), moved: false, ambiguous: false }
    },
    move(id: number, point: Point) {
      if (!first || id !== first.id || first.ambiguous) return
      if (Math.hypot(point.x - first.origin.x, point.y - first.origin.y) >= 6) first.moved = true
      if (first.moved) pan({ x: point.x - first.last.x, y: point.y - first.last.y })
      first.last = point
    },
    up(id: number, point: Point) {
      pointers.delete(id)
      if (first?.id === id) {
        if (!first.moved && !first.ambiguous && now() - first.at < 600 && Math.hypot(point.x - first.origin.x, point.y - first.origin.y) < 6) select(point)
        first = null
      }
      if (!pointers.size) first = null
    },
    lost(id: number) { if (pointers.has(id)) { pointers.clear(); first = null; cancel() } },
    cancel() { pointers.clear(); first = null; cancel() },
  }
}
