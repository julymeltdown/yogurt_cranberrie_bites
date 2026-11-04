import { expect, it } from 'vitest'
import { testClock } from '../../../sync/__tests__/fixture'
import { drain, toolContext, toolsFixture, time } from './fixture'

it('rejects capability arrival at the 5000ms request-start deadline without moving expiry to arrival', async () => {
  let at = 0, finish!: (value: unknown) => void
  const f = toolsFixture({ now: () => at, port: { load: () => new Promise(resolve => { finish = resolve }) } })
  try {
    at = 5000; finish(toolContext()); await drain()
    expect(f.controller.read().toolContext).toBeNull(); expect(f.controller.canEdit()).toBe(false)
    expect(f.controller.canUseTool('pencil')).toBe(false)
  } finally { f.dispose() }
})
it('withdraws the old editable draft at its own deadline while a newer capability read is still pending', async () => {
  const clock = testClock(); let reads = 0
  const f = toolsFixture({ clock, port: { load: async () => ++reads === 1 ? toolContext() : new Promise(() => {}) } })
  try {
    await f.ready(); f.controller.chooseTool('fill'); f.controller.chooseColor(2); f.controller.select(10, 10)
    expect(f.controller.canConfirm()).toBe(true)
    await clock.advance(2000); expect(reads).toBe(2); expect(f.controller.canConfirm()).toBe(true)
    await clock.advance(3000)
    expect(f.controller.read().toolContext).toBeNull(); expect(f.controller.read().draft).toBeNull()
    expect(f.controller.canEdit()).toBe(false); expect(f.controller.read().snapshot).not.toBeNull()
    await f.controller.confirm(); expect(f.actions).toHaveLength(0)
  } finally { f.dispose(); await drain(); expect(clock.count()).toBe(0) }
})
it('refreshes visible qualified state every 2s without cancelling a same-revision explicit draft', async () => {
  const clock = testClock(); let reads = 0
  const f = toolsFixture({ clock, port: { load: async () => { reads++; return toolContext({ serverTime: new Date(Date.parse(time) + clock.now()).toISOString() }) } } })
  try {
    await f.ready(); f.controller.chooseTool('fill'); f.controller.chooseColor(2); f.controller.select(10, 10)
    const draft = f.controller.read().draft
    await clock.advance(2000); expect(reads).toBe(2)
    expect(f.controller.read().draft).toBe(draft); expect(f.controller.canConfirm()).toBe(true)
    await clock.advance(2000); expect(reads).toBe(3); expect(f.controller.canConfirm()).toBe(true)
    expect(f.actions).toHaveLength(0)
  } finally { f.dispose(); await drain(); expect(clock.count()).toBe(0) }
})
it('withdraws on a read failure and never grants legacy Pencil fallback when a configured capability port fails', async () => {
  const clock = testClock(); let reads = 0
  const f = toolsFixture({ clock, port: { load: async () => { if (++reads > 1) throw new Error('Unavailable'); return toolContext() } } })
  try {
    await f.ready(); expect(f.controller.canUseTool('pencil')).toBe(true)
    await clock.advance(2000)
    expect(f.controller.read().toolContext).toBeNull(); expect(f.controller.canUseTool('pencil')).toBe(false)
    expect(f.controller.canUseTool('erase')).toBe(false); expect(f.controller.canEdit()).toBe(false)
  } finally { f.dispose(); await drain(); expect(clock.count()).toBe(0) }
})
it('synchronously withdraws on visibility loss, stops timers, and preserves the original unknown action for GET recovery', async () => {
  const clock = testClock(), f = toolsFixture({ clock, port: { submit: async () => ({ kind: 'unknown' }) } })
  try {
    await f.ready(); f.controller.chooseTool('erase'); f.controller.select(10, 10); await f.controller.confirm()
    const original = f.controller.read().action
    f.controller.setVisible(false)
    expect(f.controller.read().toolContext).toBeNull(); expect(f.controller.read().action).toBe(original)
    expect(f.controller.canEdit()).toBe(false); expect(clock.count()).toBe(0)
    await clock.advance(10000); expect(f.actions).toHaveLength(1)
    f.controller.setVisible(true); await drain(); expect(f.controller.read().action).toBe(original)
    await f.controller.check(); expect(f.checks[0]).toBe(original); expect(f.actions).toHaveLength(1)
  } finally { f.dispose(); await drain(); expect(clock.count()).toBe(0) }
})
