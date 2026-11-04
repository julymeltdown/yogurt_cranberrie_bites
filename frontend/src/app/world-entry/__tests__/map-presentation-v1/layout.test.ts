import { expect, it } from 'vitest'
import { map4LocalZoom, map4ServiceTargets, packMap4Markers } from '../../../../world/reference-scene/map4-service-v1/layout'
import { map4LabelObstacles, presentationHash, presentationVersion } from '../../../../world/reference-scene/map4-service-v1/annotations'
import { map4Parade } from '../../map-presentation-v1/identity'
import { mapViewport } from '../../../../features/places/ui/reference-scene/viewport'
import { loadedMap4 } from './fixture'

const overlaps = (a: { left: number; top: number; width: number; height: number }, b: typeof a) =>
  a.left < b.left + b.width && a.left + a.width > b.left && a.top < b.top + b.height && a.top + a.height > b.top
it.each([{ width: 360, height: 520 }, { width: 390, height: 520 }, { width: 430, height: 520 }, { width: 390, height: 96 }])('packs full 44px actions and distinct canonical markers in $width×$height local viewport', async slot => {
  const identity = { ...map4Parade, presentationHash, presentationVersion }
  const view = mapViewport(slot, identity, map4LocalZoom(slot, -1))
  expect(view.height).toBeGreaterThanOrEqual(448)
  const targets = map4ServiceTargets(identity, view.width, view.height)
  expect(targets).toHaveLength(4)
  for (const t of targets) {
    expect(t.width).toBeGreaterThanOrEqual(44); expect(t.height).toBeGreaterThanOrEqual(44)
    expect(t.left).toBeGreaterThanOrEqual(0); expect(t.top).toBeGreaterThanOrEqual(0)
    expect(t.left + t.width).toBeLessThanOrEqual(view.width + .001)
    expect(t.top + t.height).toBeLessThanOrEqual(view.height + .001)
    for (const other of targets) if (t !== other) expect(overlaps(t, other)).toBe(false)
  }
  const map = await loadedMap4(), anchors = map.map.geometry.interactions.map(p => ({ id: p.place_id,
    x: (p.cell_x + .5) / map.map.geometry.width_cells * view.width, y: (p.cell_y + .5) / map.map.geometry.height_cells * view.height }))
  const controls = [{ left: view.width - 56, top: 8, width: 48, height: 92 }]
  const excluded = [...targets, ...map4LabelObstacles(identity, view.width, view.height), ...controls]
  const markers = packMap4Markers(anchors, view.width, view.height, excluded), rects: typeof controls = []
  expect(markers.size).toBe(anchors.length)
  for (const p of markers.values()) {
    const r = { left: p.x - 22, top: p.y - 22, width: 44, height: 44 }
    for (const other of [...excluded, ...rects]) expect(overlaps(r, other)).toBe(false)
    rects.push(r)
  }
})
