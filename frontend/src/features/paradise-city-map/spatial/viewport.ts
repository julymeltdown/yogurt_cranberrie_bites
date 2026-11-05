import { imagePoint, requireSiteplan, type ImagePoint, type ImageRect } from './contract'

export interface Extent { readonly width: number; readonly height: number }
export interface ViewportPoint { readonly x: number; readonly y: number }
export interface ViewportTransform { readonly scale: number; readonly offsetX: number; readonly offsetY: number }
export type RecenterIntent = Readonly<{ kind: 'fit' }> | Readonly<{ kind: 'image-point'; point: ImagePoint }>
function extent(value: Extent) { requireSiteplan(Number.isFinite(value.width) && Number.isFinite(value.height) && value.width >= 1 && value.height >= 1 && value.width <= 1_000_000 && value.height <= 1_000_000) }
function finite(...values: number[]) { requireSiteplan(values.every(Number.isFinite)) }
function clamp(value: number, min: number, max: number) { return Math.min(max, Math.max(min, value)) }
export function fitViewport(image: Extent, viewport: Extent): ViewportTransform {
  extent(image); extent(viewport)
  const scale = Math.min(viewport.width / image.width, viewport.height / image.height)
  return Object.freeze({ scale, offsetX: (viewport.width - image.width * scale) / 2, offsetY: (viewport.height - image.height * scale) / 2 })
}
export function constrainViewport(image: Extent, viewport: Extent, transform: ViewportTransform): ViewportTransform {
  const fit = fitViewport(image, viewport); finite(transform.scale, transform.offsetX, transform.offsetY)
  requireSiteplan(transform.scale > 0)
  const scale = clamp(transform.scale, fit.scale, Math.max(1, fit.scale * 8))
  const width = image.width * scale, height = image.height * scale
  return Object.freeze({ scale, offsetX: width <= viewport.width ? (viewport.width - width) / 2 : clamp(transform.offsetX, viewport.width - width, 0), offsetY: height <= viewport.height ? (viewport.height - height) / 2 : clamp(transform.offsetY, viewport.height - height, 0) })
}
export function imageToViewport(point: ImagePoint, transform: ViewportTransform): ViewportPoint {
  finite(point.x, point.y, transform.scale, transform.offsetX, transform.offsetY); requireSiteplan(transform.scale > 0)
  return Object.freeze({ x: point.x * transform.scale + transform.offsetX, y: point.y * transform.scale + transform.offsetY })
}
export function viewportToImage(point: ViewportPoint, transform: ViewportTransform, image: Extent): ImagePoint | null {
  extent(image); finite(point.x, point.y, transform.scale, transform.offsetX, transform.offsetY); requireSiteplan(transform.scale > 0)
  const x = (point.x - transform.offsetX) / transform.scale, y = (point.y - transform.offsetY) / transform.scale
  return x >= 0 && y >= 0 && x < image.width && y < image.height ? imagePoint(x, y) : null
}
export function panViewport(image: Extent, viewport: Extent, transform: ViewportTransform, delta: ViewportPoint): ViewportTransform {
  finite(delta.x, delta.y)
  return constrainViewport(image, viewport, { scale: transform.scale, offsetX: transform.offsetX + delta.x, offsetY: transform.offsetY + delta.y })
}
export function zoomViewport(image: Extent, viewport: Extent, transform: ViewportTransform, factor: number, around: ViewportPoint): ViewportTransform {
  finite(factor, around.x, around.y); requireSiteplan(factor > 0 && transform.scale > 0)
  const bounded = constrainViewport(image, viewport, transform)
  const fit = fitViewport(image, viewport), scale = clamp(bounded.scale * factor, fit.scale, Math.max(1, fit.scale * 8))
  const x = (around.x - bounded.offsetX) / bounded.scale, y = (around.y - bounded.offsetY) / bounded.scale
  return constrainViewport(image, viewport, { scale, offsetX: around.x - x * scale, offsetY: around.y - y * scale })
}
export function revealImageRect(image: Extent, viewport: Extent, transform: ViewportTransform, rect: ImageRect, margin = 16): ViewportTransform {
  extent(image); extent(viewport); finite(rect.x, rect.y, rect.width, rect.height, margin)
  requireSiteplan(rect.x >= 0 && rect.y >= 0 && rect.width > 0 && rect.height > 0 && rect.x + rect.width <= image.width && rect.y + rect.height <= image.height && margin >= 0 && margin * 2 < Math.min(viewport.width, viewport.height))
  const current = constrainViewport(image, viewport, transform), scale = Math.min(current.scale, (viewport.width - margin * 2) / rect.width, (viewport.height - margin * 2) / rect.height)
  return constrainViewport(image, viewport, { scale, offsetX: viewport.width / 2 - (rect.x + rect.width / 2) * scale, offsetY: viewport.height / 2 - (rect.y + rect.height / 2) * scale })
}
export function recenterViewport(image: Extent, viewport: Extent, transform: ViewportTransform, intent: RecenterIntent): ViewportTransform {
  if (intent.kind === 'fit') return fitViewport(image, viewport)
  const { point } = intent; finite(point.x, point.y)
  requireSiteplan(point.x >= 0 && point.y >= 0 && point.x < image.width && point.y < image.height)
  return constrainViewport(image, viewport, { scale: transform.scale, offsetX: viewport.width / 2 - point.x * transform.scale, offsetY: viewport.height / 2 - point.y * transform.scale })
}
