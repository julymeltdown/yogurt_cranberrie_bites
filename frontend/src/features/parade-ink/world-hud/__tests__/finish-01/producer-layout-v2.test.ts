import { expect, it } from 'vitest'
import { execFileSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { createHash } from 'node:crypto'
import { createPhoto2Projection, PHOTO2_NATIVE_ANCHORS, type DisplayMesh, type Point, type Rect } from '../../photo2-world-display-v1'

interface PaintFrame {
  id: string; native: { id: string; x: number; y: number; width: number; height: number; depth_y: number }
  nativeFoot: Point; displayFoot: Point; displayRect: Rect; paintWidth: number; paintHeight: number
  atlasRect: Rect
}
interface Candidate {
  mesh: DisplayMesh; frames: PaintFrame[]
  boards: { id: string; interactionId: null; destination: string; atlasRect: Rect; displayRect: Rect }[]
  ground: { displayBounds: Rect }; atlas: { width: number; height: number }
}
const data = readFileSync(new URL('../../../../../../../docs/mcr/evidence/pi08-finish-01/photo2-world-contract/producer-input-v2.json', import.meta.url))
const candidate = JSON.parse(data.toString()) as Candidate
const close = (a: Point | null, b: Point) => { expect(a).not.toBeNull(); expect(a!.x).toBeCloseTo(b.x, 7); expect(a!.y).toBeCloseTo(b.y, 7) }

it('qualifies the corrected204-vertex producer layout and22 frame feet without admitting pixels or runtime', () => {
  expect(createHash('sha256').update(data).digest('hex')).toBe('da87fef51caf2333e2fb9b05755efff0046c7254f0b399bc234b1308697c52fd')
  const p = createPhoto2Projection(candidate.mesh)
  expect(candidate.mesh.display).toHaveLength(204)
  expect(candidate.frames).toHaveLength(22)
  for (const anchor of PHOTO2_NATIVE_ANCHORS) {
    const matches = candidate.frames.filter(f => f.id === anchor.id)
    expect(matches).toHaveLength(1)
    const frame = matches[0]!
    expect(frame.native).toEqual(anchor)
    expect(frame.nativeFoot).toEqual({ x: anchor.x + anchor.width / 2, y: anchor.depth_y })
    close(p.forward(frame.nativeFoot), frame.displayFoot)
    close(p.inverse(frame.displayFoot), frame.nativeFoot)
    expect(frame.displayRect.width).toBe(frame.paintWidth); expect(frame.displayRect.height).toBe(frame.paintHeight)
  }
  // Independently measured Photo2 source regions: new mesh places these actual
  // native foot anchors at the reference-aligned art frame, not per-object shifts.
  expect(candidate.frames.find(f => f.id === 'main_stage')!.displayRect).toEqual({ x: 0, y: 4, width: 211, height: 211 })
  expect(candidate.frames.find(f => f.id === 'north_white_food')!.displayRect).toEqual({ x: 275, y: 42, width: 109, height: 141 })
  for (const y of candidate.mesh.rows) for (const x of candidate.mesh.columns) close(p.inverse(p.forward({ x, y })!), { x, y })
  const source = JSON.parse(execFileSync('git', ['show', '8dee1335a79f831e6a29f005a50f2c0f65777d46:frontend/src/world/reference-scene/spatial-v3/source-inputs.json']).toString()) as { geometry: { collision_grid: number[][] } }
  const bounds = candidate.ground.displayBounds
  // Source support coverage only: ART must later prove those support regions are
  // actually depicted. This cannot be a pixel/occlusion/native acceptance claim.
  for (let y = 0; y < source.geometry.collision_grid.length; y++) {
    for (let x = 0; x < source.geometry.collision_grid[y]!.length; x++) {
      if (!source.geometry.collision_grid[y]![x]) continue
      for (const [dx, dy] of [[0, 0], [1, 0], [1, 1], [0, 1], [.5, .5]] as const) {
        const native = { x: (x + dx) * 16, y: (y + dy) * 16 }, display = p.forward(native)!
        expect(display).not.toBeNull(); close(p.inverse(display), native)
        expect(display.x).toBeGreaterThanOrEqual(bounds.x - 1e-7)
        expect(display.y).toBeGreaterThanOrEqual(bounds.y - 1e-7)
        expect(display.x).toBeLessThanOrEqual(bounds.x + bounds.width + 1e-7)
        expect(display.y).toBeLessThanOrEqual(bounds.y + bounds.height + 1e-7)
      }
    }
  }
  const frames = [...candidate.frames, ...candidate.boards]
  expect(new Set(frames.map(f => f.id)).size).toBe(24)
  for (let i = 0; i < frames.length; i++) {
    const a = frames[i]!.atlasRect
    expect(a.x).toBeGreaterThanOrEqual(2); expect(a.y).toBeGreaterThanOrEqual(2)
    expect(a.x + a.width).toBeLessThanOrEqual(candidate.atlas.width - 2)
    expect(a.y + a.height).toBeLessThanOrEqual(candidate.atlas.height - 2)
    for (const b of frames.slice(i + 1).map(f => f.atlasRect)) expect(a.x + a.width <= b.x || b.x + b.width <= a.x || a.y + a.height <= b.y || b.y + b.height <= a.y).toBe(true)
  }
  expect(candidate.boards.map(b => b.destination)).toEqual(['/meet', '/wall'])
  expect(candidate.boards.every(b => b.interactionId === null)).toBe(true)
  expect(bounds.width * bounds.height + candidate.atlas.width * candidate.atlas.height).toBeLessThanOrEqual(2097152)
})
