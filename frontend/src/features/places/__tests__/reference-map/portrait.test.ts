import { expect, it } from 'vitest'
import { mapViewport, overviewIdentity } from '../../ui/reference-scene/viewport'
import { referenceAnnotations } from '../../ui/reference-scene/annotations'
import proposal from '../../../../../public/world/reference-scene-v2/geometry-proposal.json'
import type { SceneryOverview } from '../../../../world/illustrated/overview'

it.each([[326, 718], [356, 718], [396, 718], [356, 304]])('fits the entire portrait without stretching or clipping at %sx%s', (width, height) => {
  const slot = { width, height }, image = { width: 768, height: 1296 }
  const cover = mapViewport(slot, image, 1), fit = mapViewport(slot, image, cover.minZoom)
  expect(fit.width).toBeLessThanOrEqual(width + 1e-8)
  expect(fit.height).toBeLessThanOrEqual(height + 1e-8)
  expect(fit.width / fit.height).toBeCloseTo(768 / 1296)
  expect(fit.width + fit.offsetX * 2).toBeCloseTo(width)
  expect(fit.height + fit.offsetY * 2).toBeCloseTo(height)
  expect(mapViewport(slot, image, -1).effectiveZoom).toBe(fit.minZoom)
  expect(mapViewport(slot, image, 100).effectiveZoom).toBe(3)
})

it.each([[326, 718], [356, 718], [396, 718], [356, 304]])('keeps four fitted labels apart and clear of short-view zoom at %sx%s', (width, height) => {
  const slot = { width, height }, image = { width: 768, height: 1296 }
  const cover = mapViewport(slot, image, 1), fit = mapViewport(slot, image, cover.minZoom)
  const view = { left: -fit.offsetX, top: -fit.offsetY, width, height }
  const anchors = proposal.geometry.interactions.map(p => ({ id: p.place_id, x: (p.cell_x + .5) / 48 * fit.width, y: (p.cell_y + .5) / 81 * fit.height }))
  const original = structuredClone(anchors)
  const zoom = { left: view.left + width - 56, top: view.top + 8, width: 48, height: 92 }
  const labels = [...referenceAnnotations(anchors, view, [zoom]).values()]
  expect(anchors).toEqual(original)
  expect(labels).toHaveLength(4)
  for (const [i, a] of labels.entries()) {
    expect(a.x - 70).toBeGreaterThanOrEqual(view.left)
    expect(a.x + 70).toBeLessThanOrEqual(view.left + width)
    expect(a.y - 36).toBeGreaterThanOrEqual(view.top)
    expect(a.y + 36).toBeLessThanOrEqual(view.top + height)
    expect(a.x + 70 <= zoom.left || a.x - 70 >= zoom.left + zoom.width || a.y + 36 <= zoom.top || a.y - 36 >= zoom.top + zoom.height).toBe(true)
    for (const b of labels.slice(i + 1)) expect(Math.abs(a.x - b.x) >= 140 || Math.abs(a.y - b.y) >= 72).toBe(true)
  }
})

it('fences image identity by release, theme, dimensions and URL rather than URL alone', () => {
  const original: SceneryOverview = { mapId: 'mcr_reference_scene_r3', mapRevision: 1, mapHash: 'sha256:map', geometryHash: proposal.geometry_hash,
    width: 768, height: 1296, themeId: 'PARADE', themeRevision: 1, themeHash: 'sha256:theme', imageUrl: 'blob:test-only' }
  const key = overviewIdentity(original)
  for (const replacement of [{ mapRevision: 2 }, { mapHash: 'sha256:next' }, { geometryHash: 'sha256:next' }, { themeId: 'REVENGE' },
    { themeRevision: 2 }, { themeHash: 'sha256:next' }, { width: 769 }, { height: 1297 }, { imageUrl: 'blob:replacement' }]) {
    expect(overviewIdentity({ ...original, ...replacement })).not.toBe(key)
  }
})
