import { describe, expect, it, vi } from 'vitest'
import { createPreWorldPolicy } from '../controller'
import type { PolicyAttempt, PolicyClock, PreWorldDecision, PreWorldPolicyPort } from '../model'

const binding: PolicyAttempt = {
  scope: { userId: '11111111-1111-4111-8111-111111111111', eventId: '22222222-2222-4222-8222-222222222222',
    generation: 3, policyGeneration: 7, gate: 'open' },
  release: { version: 1, applicationVersion: '0.1.0', buildId: 'a'.repeat(64), sourceRevision: 'b'.repeat(40) },
  attempt: 0,
}
const normal: PreWorldDecision = { kind: 'normal', eventRevision: 2, policyRevision: 3,
  minimumClientVersion: '0.1.0', serverTimeMs: 1_000_000, validUntilMs: 1_005_000 }
function setup(read: PreWorldPolicyPort['read'] = async () => normal) {
  let now = 100, current = true, next = 0
  const timers = new Map<number, { at: number; callback: () => void }>()
  const clock: PolicyClock = { now: () => now,
    set(callback, ms) { const id = ++next; timers.set(id, { at: now + ms, callback }); return id },
    clear(id) { timers.delete(id as number) } }
  const port = { release: () => binding.release, read: vi.fn(read) }
  const controller = createPreWorldPolicy({ binding, port, isCurrent: () => current, clock })
  return { controller, port, timers,
    expireScope: () => { current = false },
    elapse(ms: number, flush = true) {
      now += ms
      if (flush) for (const [id, timer] of timers) if (timer.at <= now) { timers.delete(id); timer.callback() }
    } }
}
function deferred<T>() {
  let resolve!: (value: T) => void, reject!: (error: unknown) => void
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no })
  return { promise, resolve, reject }
}

describe('trusted pre-World attempt lifetime', () => {
  it('coalesces a read, charges latency from request start and checks freshness before consumption', async () => {
    const response = deferred<PreWorldDecision>()
    const f = setup(() => response.promise), signal = new AbortController().signal
    const first = f.controller.prepare(signal)
    expect(f.controller.prepare(signal)).toBe(first)
    await Promise.resolve()
    f.elapse(4_000)
    response.resolve(normal)
    expect(await first).toBe(true)
    expect(f.port.read).toHaveBeenCalledTimes(1)
    expect(f.port.read.mock.calls[0]?.[0]).toEqual(binding)
    f.elapse(1_000, false) // blocked event loop cannot extend the grant
    expect(f.controller.isFresh()).toBe(false)
    expect(f.controller.consume()).toBe(false)
    expect(f.controller.read()).toEqual({ kind: 'unavailable', reason: 'expired' })
  })

  it('rejects an already-expired response and ignores late completion after timeout', async () => {
    const response = deferred<PreWorldDecision>(), f = setup(() => response.promise)
    const pending = f.controller.prepare(new AbortController().signal)
    await Promise.resolve(); f.elapse(5_001, false); response.resolve(normal)
    expect(await pending).toBe(false)
    expect(f.controller.read().kind).toBe('unavailable')
    const late = deferred<PreWorldDecision>(), g = setup(() => late.promise)
    const timed = g.controller.prepare(new AbortController().signal)
    await Promise.resolve(); g.elapse(8_000)
    expect(await timed).toBe(false)
    late.resolve(normal); await Promise.resolve(); await Promise.resolve()
    expect(g.controller.read().kind).toBe('unavailable')
    expect(g.timers.size).toBe(0)
  })

  it('withdraws trusted maintenance and minimum-client notices at expiry', async () => {
    for (const kind of ['maintenance', 'version'] as const) {
      const f = setup(async () => ({ ...normal, kind }))
      expect(await f.controller.prepare(new AbortController().signal)).toBe(false)
      expect(f.controller.read().kind).toBe(kind)
      f.elapse(5_000)
      expect(f.controller.read()).toEqual({ kind: 'unavailable', reason: 'expired' })
    }
  })

  it('withdraws on foreground even when a background timer has not run, without fetching', async () => {
    const f = setup(async () => ({ ...normal, kind: 'maintenance' }))
    await f.controller.prepare(new AbortController().signal)
    f.elapse(5_001, false)
    expect(f.controller.read()).toEqual({ kind: 'unavailable', reason: 'expired' })
    f.controller.check()
    expect(f.timers.size).toBe(0)
    expect(f.controller.read()).toEqual({ kind: 'unavailable', reason: 'expired' })
    expect(f.port.read).toHaveBeenCalledTimes(1)
  })

  it('consumes once and detaches preflight timers without withdrawing an admitted World', async () => {
    const f = setup(), signal = new AbortController()
    await f.controller.prepare(signal.signal)
    expect(f.controller.consume()).toBe(true)
    expect(f.timers.size).toBe(0)
    f.elapse(50_000); signal.abort()
    expect(f.controller.read().kind).toBe('consumed')
    expect(f.controller.consume()).toBe(false)
    expect(f.controller.read().kind).toBe('consumed')
    expect(f.port.read).toHaveBeenCalledTimes(1)
  })

  it('cancels an ignored-abort producer and prevents cross-attempt/auth/privacy reuse', async () => {
    const response = deferred<PreWorldDecision>(), f = setup(() => response.promise), signal = new AbortController()
    const pending = f.controller.prepare(signal.signal)
    await Promise.resolve(); f.expireScope(); signal.abort()
    expect(await pending).toBe(false)
    response.resolve(normal); await Promise.resolve(); await Promise.resolve()
    expect(f.controller.consume()).toBe(false)
    expect(f.controller.read().kind).toBe('unavailable')
    expect(f.port.read.mock.calls[0]?.[1].aborted).toBe(true)
  })

  it('never derives maintenance or version authority from transport errors', async () => {
    for (const status of [503, 4409, 'GAP']) {
      const error = Object.assign(new Error('transport unavailable'), { status })
      const f = setup(async () => { throw error })
      await expect(f.controller.prepare(new AbortController().signal)).rejects.toBe(error)
      expect(f.controller.read()).toEqual({ kind: 'unavailable', reason: 'unavailable' })
      expect(f.timers.size).toBe(0)
    }
  })

  it('fails closed without a real port or captured release and never dispatches after disposal', async () => {
    for (const absent of ['port', 'binding'] as const) {
      const f = setup()
      const controller = createPreWorldPolicy({ binding: absent === 'binding' ? null : binding,
        port: absent === 'port' ? undefined : f.port, isCurrent: () => true })
      expect(await controller.prepare(new AbortController().signal)).toBe(false)
      expect(f.port.read).not.toHaveBeenCalled()
    }
    const f = setup(); f.controller.dispose(); f.controller.dispose()
    expect(await f.controller.prepare(new AbortController().signal)).toBe(false)
    expect(f.port.read).not.toHaveBeenCalled()
  })

  it('rejects malformed projections and never treats HTTP success as NORMAL', async () => {
    for (const patch of [
      { eventRevision: 0 }, { policyRevision: null }, { minimumClientVersion: '01.0.0' }, { minimumClientVersion: '0.1.0\n' },
      { minimumClientVersion: '65536.0.0' }, { validUntilMs: normal.serverTimeMs + 30_001 },
      { validUntilMs: Number.NaN }, { kind: 'unexpected' },
    ]) {
      const f = setup(async () => ({ ...normal, ...patch }) as PreWorldDecision)
      expect(await f.controller.prepare(new AbortController().signal)).toBe(false)
      expect(f.controller.isFresh()).toBe(false)
    }
    const f = setup(async () => ({ ...normal, kind: 'unavailable', policyRevision: null }))
    expect(await f.controller.prepare(new AbortController().signal)).toBe(false)
    expect(f.controller.read().kind).toBe('unavailable')
  })
})
