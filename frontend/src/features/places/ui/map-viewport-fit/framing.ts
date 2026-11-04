import { mapViewport } from '../reference-scene/viewport'

export type Size = { width: number; height: number }
export type ScrollPoint = { left: number; top: number }
export type MapViewMode = number | 'fit' | 'readable'
const clamp = (value: number, min: number, max: number) => Math.max(min, Math.min(max, value))

/** DOM presentation only. The qualified image and every source coordinate stay intact. */
export function readableMapFrame(slot: Size, image: Size, mode: MapViewMode) {
  const baseHeight = Math.max(slot.height, slot.width * image.height / image.width)
  // Preserve the incumbent minimum needed by the canonical 44px target packer.
  const minimum = 448 / baseHeight
  const contain = mapViewport(slot, image, -1).minZoom
  const minZoom = Math.max(minimum, contain)
  // A readable initial view fills the slot without stretching the raster. Any
  // overflow remains scrollable; Fit explicitly returns to the full overview.
  const requested = mode === 'readable' ? Math.max(1, minimum)
    : mode === 'fit' ? minZoom : mode
  return { ...mapViewport(slot, image, Math.max(minZoom, requested)), minZoom }
}

/** Preserve the same source point at the viewport centre when zooming/resizing. */
export function reframeScroll(previous: { plane: Size; slot: Size; scroll: ScrollPoint } | null,
  plane: Size, slot: Size): ScrollPoint {
  const x = previous ? (previous.scroll.left + previous.slot.width / 2) / previous.plane.width : .5
  const y = previous ? (previous.scroll.top + previous.slot.height / 2) / previous.plane.height : 0
  return {
    left: clamp(x * plane.width - slot.width / 2, 0, Math.max(0, plane.width - slot.width)),
    top: clamp(y * plane.height - slot.height / 2, 0, Math.max(0, plane.height - slot.height)),
  }
}

/** Reveal focus using this local scroller only, without moving the page or World. */
export function revealLandmark(scroll: ScrollPoint, slot: Size, plane: Size,
  target: ScrollPoint & Size, margin = 6): ScrollPoint {
  function axis(current: number, visible: number, full: number, start: number, length: number) {
    const next = length + margin * 2 > visible ? start + length / 2 - visible / 2
      : start - margin < current ? start - margin
        : start + length + margin > current + visible ? start + length + margin - visible : current
    return clamp(next, 0, Math.max(0, full - visible))
  }
  return { left: axis(scroll.left, slot.width, plane.width, target.left, target.width),
    top: axis(scroll.top, slot.height, plane.height, target.top, target.height) }
}
