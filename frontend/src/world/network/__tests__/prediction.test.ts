import { describe, expect, it } from 'vitest'
import { createMotionPrediction, createPointMotion, qualifyMotionContext, type MotionGeometry } from '../prediction'
import type { MotionProfile, SelfMotion } from '../WorldTransport'
import manifest from '../../../../../contracts/fixtures/assets/map-manifest.json'
import vectors from '../../../../../contracts/fixtures/realtime/motion-v1/vectors.json'

const geometry = (): MotionGeometry => ({ width_cells: 4, height_cells: 4, tile_width_px: 16, tile_height_px: 16,
  collision_grid: Array.from({ length: 4 }, () => Array(4).fill(0) as number[]) })

describe('FE17 public point collision primitive', () => {
  it('uses explicit distance, normalized rounding and analog strength without a speed default', () => {
    const map = createPointMotion(geometry()), from = { x: 24, y: 24 }
    expect(map.step(from, { axisX: 1, axisY: 0 }, 8)).toEqual({ x: 32, y: 24 })
    expect(map.step(from, { axisX: .5, axisY: 0 }, 8)).toEqual({ x: 28, y: 24 })
    expect(map.step(from, { axisX: 0, axisY: 0 }, 8)).toEqual(from)
    const diagonal = map.step(from, { axisX: Math.SQRT1_2, axisY: Math.SQRT1_2 }, 8)
    expect(Math.hypot(diagonal.x - from.x, diagonal.y - from.y)).toBeCloseTo(8, 12)
    expect(() => map.step(from, { axisX: 1, axisY: 1 }, 8)).toThrow()
    expect(() => map.step(from, { axisX: NaN, axisY: 0 }, 8)).toThrow()
  })
  it('stops the whole step at a blocked segment and does not slide along the wall', () => {
    const g = geometry(); g.collision_grid[1]![2] = 1
    const map = createPointMotion(g), from = { x: 24, y: 24 }
    expect(map.step(from, { axisX: 1, axisY: 0 }, 24)).toEqual(from)
    expect(map.step(from, { axisX: Math.SQRT1_2, axisY: Math.SQRT1_2 }, 24)).toEqual(from)
  })
  it('checks both corner neighbors and the adjacent side of an exact grid-edge path', () => {
    const g = geometry(); g.collision_grid[0]![1] = 1
    const map = createPointMotion(g)
    expect(map.clearSegment({ x: 8, y: 8 }, { x: 24, y: 24 })).toBe(false)
    expect(map.walkable({ x: 16, y: 8 })).toBe(false)
    expect(map.clearSegment({ x: 16, y: 24 }, { x: 16, y: 8 })).toBe(false)
  })
  it('rejects world bounds/nonfinite positions and owns an immutable copy of collision data', () => {
    const g = geometry(), map = createPointMotion(g)
    expect(map.walkable({ x: 0, y: 8 })).toBe(false)
    expect(map.walkable({ x: 64, y: 24 })).toBe(false)
    expect(map.walkable({ x: NaN, y: 24 })).toBe(false)
    g.collision_grid[1]![1] = 1
    expect(map.walkable({ x: 24, y: 24 })).toBe(true)
    expect(() => createPointMotion({ ...g, width_cells: 0 })).toThrow()
    expect(() => createPointMotion({ ...g, collision_grid: [[0]] })).toThrow()
  })
})

const profile: MotionProfile = { version: '1.0.0', geometryHash: vectors.geometryHash, stepMs: 50,
  speedPxPerSecond: vectors.speedPxPerSecond, holdMs: vectors.holdMs, maxCatchUpSteps: 4,
  axes: 'UNIT_DISK_ANALOG', collision: 'POINT_SUPERCOVER_STOP', catchUp: 'DISCARD_OLDEST_DUE_STEPS', holdBoundary: 'AGE_LE_HOLD_MS' }
const qualify = () => qualifyMotionContext({ version: '1.0.0', map: manifest })
const anchor = (patch: Partial<SelfMotion> = {}): SelfMotion => ({ serverTick: 0, position: { x: 48, y: 48 },
  consumedInputSeq: 0, inputAppliedTicks: 0, droppedTicks: 0, heldInput: null, ...patch })
const close = (actual: { x: number; y: number } | null, expected: { x: number; y: number }) => {
  expect(actual).not.toBeNull()
  expect(Math.abs(actual!.x - expected.x)).toBeLessThanOrEqual(vectors.tolerance)
  expect(Math.abs(actual!.y - expected.y)).toBeLessThanOrEqual(vectors.tolerance)
}

describe('FE17 negotiated bounded presentation', () => {
  it('qualifies exact C06 geometry and rejects mismatched/unsupported profiles', async () => {
    const context = await qualify()
    expect(createMotionPrediction(context, { ...profile, geometryHash: 'sha256:' + '0'.repeat(64) })).toBeNull()
    expect(createMotionPrediction(context, { ...profile, speedPxPerSecond: 0 })).toBeNull()
    expect(createMotionPrediction(context, { ...profile, collision: 'SLIDE' } as unknown as MotionProfile)).toBeNull()
    await expect(qualifyMotionContext({ version: '1.0.0', map: { ...manifest, geometry_hash: 'sha256:' + '0'.repeat(64) } })).rejects.toThrow()
  })
  it('uses the shared five wall/corner/grid-edge/bounds/walkable vectors', async () => {
    const context = await qualify()
    for (const segment of vectors.segments) expect(context.point.clearSegment(segment.from, segment.to), segment.name).toBe(segment.clear)
  })
  for (const vector of vectors.cases) it(`consumes producer anchors and resets for shared ${vector.name}`, async () => {
    const context = await qualify()
    let model = createMotionPrediction(context, profile)!, sequence = 0, previous: { motion: SelfMotion; ack: number; at: number } | null = null
    for (const action of vector.actions) {
      if ('input' in action && action.input) { sequence = action.input.seq; continue }
      if (!('expected' in action) || !action.expected) continue
      const reset = 'reset' in action
      const at = 'advanceMs' in action ? action.advanceMs! : 'atMs' in action ? action.atMs! : 0
      const expected = action.expected
      const motion: SelfMotion = { ...expected, serverTick: Math.floor(at / profile.stepMs) }
      if (reset) { model = createMotionPrediction(context, profile)!; sequence = 0; previous = null }
      // Only infer the next held step when the oracle explicitly kept the same ACK,
      // no ticks were dropped, and no intervening input action changed the sequence.
      if (previous && at - previous.at === profile.stepMs && previous.ack === expected.ackInputSeq
        && sequence === expected.ackInputSeq && expected.droppedTicks === previous.motion.droppedTicks) close(model.sample(at, []), expected.position)
      expect(model.correct(motion, motion.serverTick, expected.ackInputSeq, sequence, at), vector.name).toBe(true)
      close(model.sample(at, []), expected.position)
      previous = { motion, ack: expected.ackInputSeq, at }
    }
  })
  it('keeps only four newest due steps after a long render stall and never infers ACK duration', async () => {
    const model = createMotionPrediction(await qualify(), profile)!
    expect(model.correct(anchor(), 0, 0, 1, 0)).toBe(true)
    const pending = [{ seq: 1, axisX: 1, axisY: 0, sentAt: 0 }]
    close(model.sample(49, pending), { x: 48, y: 48 })
    close(model.sample(50, pending), { x: 56, y: 48 })
    close(model.sample(250, pending), { x: 80, y: 48 }) // five due, four retained; inclusive hold boundary
    close(model.sample(1000, pending), { x: 80, y: 48 }) // repeated sampling never renews the four-step budget or retracts an old window
    const stalled = createMotionPrediction(await qualify(), profile)!
    expect(stalled.correct(anchor(), 0, 0, 1, 0)).toBe(true)
    close(stalled.sample(1000, pending), { x: 48, y: 48 }) // expired input does not acquire a new hold after a stall
    const corrected = anchor({ serverTick: 4, position: { x: 60, y: 48 }, consumedInputSeq: 1, inputAppliedTicks: 4 })
    expect(model.correct(corrected, 4, 1, 1, 1000)).toBe(true)
    close(model.sample(1000, pending), corrected.position)
    close(model.sample(1050, pending), corrected.position) // ACK4 ticks is not four new replay steps
  })
  it('coalesces latest pending axes, preserves neutral stop, and never replays expired consumed inputs', async () => {
    const model = createMotionPrediction(await qualify(), profile)!
    expect(model.correct(anchor(), 0, 0, 3, 0)).toBe(true)
    const pending = [{ seq: 1, axisX: 1, axisY: 0, sentAt: 0 }, { seq: 2, axisX: .5, axisY: 0, sentAt: 20 },
      { seq: 3, axisX: 0, axisY: 0, sentAt: 60 }]
    close(model.sample(50, pending), { x: 52, y: 48 })
    close(model.sample(100, pending), { x: 52, y: 48 })
    expect(model.correct(anchor({ serverTick: 5, consumedInputSeq: 3, droppedTicks: 1 }), 5, 0, 3, 251)).toBe(true)
    close(model.sample(301, pending), { x: 48, y: 48 })
  })
  it('uses only declared remaining held time and withdraws on inconsistent counters/anchors', async () => {
    const context = await qualify(), model = createMotionPrediction(context, profile)!
    const held = anchor({ serverTick: 5, consumedInputSeq: 1, inputAppliedTicks: 4, droppedTicks: 1,
      position: { x: 80, y: 48 }, heldInput: { seq: 1, axisX: 1, axisY: 0, remainingHoldMs: 0 } })
    expect(model.correct(held, 5, 1, 1, 250)).toBe(true)
    close(model.sample(300, []), held.position)
    expect(model.correct({ ...held, consumedInputSeq: 2 }, 5, 1, 1, 251)).toBe(false)
    expect(model.sample(300, [])).toBeNull()
    const fresh = () => createMotionPrediction(context, profile)!
    expect(fresh().correct({ ...held, serverTick: 4 }, 5, 1, 1, 250)).toBe(false)
    expect(fresh().correct({ ...held, position: { x: 0, y: 0 } }, 5, 1, 1, 250)).toBe(false)
    expect(fresh().correct({ ...held, heldInput: { ...held.heldInput!, seq: 2 } }, 5, 1, 1, 250)).toBe(false)
    expect(fresh().correct({ ...held, inputAppliedTicks: 5 }, 5, 1, 1, 250)).toBe(false)
  })
})
