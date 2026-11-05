import { expect, it } from 'vitest'
import { readFileSync } from 'node:fs'
import { createHash } from 'node:crypto'
import type { DisplayMesh, Point, Rect } from '../../photo2-world-display-v1'

interface Frame {
  id: string; native: unknown; nativeFoot: Point; displayFoot: Point
  origin: Point; paintWidth: number; paintHeight: number; displayRect: Rect
  atlasRect: Rect; collisionSupport: string
}
interface Board { id: string; displayRect: Rect; atlasRect: Rect; destination: string; interactionId: null }
interface Input {
  mesh: DisplayMesh; nativeSource: unknown; frames: Frame[]; boards: Board[]
  atlas: { width: number; height: number; padding: number }; ground: unknown
  paintRevision: number; supersedesCandidateSHA256: string
  paintAmendment: { lamp: { stemLocalX: number; visualBaseLocalY: number; headAndShaftMinLocalX: number; groundBandMinLocalY: number; focusClearanceDisplayPx: number }; rearMinimumPaintedPixels: number }
}
const load = (revision: number) => readFileSync(new URL(`../../../../../../../docs/mcr/evidence/pi08-finish-01/photo2-world-contract/producer-input-v${revision}.json`, import.meta.url))
const previousBytes = load(2)
const previous = JSON.parse(previousBytes.toString()) as Input
const candidate = JSON.parse(load(3).toString()) as Input

it('places the front lamp paint and support beside WALL without moving native feet, and packs the changed24-frame atlas', () => {
  expect(candidate.paintRevision).toBe(3)
  expect(candidate.supersedesCandidateSHA256).toBe(createHash('sha256').update(previousBytes).digest('hex'))
  expect(candidate.mesh).toEqual(previous.mesh)
  expect(candidate.nativeSource).toEqual(previous.nativeSource)
  expect(candidate.ground).toEqual(previous.ground)
  expect(candidate.frames.map(f => f.id)).toEqual(previous.frames.map(f => f.id))
  expect(candidate.frames).toHaveLength(22)
  for (const [i, frame] of candidate.frames.entries()) {
    const old = previous.frames[i]!
    for (const key of ['native', 'nativeFoot', 'displayFoot', 'origin', 'collisionSupport'] as const) expect(frame[key]).toEqual(old[key])
    if (frame.id !== 'rooted_tree_06') {
      expect(frame.paintWidth).toBe(old.paintWidth); expect(frame.paintHeight).toBe(old.paintHeight)
      expect(frame.displayRect).toEqual(old.displayRect)
    }
  }
  expect(candidate.boards.map(({ atlasRect: _rect, ...board }) => board)).toEqual(previous.boards.map(({ atlasRect: _rect, ...board }) => board))
  const tree = candidate.frames.find(f => f.id === 'rooted_tree_06')!
  expect([tree.paintWidth, tree.paintHeight]).toEqual([80, 170])
  expect(tree.displayRect).toEqual({ x: 277.51, y: tree.displayFoot.y - 170, width: 80, height: 170 })
  expect(tree.displayRect.x + tree.paintWidth * tree.origin.x).toBeCloseTo(tree.displayFoot.x, 9)
  expect(tree.displayRect.y + tree.paintHeight * tree.origin.y).toBeCloseTo(tree.displayFoot.y, 9)
  const lamp = candidate.paintAmendment.lamp
  expect(tree.displayRect.x + lamp.stemLocalX).toBeCloseTo(341.51, 7)
  expect(tree.displayRect.y + lamp.visualBaseLocalY).toBeCloseTo(462.3561643835617, 7)
  const wall = candidate.boards.find(b => b.id === 'photo2-shortcut-wall')!.displayRect
  expect(tree.displayRect.x + lamp.headAndShaftMinLocalX).toBeGreaterThanOrEqual(wall.x + wall.width + lamp.focusClearanceDisplayPx)
  expect(tree.displayRect.y + lamp.groundBandMinLocalY).toBeGreaterThanOrEqual(wall.y + wall.height + lamp.focusClearanceDisplayPx)
  expect(tree.displayRect.y + lamp.groundBandMinLocalY).toBeGreaterThanOrEqual(464)
  expect(lamp.groundBandMinLocalY).toBeLessThan(tree.paintHeight)
  expect(candidate.paintAmendment.rearMinimumPaintedPixels).toBeGreaterThan(100)
  const all = [...candidate.frames, ...candidate.boards]
  expect(all).toHaveLength(24); expect(new Set(all.map(f => f.id)).size).toBe(24)
  expect(candidate.atlas).toEqual(previous.atlas)
  for (const [i, frame] of all.entries()) {
    const a = frame.atlasRect
    expect([a.width, a.height]).toEqual([frame.displayRect.width, frame.displayRect.height])
    expect([a.x, a.y, a.width, a.height].every(Number.isInteger)).toBe(true)
    expect(a.x).toBeGreaterThanOrEqual(2); expect(a.y).toBeGreaterThanOrEqual(2)
    expect(a.x + a.width).toBeLessThanOrEqual(510); expect(a.y + a.height).toBeLessThanOrEqual(1022)
    for (const b of all.slice(i + 1).map(f => f.atlasRect)) {
      expect(a.x + a.width + 4 <= b.x || b.x + b.width + 4 <= a.x || a.y + a.height + 4 <= b.y || b.y + b.height + 4 <= a.y).toBe(true)
    }
  }
})
