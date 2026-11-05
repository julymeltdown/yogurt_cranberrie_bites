import { describe, expect, it } from 'vitest'
import { imagePoint } from '../../spatial/contract'
import { fitViewport, imageToViewport, viewportToImage, panViewport, zoomViewport, constrainViewport, revealImageRect, recenterViewport } from '../../spatial/viewport'

const image = { width: 1024, height: 1280 }
describe('reversible nongeographic image viewport', () => {
  it.each([360, 390, 430])('contains the complete plan and round-trips points at %spx', width => {
    const viewport = { width, height: 620 }, fit = fitViewport(image, viewport)
    expect(image.width * fit.scale).toBeLessThanOrEqual(width)
    expect(image.height * fit.scale).toBeLessThanOrEqual(620)
    for (const point of [imagePoint(0, 0), imagePoint(500, 900), imagePoint(1023, 1279)]) {
      const back = viewportToImage(imageToViewport(point, fit), fit, image)!
      expect(back.x).toBeCloseTo(point.x, 9); expect(back.y).toBeCloseTo(point.y, 9)
    }
    expect(viewportToImage({ x: -1, y: -1 }, fit, image)).toBeNull()
  })
  it('preserves the pointer anchor when zooming in the interior, then clamps extreme pan', () => {
    const viewport = { width: 390, height: 620 }, fit = fitViewport(image, viewport), point = { x: 195, y: 310 }
    const original = viewportToImage(point, fit, image)!, zoom = zoomViewport(image, viewport, fit, 3, point)
    const after = viewportToImage(point, zoom, image)!
    expect(after.x).toBeCloseTo(original.x, 9); expect(after.y).toBeCloseTo(original.y, 9)
    const pan = panViewport(image, viewport, zoom, { x: 1e9, y: -1e9 })
    expect(pan.offsetX).toBe(0); expect(pan.offsetY).toBeCloseTo(viewport.height - image.height * pan.scale)
  })
  it('reveals a selected source region without destructive image cropping and returns to fit', () => {
    const viewport = { width: 390, height: 420 }, fit = fitViewport(image, viewport), zoom = zoomViewport(image, viewport, fit, 5, { x: 195, y: 210 })
    const rect = { x: 450, y: 850, width: 100, height: 100 }, reveal = revealImageRect(image, viewport, zoom, rect)
    const a = imageToViewport(imagePoint(rect.x, rect.y), reveal), b = imageToViewport(imagePoint(rect.x + rect.width, rect.y + rect.height), reveal)
    expect(a.x).toBeGreaterThanOrEqual(16); expect(a.y).toBeGreaterThanOrEqual(16)
    expect(b.x).toBeLessThanOrEqual(374); expect(b.y).toBeLessThanOrEqual(404)
    expect(recenterViewport(image, viewport, reveal, { kind: 'fit' })).toEqual(fit)
    expect(constrainViewport(image, viewport, reveal)).toEqual(reveal)
  })
  it('rejects invalid transforms and viewports instead of inventing a GPS transform', () => {
    expect(() => fitViewport(image, { width: 0, height: 620 })).toThrow()
    expect(() => constrainViewport(image, { width: 390, height: 620 }, { scale: NaN, offsetX: 0, offsetY: 0 })).toThrow()
    expect(() => zoomViewport(image, { width: 390, height: 620 }, fitViewport(image, { width: 390, height: 620 }), 0, { x: 0, y: 0 })).toThrow()
  })
})
