import { describe, expect, it } from 'vitest'
import { readableMapFrame, reframeScroll, revealLandmark } from '../../ui/map-viewport-fit/framing'

const source = { width: 768, height: 1296 }
describe('qualified Map local framing', () => {
  for (const width of [326, 356, 396]) it(`fills the ${width}px portrait slot without altering source aspect or extents`, () => {
    const slot = { width, height: 637 }, frame = readableMapFrame(slot, source, 'readable')
    expect(frame.width).toBeGreaterThanOrEqual(slot.width)
    expect(frame.height).toBeGreaterThanOrEqual(slot.height)
    expect(frame.width / frame.height).toBeCloseTo(source.width / source.height, 10)
    // Both extreme landmarks can be revealed in full, rather than cut off by a
    // centred/overflow-hidden cover image. Their source-space anchors stay fixed.
    for (const edge of ['start', 'end']) {
      const target = { left: edge === 'start' ? 0 : frame.width - 44,
        top: edge === 'start' ? 0 : frame.height - 44, width: 44, height: 44 }
      const next = revealLandmark({ left: 0, top: 0 }, slot, frame, target)
      expect(target.left).toBeGreaterThanOrEqual(next.left)
      expect(target.left + 44).toBeLessThanOrEqual(next.left + slot.width)
      expect(target.top).toBeGreaterThanOrEqual(next.top)
      expect(target.top + 44).toBeLessThanOrEqual(next.top + slot.height)
    }
  })
  it('uses vertical local scrolling at short height instead of shrinking labels or losing the bottom landmark', () => {
    const slot = { width: 356, height: 213 }, frame = readableMapFrame(slot, source, 'readable')
    expect(frame.width).toBe(356)
    expect(frame.height).toBe(600.75)
    const next = revealLandmark({ left: 0, top: 0 }, slot, frame,
      { left: 200, top: frame.height - 44, width: 80, height: 44 })
    expect(next.top).toBe(frame.height - slot.height)
    expect(next.left).toBe(0)
  })
  it('keeps an explicit overview with the incumbent minimum height and numeric zoom bounds', () => {
    const short = readableMapFrame({ width: 356, height: 213 }, source, 'fit')
    expect(short.height).toBeCloseTo(448)
    expect(readableMapFrame({ width: 326, height: 637 }, source, 'fit').width).toBeCloseTo(326)
    expect(readableMapFrame({ width: 326, height: 637 }, source, -10).height).toBeGreaterThanOrEqual(448 - 1e-8)
    expect(readableMapFrame({ width: 326, height: 637 }, source, 100).effectiveZoom).toBe(3)
  })
  it('keeps the same source point under the viewport centre when zoom changes', () => {
    const old = { plane: { width: 800, height: 1350 }, slot: { width: 356, height: 400 }, scroll: { left: 150, top: 300 } }
    const next = reframeScroll(old, { width: 1200, height: 2025 }, old.slot)
    expect((next.left + 178) / 1200).toBeCloseTo((150 + 178) / 800)
    expect((next.top + 200) / 2025).toBeCloseTo((300 + 200) / 1350)
  })
  it('clamps at source edges on resize and starts reading at the top without a saved pan', () => {
    expect(reframeScroll(null, { width: 400, height: 675 }, { width: 326, height: 213 })).toEqual({ left: 37, top: 0 })
    expect(reframeScroll({ plane: { width: 400, height: 675 }, slot: { width: 326, height: 213 }, scroll: { left: 74, top: 462 } },
      { width: 356, height: 600.75 }, { width: 356, height: 637 })).toEqual({ left: 0, top: 0 })
  })
  it('does not move a fully visible landmark or mutate caller-owned geometry', () => {
    const scroll = { left: 30, top: 120 }, target = { left: 60, top: 150, width: 44, height: 44 }
    const before = JSON.stringify([scroll, target])
    expect(revealLandmark(scroll, { width: 326, height: 213 }, { width: 400, height: 675 }, target)).toEqual(scroll)
    expect(JSON.stringify([scroll, target])).toBe(before)
  })
})
