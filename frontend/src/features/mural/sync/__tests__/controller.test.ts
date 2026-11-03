import { afterEach, describe, expect, it } from 'vitest'
import { delta, drain, fixture, ids, pixel, snapshot, time } from './fixture'
import type { PixelCommand } from '../../editor/contract'
const cleanups: (() => void)[] = []
const setup = (...args: Parameters<typeof fixture>) => { const f = fixture(...args); cleanups.push(f.dispose); return f }
afterEach(async () => { cleanups.splice(0).reverse().forEach(f => f()); await drain() })
function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(r => { resolve = r }); return { promise, resolve } }
const own = (c: PixelCommand) => { const { expected_revision, ...selection } = c.body; return { kind: 'acknowledged' as const, pixel: { ...pixel(expected_revision + 1, selection), next_allowed_at: time } } }

describe('FE33 editor/reconciliation/resource ownership', () => {
  it('new and reconnect consumers converge despite reordered/duplicate packets without another connection', async () => {
    const a = setup(), b = setup({ actor: ids.other }); await a.start(); await b.start()
    a.sync.receiveDelta(a.sync.lease()!, delta(10, [pixel(11), pixel(12)]))
    b.sync.receiveDelta(b.sync.lease()!, delta(11, [pixel(12)])); expect(b.sync.read().phase).toBe('gap'); expect(b.sync.editor.canEdit()).toBe(false)
    b.sync.receiveDelta(b.sync.lease()!, delta(10, [pixel(11)])); b.sync.receiveDelta(b.sync.lease()!, delta(10, [pixel(11), pixel(12)]))
    expect(a.sync.editor.read().snapshot!.pixels).toEqual(b.sync.editor.read().snapshot!.pixels); expect(b.sync.read().revision).toBe(12)
    const latest = a.sync.editor.read().snapshot!; b.server(latest); b.authority.set(null); expect(b.sync.editor.read().snapshot).toBeNull()
    b.authority.set({ binding: { eventId: ids.event, roomId: ids.room, roomEpoch: ids.next, streamId: ids.next }, writable: true }); await drain()
    expect(b.sync.editor.read().snapshot!.pixels).toEqual(latest.pixels); expect(b.sync.read().revision).toBe(12)
  })
  it('gap expires into exactly one fresh snapshot, with bounded reads and no mutation', async () => {
    const f = setup(); await f.start(); f.server(snapshot({ revision: 12 }))
    f.sync.receiveDelta(f.sync.lease()!, delta(11, [pixel(12)])); expect(f.sync.read().queued).toBe(1)
    await f.clock.advance(200); expect(f.requests.filter(r => r.kind === 'snapshot')).toHaveLength(2)
    expect(f.sync.read().phase).toBe('ready'); expect(f.sync.read().revision).toBe(12); expect(f.sync.read().queued).toBe(0)
    expect(f.requests.some(r => r.kind === 'place')).toBe(false)
  })
  it('old epoch/stream/room leases and late snapshots cannot resurrect withdrawn pixels', async () => {
    const pending = deferred<unknown>(); let first = true
    const f = setup({ reads: { snapshot: () => first ? (first = false, pending.promise) : Promise.resolve(snapshot({ revision: 20 })), delta: async () => delta(20, []) } })
    f.sync.start(); const old = f.sync.lease()!; f.authority.set({ binding: { ...old.binding, roomId: ids.next, roomEpoch: ids.next, streamId: ids.next }, writable: true }); await drain()
    expect(f.sync.read().revision).toBe(20); expect(f.sync.receiveDelta(old, delta(20, [pixel(21)]))).toBe(false)
    pending.resolve(snapshot()); await drain(); expect(f.sync.read().revision).toBe(20); expect(f.requests[0]!.signal.aborted).toBe(true)
  })
  it('restore barrier resets preview and old delivery generation but preserves an unknown command', async () => {
    const f = setup(); await f.start(); const c = f.sync.editor
    c.select(2, 2); await c.confirm(); const command = c.read().command; expect(command).not.toBeNull()
    const old = f.sync.lease()!; f.server(snapshot({ revision: 50, state: 'PAUSED', snapshot_id: ids.next }))
    f.sync.receiveDelta(old, delta(10, [], true, 50)); await drain()
    expect(c.read().selection).toBeNull(); expect(c.read().command).toBe(command); expect(c.read().snapshot!.revision).toBe(50); expect(c.canEdit()).toBe(false)
    expect(f.sync.receiveDelta(old, delta(50, [pixel(51)]))).toBe(false); expect(f.sync.read().revision).toBe(50)
  })
  it('an older overlapping snapshot cannot overwrite updates received during refresh', async () => {
    let call = 0; const pending = deferred<unknown>(), f = setup({ reads: { snapshot: () => ++call === 1 ? Promise.resolve(snapshot()) : pending.promise, delta: async () => delta(11, []) } })
    await f.start(); const refresh = f.sync.refresh(), current = f.sync.lease()!
    f.sync.receiveDelta(current, delta(10, [pixel(11)])); pending.resolve(snapshot()); await refresh
    expect(f.sync.read().revision).toBe(11); expect(f.sync.editor.read().snapshot!.pixels[139]).toBe(11)
  })
  it('own acknowledged placement updates the world sink only from server data, then public echo deduplicates', async () => {
    const f = setup({ commands: { place: async c => own(c), check: async () => ({ kind: 'unknown' }) } }); await f.start()
    f.sync.editor.chooseColor(2); f.sync.editor.select(4, 4); expect(f.scene.read().uploads).toBe(1)
    await f.sync.editor.confirm(); f.scene.frame(); expect(f.sync.read().revision).toBe(11); expect(f.sync.editor.read().phase).toBe('confirmed')
    expect(f.scene.read().rgba![4 * (4 * 128 + 4)]).toBe(162)
    f.sync.receiveDelta(f.sync.lease()!, delta(10, [pixel(11, { x: 4, y: 4, color_index: 2 })])); expect(f.sync.read().revision).toBe(11)
  })
  it('rejected preview rolls back while unknown status/reloads never replay a mutation', async () => {
    let rejected = true
    const f = setup({ commands: { place: async () => rejected ? { kind: 'rejected', reason: 'INVALID_INPUT' } : { kind: 'unknown' }, check: async () => ({ kind: 'pending' }) } }); await f.start()
    const c = f.sync.editor; c.select(1, 1); await c.confirm(); expect(c.read().selection).toBeNull(); expect(c.read().snapshot!.revision).toBe(10)
    rejected = false; c.select(2, 2); await c.confirm(); const original = c.read().command; await f.sync.refresh(); await c.check(); await c.confirm()
    expect(c.read().command).toBe(original); expect(c.read().phase).toBe('pending'); expect(f.requests.filter(r => r.kind === 'place')).toHaveLength(2)
  })
  it.each(['PAUSED', 'FROZEN'] as const)('%s reset remains read-only and clears local preview', async state => {
    const f = setup(); await f.start(); f.sync.editor.select(3, 3); f.server(snapshot({ revision: 20, state })); f.sync.receiveDelta(f.sync.lease()!, delta(10, [], true, 20)); await drain()
    expect(f.sync.editor.read().selection).toBeNull(); expect(f.sync.editor.canEdit()).toBe(false); expect(f.sync.editor.read().snapshot!.state).toBe(state)
  })
  it('external read-only authority survives polling/snapshot refresh', async () => {
    const f = setup(); await f.start(); f.authority.set({ ...f.authority.read()!, writable: false }); await f.sync.refresh()
    expect(f.sync.editor.canEdit()).toBe(false); expect(f.sync.editor.read().availability).toBe('read-only')
  })
  it.each(['logout', 'identity', 'event', 'policy', 'dispose'] as const)('%s cleans timers/listeners/textures/commands and fences late work', async change => {
    const pending = deferred<unknown>(); let calls = 0
    const f = setup({ reads: { snapshot: () => ++calls === 1 ? Promise.resolve(snapshot()) : pending.promise, delta: async () => delta(10, []) } }); await f.start()
    f.sync.editor.select(3, 3); await f.sync.editor.confirm(); const refresh = f.sync.refresh()
    if (change === 'policy') f.privacy.changePolicy(f.sync.scope, 'block')
    else if (change === 'dispose') f.sync.dispose()
    else f.session.replace(f.session.read().generation, change === 'logout' ? null : { userId: change === 'identity' ? ids.other : ids.user, eventId: change === 'event' ? ids.next : ids.event, accessToken: 'next-token' })
    await refresh; pending.resolve(snapshot({ revision: 100 })); await drain()
    expect(f.sync.read().phase).toBe('disposed'); expect(f.sync.editor.read().command).toBeNull(); expect(f.sync.editor.read().snapshot).toBeNull()
    expect(f.authority.count()).toBe(0); expect(f.clock.count()).toBe(0); expect(f.scene.read()).toMatchObject({ textures: 0, objects: 0, listeners: 0 })
  })
  it('no authority fails closed without requests or world resources', async () => {
    const f = setup(); f.authority.set(null); await f.start(); expect(f.requests).toHaveLength(0); expect(f.sync.read().phase).toBe('unavailable')
    expect(f.sync.editor.canEdit()).toBe(false); expect(f.scene.read().textures).toBe(0)
  })
  it('a hanging read reaches a bounded error with no retry loop', async () => {
    const f = setup({ reads: { snapshot: () => new Promise(() => {}), delta: async () => null } }); f.sync.start(); await f.clock.advance(8000)
    expect(f.requests[0]!.signal.aborted).toBe(true); expect(f.sync.read()).toMatchObject({ phase: 'error', problem: 'timeout' })
    await f.clock.advance(60000); expect(f.requests).toHaveLength(1); expect(f.clock.count()).toBe(0)
  })
  it('slow world sink coalesces to one pending frame and clears it before privacy disposal', async () => {
    const f = setup(); await f.start()
    for (let revision = 11; revision < 100; revision++) f.sync.receiveDelta(f.sync.lease()!, delta(revision - 1, [pixel(revision)]))
    expect(f.sink.read().queued).toBe(1); expect(f.scene.read().uploads).toBe(1); f.scene.frame()
    expect(f.sink.read().revision).toBe(99); expect(f.scene.read().uploads).toBe(2)
  })
})
