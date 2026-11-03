import { describe, expect, it } from 'vitest'
import { createMuralProjection } from '../projection'
import { delta, ids, pixel, snapshot } from './fixture'
describe('FE33 bounded authoritative projection', () => {
  it('two consumers converge with ordered, reordered, overlapping and duplicate batches', () => {
    const a = createMuralProjection(ids.event), b = createMuralProjection(ids.event), eleven = pixel(11), twelve = pixel(12)
    a.seed(snapshot()); b.seed(snapshot())
    expect(a.delta(delta(10, [eleven, twelve]))).toBe('applied')
    expect(b.delta(delta(11, [twelve]))).toBe('gap'); expect(b.read()!.revision).toBe(10)
    expect(b.delta(delta(10, [eleven]))).toBe('applied'); expect(b.delta(delta(10, [eleven, twelve]))).toBe('duplicate')
    expect(a.read()).toEqual(b.read()); expect(b.queued()).toBe(0)
  })
  it('buffers updates received before snapshot baseline, then drains the contiguous suffix', () => {
    const p = createMuralProjection(ids.event); p.delta(delta(10, [pixel(11), pixel(12)])); expect(p.read()).toBeNull()
    expect(p.seed(snapshot())).toBe('accepted'); expect(p.read()!.revision).toBe(12)
  })
  it('stale overlapping snapshot cannot erase applied updates or pending successors', () => {
    const p = createMuralProjection(ids.event); p.seed(snapshot()); p.delta(delta(10, [pixel(11)])); p.delta(delta(12, [pixel(13)]))
    expect(p.seed(snapshot())).toBe('stale'); expect(p.read()!.revision).toBe(11); expect(p.queued()).toBe(1)
    p.delta(delta(11, [pixel(12)])); expect(p.read()!.revision).toBe(13)
  })
  it('restore/retention barriers discard old data and prohibit a snapshot rewind', () => {
    const p = createMuralProjection(ids.event); p.seed(snapshot()); p.delta(delta(11, [pixel(12)]))
    expect(p.delta(delta(10, [], true, 50))).toBe('reset'); expect(p.read()).toBeNull(); expect(p.queued()).toBe(0)
    expect(p.seed(snapshot({ revision: 49 }))).toBe('stale'); expect(p.seed(snapshot({ revision: 50 }))).toBe('accepted')
    expect(p.delta(delta(10, [pixel(11), pixel(12)]))).toBe('duplicate'); expect(p.read()!.revision).toBe(50)
  })
  it.each(['hole', 'wrong-event', 'private-field', 'reverse', 'reset-with-ops', 'duplicate-id'] as const)('rejects %s', kind => {
    const p = createMuralProjection(ids.event); p.seed(snapshot())
    const d: Record<string, unknown> = delta(10, [pixel(11)]) as unknown as Record<string, unknown>
    if (kind === 'hole') d.to_revision = 12
    if (kind === 'wrong-event') d.event_id = ids.user
    if (kind === 'private-field') d.operations = [{ ...pixel(11), author_id: ids.user }]
    if (kind === 'reverse') d.operations = [pixel(12), pixel(11)]
    if (kind === 'reset-with-ops') d.reset_required = true
    if (kind === 'duplicate-id') { d.operations = [pixel(11), pixel(12, { operation_id: pixel(11).operation_id })]; d.to_revision = 12 }
    expect(p.delta(d)).toBe('invalid'); expect(p.read()).toBeNull()
  })
  it('conflicting duplicate revision and reused operation ID invalidate authority', () => {
    for (const incoming of [delta(10, [pixel(11, { color_index: 2 })]), delta(11, [pixel(12, { operation_id: pixel(11).operation_id })])]) {
      const p = createMuralProjection(ids.event); p.seed(snapshot()); p.delta(delta(10, [pixel(11)])); expect(p.delta(incoming)).toBe('invalid')
    }
  })
  it('bounds queued slow-consumer work and rejects oversized batches before traversal', () => {
    const p = createMuralProjection(ids.event, 2); p.seed(snapshot()); p.delta(delta(11, [pixel(12), pixel(13)]))
    expect(p.queued()).toBe(2); expect(p.delta(delta(13, [pixel(14)]))).toBe('overflow'); expect(p.queued()).toBe(0)
    expect(p.delta(delta(10, Array.from({ length: 513 }, (_, i) => pixel(i + 11))))).toBe('overflow')
  })
  it('frozen wall never reopens and pixel deltas cannot bypass pause', () => {
    const p = createMuralProjection(ids.event); p.seed(snapshot({ state: 'FROZEN' })); p.invalidate()
    expect(p.seed(snapshot({ state: 'EDITABLE', revision: 11 }))).toBe('invalid')
    const paused = createMuralProjection(ids.event); paused.seed(snapshot({ state: 'PAUSED' })); paused.delta(delta(10, [pixel(11)])); expect(paused.read()).toBeNull()
  })
  it('same revision cannot change confirmed pixel/state; source arrays are copied and frozen', () => {
    const p = createMuralProjection(ids.event), source = snapshot(); p.seed(source); source.pixels[0] = 9
    expect(p.read()!.pixels[0]).toBe(0); expect(Object.isFrozen(p.read()!.pixels)).toBe(true)
    expect(p.seed(source)).toBe('invalid'); expect(p.seed(snapshot({ state: 'PAUSED' }))).toBe('invalid')
  })
})
