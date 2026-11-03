import { expect, it } from 'vitest'
import { layoutPlaques } from '../../ui/plaque-layout'
import geometry from '../../../../../public/world/maps/geometry.json'
it.each([326, 356, 396])('keeps the five authorized fit-view annotations readable without moving interaction anchors at %spx', width => {
  const height = width * geometry.height_cells / geometry.width_cells
  const anchors = geometry.interactions.map(p => ({ id: p.place_id, x: (p.cell_x + .5) / geometry.width_cells * width, y: (p.cell_y + .5) / geometry.height_cells * height }))
  const original = structuredClone(anchors), result = layoutPlaques(anchors, { left: 0, top: 0, width, height: 727 }, height)
  expect(anchors).toEqual(original)
  const labels = [...result.values()]
  for (const [i, a] of labels.entries()) {
    expect(a.x).toBeGreaterThanOrEqual(72); expect(a.x).toBeLessThanOrEqual(width - 72)
    for (const b of labels.slice(i + 1)) expect(Math.abs(a.x - b.x) >= 140 || Math.abs(a.y - b.y) >= 64).toBe(true)
  }
})
