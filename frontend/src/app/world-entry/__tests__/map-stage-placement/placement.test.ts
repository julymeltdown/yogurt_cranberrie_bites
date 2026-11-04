import { expect, it } from 'vitest'
import { mapCompositionArchitectureObstacles } from '../../../../features/places/ui/IllustratedMap'
import { map4LocalZoom, packMap4Markers } from '../../../../world/reference-scene/map4-service-v1/layout'
import { currentComposition } from '../../../../world/reference-scene/map4-service-v2/current'
import { map4Parade } from '../../../../world/reference-scene/map4-service-v1/identity'
import { mapViewport } from '../../../../features/places/ui/reference-scene/viewport'
import approved from '../../../../world/reference-scene/map4-service-v2/layout-source.json'

type Rect = { left: number; top: number; width: number; height: number }
const intersects = (a: Rect, b: Rect) => a.left < b.left + b.width && a.left + a.width > b.left
  && a.top < b.top + b.height && a.top + a.height > b.top
const box = (p: { x: number; y: number }): Rect => ({ left: p.x - 22, top: p.y - 22, width: 44, height: 44 })
const model = currentComposition()
const identity = { ...map4Parade, presentationVersion: model.registration.version, presentationHash: model.registration.presentationHash }
const anchors = (width: number, height: number) => model.parsed.canonicalDisplayAnchors.map(a => ({
  id: a.interactionId, x: a.x / approved.dimensions.width * width, y: a.y / approved.dimensions.height * height,
}))

it('projects all 22 approved architecture footprints, including clipped edge venues, without mutating art', () => {
  const before = JSON.stringify(approved), rectangles = mapCompositionArchitectureObstacles(384, 648)
  expect(rectangles).toHaveLength(22)
  const central = approved.decorations.find(d => d.id === 'map_only_stage_central_hall')!
  const projected = rectangles.find(r => Math.abs(r.left - central.rect[0]! / 2) < .001)!
  expect(projected.left).toBeCloseTo(central.rect[0]! / 2); expect(projected.top).toBeCloseTo(central.rect[1]! / 2)
  expect(projected.width).toBeCloseTo(central.rect[2]! / 2); expect(projected.height).toBeCloseTo(central.rect[3]! / 2)
  expect(rectangles.some(r => r.left < 0)).toBe(true)
  expect(JSON.stringify(approved)).toBe(before)
})

it('finds a full-size marker in a narrow corridor missed by the retained 48px grid', () => {
  const excluded = [{ left: 0, top: 0, width: 200, height: 74 }, { left: 0, top: 134, width: 200, height: 66 },
    { left: 0, top: 74, width: 70, height: 60 }, { left: 130, top: 74, width: 70, height: 60 }]
  const packed = packMap4Markers([{ id: 'canonical', x: 100, y: 104 }], 200, 200, excluded)
  const rectangle = box(packed.get('canonical')!)
  expect(rectangle.width).toBe(44); expect(rectangle.height).toBe(44)
  for (const other of excluded) expect(intersects(rectangle, other)).toBe(false)
  expect(packMap4Markers([{ id: 'canonical', x: 100, y: 104 }], 200, 200, excluded)).toEqual(packed)
})

it('reproduces the retained 390px stage occlusion before architectural exclusions', () => {
  const width = 356, height = 600.75
  const original = packMap4Markers(anchors(width, height), width, height,
    [...model.obstacles(identity, width, height), ...model.targets(identity, width, height)])
  expect(mapCompositionArchitectureObstacles(width, height).some(r => intersects(box(original.get('stage_info')!), r))).toBe(true)
})

const slots = [326, 356, 396].flatMap(width => [681, 96].flatMap(height => [-1, 1.25, 3].map(zoom => ({ width, height, zoom }))))
it.each(slots)('keeps every canonical marker clear of approved buildings, labels, routes and controls at $width×$height zoom $zoom', slot => {
  const view = mapViewport(slot, identity, map4LocalZoom(slot, slot.zoom))
  const input = anchors(view.width, view.height), original = structuredClone(input)
  const architecture = mapCompositionArchitectureObstacles(view.width, view.height)
  const excluded = [...model.obstacles(identity, view.width, view.height), ...model.targets(identity, view.width, view.height),
    { left: view.width - 56, top: 8, width: 48, height: 92 }, ...architecture]
  const result = packMap4Markers(input, view.width, view.height, excluded, { preferAnchorProximity: true })
  expect(input).toEqual(original)
  expect([...result.keys()]).toEqual(input.map(a => a.id))
  const placed: Rect[] = []
  for (const point of result.values()) {
    const rectangle = box(point)
    expect(rectangle.left).toBeGreaterThanOrEqual(0); expect(rectangle.top).toBeGreaterThanOrEqual(0)
    expect(rectangle.left + rectangle.width).toBeLessThanOrEqual(view.width)
    expect(rectangle.top + rectangle.height).toBeLessThanOrEqual(view.height)
    for (const other of [...excluded, ...placed]) expect(intersects(rectangle, other)).toBe(false)
    placed.push(rectangle)
  }
})

it.each([326, 356, 396])('keeps the stage marker near its own anchor rather than a distant vacant grid cell at %spx', width => {
  const height = width * 1296 / 768, input = anchors(width, height)
  // Production items are ordered park, stage, gifts, wall, not the source-anchor order.
  input.sort((a, b) => ['park_info', 'stage_info', 'gifts_info', 'wall_info'].indexOf(a.id)
    - ['park_info', 'stage_info', 'gifts_info', 'wall_info'].indexOf(b.id))
  const excluded = [...model.obstacles(identity, width, height), ...model.targets(identity, width, height),
    ...mapCompositionArchitectureObstacles(width, height), { left: width - 56, top: height - 160, width: 48, height: 92 }]
  const result = packMap4Markers(input, width, height, excluded, { preferAnchorProximity: true })
  const anchor = input.find(a => a.id === 'stage_info')!, marker = result.get('stage_info')!
  expect(Math.hypot(marker.x - anchor.x, marker.y - anchor.y)).toBeLessThan(96)
  for (const other of excluded) expect(intersects(box(marker), other)).toBe(false)
})
