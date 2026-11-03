import { afterEach, describe, expect, it, vi } from 'vitest'
import { ack, fixture, ids, snapshot } from './fixture'
import type { CommandResult, MuralSnapshot } from '../contract'
import { placePixelSchema, snapshotSchema } from '../contract'

const cleanup: (() => void)[] = []
const setup = (...args: Parameters<typeof fixture>) => { const f = fixture(...args); cleanup.push(f.dispose); return f }
afterEach(() => { cleanup.splice(0).reverse().forEach(f => f()); vi.useRealTimers() })
const deferred = <T,>() => { let resolve!: (v: T) => void; const promise = new Promise<T>(r => { resolve = r }); return { promise, resolve } }

describe('FE32 authoritative placement', () => {
  it('preview and color never place; only explicit confirm sends a frozen scoped command', async () => {
    const response = deferred<CommandResult>(), f = setup({ place: () => response.promise }), c = f.controller
    c.chooseColor(2); c.select(127, 0)
    expect(f.places).toHaveLength(0); expect(c.read().snapshot!.revision).toBe(10); expect(c.read().snapshot!.pixels[127]).toBe(0)
    const pending = c.confirm(); void c.confirm()
    expect(f.places).toHaveLength(1); expect(c.read().phase).toBe('sending'); expect(c.read().snapshot!.revision).toBe(10)
    expect(f.places[0]!.body).toEqual({ x: 127, y: 0, color_index: 2, expected_revision: 10 })
    expect(Object.isFrozen(f.places[0]!.body)).toBe(true); expect(f.places[0]!.scope.userId).toBe(ids.user)
    response.resolve(ack(f.places[0]!)); await pending
    expect(c.read().snapshot!.revision).toBe(11); expect(c.read().snapshot!.pixels[127]).toBe(2); expect(c.read().selection).toBeNull()
  })
  it.each([-1, 128, 0.5, NaN, Infinity])('rejects coordinate %s without changing the selection', n => {
    const { controller: c, places } = setup(); expect(c.select(n, 0)).toBe(false); expect(c.select(0, n)).toBe(false)
    expect(c.read().selection).toBeNull(); expect(places).toHaveLength(0)
  })
  it.each([-1, 16, 1.5, NaN])('rejects color %s', n => { const { controller: c } = setup(); expect(c.chooseColor(n)).toBe(false); expect(c.read().color).toBe(0) })
  it('uses server-relative cooldown with a monotonic elapsed clock', async () => {
    let clock = 100; const f = setup({}, () => clock), c = f.controller
    c.acceptSnapshot(c.scope, snapshot({ server_time: '2026-11-04T00:00:03Z', next_allowed_at: '2026-11-04T00:00:05Z' }))
    c.select(1, 1); expect(c.remaining()).toBe(2000); await c.confirm(); expect(f.places).toHaveLength(0)
    clock += 2000; await c.confirm(); expect(f.places).toHaveLength(1); expect(c.remaining()).toBe(5000)
  })
  it.each(['PAUSED', 'FROZEN'] as const)('%s is inspectable but cannot select or commit', async state => {
    const f = setup(), c = f.controller; c.select(1, 1); c.acceptSnapshot(c.scope, snapshot({ state }))
    expect(c.read().selection).toBeNull(); expect(c.select(2, 2)).toBe(false); await c.confirm(); expect(f.places).toHaveLength(0)
  })
  it.each(['loading', 'read-only', 'error'] as const)('availability %s fails closed', async availability => {
    const f = setup(), c = f.controller; c.select(1, 1); c.setAvailability(c.scope, availability)
    expect(c.read().selection).toBeNull(); expect(c.chooseColor(3)).toBe(false); await c.confirm(); expect(f.places).toHaveLength(0)
  })
  it('fresh snapshots cannot reopen an independently read-only event', () => {
    const { controller: c } = setup(); c.setAvailability(c.scope, 'read-only')
    c.acceptSnapshot(c.scope, snapshot({ revision: 20 })); expect(c.canEdit()).toBe(false); expect(c.read().availability).toBe('read-only')
    c.setAvailability(c.scope, 'error'); c.acceptSnapshot(c.scope, snapshot({ revision: 21 })); expect(c.canEdit()).toBe(false)
    c.setAvailability(c.scope, 'writable'); expect(c.canEdit()).toBe(true)
  })
  it.each(['MURAL_COOLDOWN', 'MURAL_PAUSED', 'MURAL_FROZEN', 'MURAL_REGION_LOCKED', 'VERSION_CONFLICT', 'PERMISSION_DENIED', 'INVALID_INPUT'])('terminal %s rolls back preview without painting', async reason => {
    const f = setup({ place: async () => ({ kind: 'rejected', reason }) }), c = f.controller
    c.select(1, 1); await c.confirm(); expect(c.read().phase).toBe('rejected'); expect(c.read().snapshot!.revision).toBe(10)
    expect(c.read().selection).toBeNull(); expect(c.read().command).toBeNull()
    if (reason !== 'INVALID_INPUT') expect(c.canEdit()).toBe(false)
  })
  it.each(['throw', 'pending', 'unknown'] as const)('%s retains one command and checks without replay', async kind => {
    const f = setup({ place: async () => { if (kind === 'throw') throw new Error('network'); return { kind } } }), c = f.controller
    c.select(1, 2); await c.confirm(); await c.confirm(); expect(f.places).toHaveLength(1)
    const command = c.read().command; expect(command).not.toBeNull(); expect(c.canEdit()).toBe(false)
    await c.check(); expect(f.checks).toEqual([command]); expect(f.places).toHaveLength(1); expect(c.read().phase).toBe('confirmed')
  })
  it.each(['IDEMPOTENCY_EXPIRED', 'IDEMPOTENCY_CONFLICT', 'INTERNAL_ERROR', 'DEADLINE_EXCEEDED', 'unknown-code'])('%s is not proof the original write failed', async reason => {
    const f = setup({ place: async () => ({ kind: 'rejected', reason }) }), c = f.controller
    c.select(1, 2); await c.confirm(); expect(c.read().phase).toBe('unknown'); expect(c.read().command).not.toBeNull()
    c.acceptSnapshot(c.scope, snapshot()); await c.confirm(); expect(f.places).toHaveLength(1)
  })
  it('retains uncertainty across failed checks and refresh/reset; never auto-replays', async () => {
    const f = setup({ place: async () => ({ kind: 'unknown' }), check: async () => { throw new Error('offline') } }), c = f.controller
    c.select(1, 2); await c.confirm(); const command = c.read().command
    c.acceptSnapshot(c.scope, snapshot({ revision: 50 }), { reset: true }); await c.check()
    expect(c.read().command).toBe(command); expect(c.read().snapshot!.revision).toBe(50); expect(c.read().selection).toBeNull()
    expect(f.places).toHaveLength(1); expect(f.checks).toHaveLength(1)
  })
  it('times out an uncooperative port, keeps the original command, and permits read-only recovery', async () => {
    vi.useFakeTimers()
    const f = setup({ place: () => new Promise(() => {}) }, () => 0, 100), c = f.controller
    c.select(1, 1); const pending = c.confirm(); await vi.advanceTimersByTimeAsync(100); await pending
    expect(f.signals[0]!.aborted).toBe(true); expect(c.read().phase).toBe('unknown'); expect(c.read().command).not.toBeNull()
    expect(vi.getTimerCount()).toBe(0); await c.check(); expect(c.read().phase).toBe('confirmed'); expect(f.places).toHaveLength(1)
  })
  it('privacy cleanup settles ignored aborts immediately and removes timeout/listeners even when a subscriber throws', async () => {
    vi.useFakeTimers()
    const f = setup({ place: () => new Promise(() => {}) }), c = f.controller, observed: unknown[] = []
    c.subscribe(() => { throw new Error('consumer') }); c.subscribe(() => observed.push(c.read().snapshot))
    c.select(1, 1); const pending = c.confirm(); f.session.replace(f.session.read().generation, null); await pending
    expect(observed.at(-1)).toBeNull(); expect(vi.getTimerCount()).toBe(0); expect(c.read().snapshot).toBeNull()
  })
  it.each([{ event_id: ids.user }, { revision: 12 }, { x: 4 }, { color_index: 5 }, { next_allowed_at: 'invalid' }, { author_id: ids.user }])('unverified ack %j stays unknown', async patch => {
    const f = setup({ place: async c => ack(c, patch) }), c = f.controller; c.select(1, 2); await c.confirm()
    expect(c.read().phase).toBe('unknown'); expect(c.read().snapshot!.revision).toBe(10); expect(c.read().command).not.toBeNull()
  })
  it('a delayed acknowledgement cannot overwrite a newer synchronized revision', async () => {
    const pending = deferred<CommandResult>(), f = setup({ place: () => pending.promise }), c = f.controller
    c.select(1, 2); const run = c.confirm(); c.acceptSnapshot(c.scope, snapshot({ revision: 20 }), { reset: true })
    pending.resolve(ack(f.places[0]!)); await run
    expect(c.read().snapshot!.revision).toBe(20); expect(c.read().snapshot!.pixels[257]).toBe(0)
  })
  it.each(['logout', 'user', 'event', 'policy', 'dispose'] as const)('%s scrubs drafts, aborts work, rejects late results and stale snapshots', async change => {
    const pending = deferred<CommandResult>(), f = setup({ place: () => pending.promise }), c = f.controller
    c.select(1, 2); const run = c.confirm()
    if (change === 'policy') f.privacy.changePolicy(c.scope, 'block')
    else if (change === 'dispose') c.dispose()
    else f.session.replace(f.session.read().generation, change === 'logout' ? null : { userId: change === 'user' ? ids.operation : ids.user,
      eventId: change === 'event' ? ids.operation : ids.event, accessToken: 'new-memory-token' })
    expect(f.signals[0]!.aborted).toBe(true); expect(c.read().snapshot).toBeNull(); expect(c.read().command).toBeNull(); expect(c.read().selection).toBeNull()
    pending.resolve(ack(f.places[0]!)); await run
    expect(c.read().snapshot).toBeNull(); expect(c.acceptSnapshot(c.scope, snapshot())).toBe(false)
    expect(c.select(2, 2)).toBe(false); await c.check(); expect(f.checks).toHaveLength(0)
  })
  it('checks exact snapshot scope, shape, event, bounds and monotonic revision; copies the arrays', () => {
    const { controller: c } = setup(), original = snapshot()
    c.acceptSnapshot(c.scope, original); original.pixels[0] = 15; expect(c.read().snapshot!.pixels[0]).toBe(0)
    expect(c.acceptSnapshot({ ...c.scope, policyGeneration: 900 }, snapshot())).toBe(false)
    expect(c.acceptSnapshot(c.scope, snapshot({ revision: 9 }))).toBe(false)
    expect(c.acceptSnapshot(c.scope, { ...snapshot(), pixels: [0] })).toBe(false); expect(c.read().availability).toBe('error')
    expect(c.acceptSnapshot(c.scope, snapshot({ event_id: ids.user }))).toBe(false)
    expect(snapshotSchema.safeParse(snapshot({ palette: ['#000000'] } as Partial<MuralSnapshot>)).success).toBe(false)
    expect(placePixelSchema.safeParse({ x: 128, y: 0, color_index: 16, expected_revision: 0 }).success).toBe(false)
  })
})
