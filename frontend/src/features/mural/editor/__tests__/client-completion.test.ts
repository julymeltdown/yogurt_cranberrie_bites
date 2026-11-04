import { expect, it, vi } from 'vitest'
import { createPointerInput } from '../input'
import { ack, fixture, snapshot } from './fixture'
import type { CommandResult } from '../contract'

it('quarantines every contact of a multitouch gesture until all contacts end', () => {
  const select = vi.fn(), input = createPointerInput({ select, pan: vi.fn(), cancel: vi.fn(), now: () => 0 })
  const point = { x: 20, y: 20 }
  input.down(1, point); input.down(2, point); input.down(3, point)
  input.up(1, point); input.up(2, point)
  // The third finger remains down: a fourth contact is still ambiguous.
  input.down(4, point); input.up(4, point); input.up(3, point)
  expect(select).not.toHaveBeenCalled()
  input.down(5, point); input.up(5, point)
  expect(select).toHaveBeenCalledExactlyOnceWith(point)
})

it('an older command acknowledgement cannot shorten a newer authoritative cooldown', async () => {
  let resolve!: (value: CommandResult) => void
  const f = fixture({ place: () => new Promise(r => { resolve = r }) }, () => 100)
  try {
    f.controller.select(2, 3)
    const sending = f.controller.confirm()
    f.controller.acceptSnapshot(f.controller.scope, snapshot({ revision: 20,
      next_allowed_at: '2026-11-04T00:00:20Z' }))
    expect(f.controller.remaining()).toBe(20000)
    resolve(ack(f.places[0]!)); await sending
    expect(f.controller.read().snapshot?.revision).toBe(20)
    expect(f.controller.remaining()).toBe(20000)
    f.controller.select(3, 4); await f.controller.confirm()
    expect(f.places).toHaveLength(1)
  } finally { f.dispose() }
})
