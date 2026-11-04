import { afterEach, expect, it, vi } from 'vitest'
import { createEnginePreferenceEffects } from '../engine'
import { createRuntimeEffects } from '../runtime'
import type { RuntimeEffectOwner } from '../runtime'
import type { RuntimeEffectLease, RuntimeEffects } from '../contract'
const reduced = { reducedMotion: true, lowPower: true }, normal = { reducedMotion: false, lowPower: false }
function engine() {
  let time = 0
  const renderer = { preRender: vi.fn(), render: vi.fn(), postRender: vi.fn() }
  const original = { ...renderer }, tweens = { timeScale: .75, setGlobalTimeScale(value: number) { this.timeScale = value; return this } }
  const effects = createEnginePreferenceEffects(renderer, tweens, () => time)
  return { effects, renderer, original, tweens, tick(value: number) { time = value; renderer.preRender(); renderer.render(); renderer.postRender() } }
}
afterEach(() => vi.useRealTimers())
it('leaves default engine methods and tween scale untouched without a lease', () => {
  const f = engine(); f.tick(0)
  expect(f.renderer).toEqual(f.original); expect(f.tweens.timeScale).toBe(.75)
  const lease = f.effects.acquire(normal, new AbortController().signal) as RuntimeEffectLease
  expect(f.renderer).toEqual(f.original); expect(f.tweens.timeScale).toBe(.75); lease.dispose()
})
it('gates all three actual paint phases together and restores exact original methods synchronously', () => {
  const f = engine(), caller = new AbortController()
  f.effects.acquire(reduced, caller.signal)
  for (const time of [0, 16, 32, 48, 64, 80]) f.tick(time)
  for (const fn of Object.values(f.original)) expect(fn).toHaveBeenCalledTimes(2)
  expect(f.effects.read()).toMatchObject({ lowPower: true, reducedMotion: true, attempts: 6, paints: 2, paintRateLimit: 15 })
  expect(f.tweens.timeScale).toBe(0)
  caller.abort()
  expect(f.renderer).toEqual(f.original); expect(f.tweens.timeScale).toBe(.75)
  f.tick(81); for (const fn of Object.values(f.original)) expect(fn).toHaveBeenCalledTimes(3)
})
it('supersedes its lease and ignores obsolete disposal or abort', () => {
  const f = engine(), oldSignal = new AbortController()
  const old = f.effects.acquire(reduced, oldSignal.signal) as RuntimeEffectLease
  const next = f.effects.acquire({ reducedMotion: false, lowPower: true }, new AbortController().signal)
  old.dispose(); oldSignal.abort()
  expect(f.tweens.timeScale).toBe(.75); expect(f.effects.read().lowPower).toBe(true)
  next.dispose(); expect(f.renderer).toEqual(f.original)
})
it('retains the paint deadline across low-power reacquisition for an OS motion change', () => {
  const f = engine(), caller = new AbortController()
  const old = f.effects.acquire(reduced, caller.signal); f.tick(0); old.dispose()
  f.effects.acquire({ reducedMotion: false, lowPower: true }, new AbortController().signal)
  f.tick(10); f.tick(67)
  for (const fn of Object.values(f.original)) expect(fn).toHaveBeenCalledTimes(2)
})
it('does not overwrite a newer external paint owner when releasing its wrappers', () => {
  const f = engine(), lease = f.effects.acquire(reduced, new AbortController().signal)
  const newer = vi.fn(); f.renderer.render = newer
  lease.dispose(); expect(f.renderer.render).toBe(newer); expect(f.renderer.preRender).toBe(f.original.preRender)
})
it('engine disposal resets effects, is idempotent and rejects later or preaborted acquisitions', () => {
  const f = engine(); f.effects.acquire(reduced, new AbortController().signal); f.effects.dispose(); f.effects.dispose()
  expect(f.renderer).toEqual(f.original); expect(f.tweens.timeScale).toBe(.75)
  expect(() => f.effects.acquire(reduced, new AbortController().signal)).toThrow()
  const cancelled = new AbortController(); cancelled.abort()
  expect(() => engine().effects.acquire(reduced, cancelled.signal)).toThrow()
})
it('preserves reentrant successors during incumbent release', () => {
  const f = engine(), first = new AbortController()
  f.effects.acquire(reduced, first.signal)
  first.signal.addEventListener('abort', () => { f.effects.acquire({ ...normal, lowPower: true }, new AbortController().signal) })
  first.abort(); expect(f.effects.read()).toMatchObject({ lowPower: true, reducedMotion: false }); expect(f.tweens.timeScale).toBe(.75)
})
function borrowed(implementation?: RuntimeEffects['acquire']) {
  const lifetime = new AbortController(), dispose = vi.fn(), aborted = vi.fn()
  const source: RuntimeEffects = { motion: true, lowPower: true, acquire: implementation ?? ((_value, signal) => { signal.addEventListener('abort', aborted); return { dispose } }) }
  let owner: RuntimeEffectOwner | null = { signal: lifetime.signal, effects: source }
  const effects = createRuntimeEffects(() => owner, 100)
  return { effects, lifetime, dispose, aborted, replace(next: RuntimeEffectOwner | null) { owner = next } }
}
it('runtime availability follows the ready incumbent and rejects unsupported requests before mutation', () => {
  const f = borrowed(); expect(f.effects.motion).toBe(true); f.replace(null)
  expect(f.effects.motion).toBe(false); expect(() => f.effects.acquire(reduced, new AbortController().signal)).toThrow()
  const source: RuntimeEffects = { motion: false, lowPower: false, acquire: vi.fn() }
  f.replace({ signal: new AbortController().signal, effects: source })
  expect(() => f.effects.acquire(reduced, new AbortController().signal)).toThrow(); expect(source.acquire).not.toHaveBeenCalled()
})
it.each(['caller', 'runtime'] as const)('synchronously aborts acquired engine effects on %s lifetime withdrawal', async which => {
  const f = borrowed(), caller = new AbortController(), lease = await f.effects.acquire(reduced, caller.signal)
  ;(which === 'caller' ? caller : f.lifetime).abort()
  expect(f.aborted).toHaveBeenCalledOnce(); expect(f.dispose).toHaveBeenCalledOnce(); lease.dispose(); expect(f.dispose).toHaveBeenCalledOnce()
})
it('fences an ignored-abort async acquisition and releases its eventual lease', async () => {
  let resolve!: (value: RuntimeEffectLease) => void
  const dispose = vi.fn(), caller = new AbortController(), f = borrowed(() => new Promise(r => { resolve = r }))
  const task = f.effects.acquire(reduced, caller.signal); const rejected = expect(task).rejects.toMatchObject({ reason: 'cancelled' })
  caller.abort(); await rejected; resolve({ dispose }); await Promise.resolve(); expect(dispose).toHaveBeenCalledOnce()
})
it('does not let an obsolete returned lease restore effects over its successor', async () => {
  const f = engine(), lifetime = new AbortController(), effects = createRuntimeEffects(() => ({ effects: f.effects, signal: lifetime.signal }), 100)
  const old = await effects.acquire(reduced, new AbortController().signal)
  const next = await effects.acquire({ ...normal, lowPower: true }, new AbortController().signal)
  old.dispose(); expect(f.effects.read().lowPower).toBe(true); next.dispose(); expect(f.renderer).toEqual(f.original)
})
it('bounds a stuck effect acquisition and forwards timeout cancellation', async () => {
  vi.useFakeTimers(); let signal!: AbortSignal
  const f = borrowed((_value, s) => { signal = s; return new Promise(() => {}) })
  const rejected = expect(f.effects.acquire(reduced, new AbortController().signal)).rejects.toMatchObject({ reason: 'timeout' })
  await vi.advanceTimersByTimeAsync(100); await rejected; expect(signal.aborted).toBe(true)
})
it('does not displace a reentrant runtime successor while cancelling the previous acquisition', async () => {
  const f = borrowed(), first = await f.effects.acquire(normal, new AbortController().signal)
  let entered = false, successor: RuntimeEffectLease | Promise<RuntimeEffectLease> | undefined
  f.lifetime.signal.addEventListener('abort', () => { entered = true })
  first.dispose(); expect(entered).toBe(false)
  f.replace({ signal: new AbortController().signal, effects: { motion: true, lowPower: true, acquire(_v, signal) {
    signal.addEventListener('abort', () => { successor = f.effects.acquire(normal, new AbortController().signal) }, { once: true }); return { dispose() {} }
  } } })
  await f.effects.acquire(normal, new AbortController().signal)
  expect(() => f.effects.acquire(reduced, new AbortController().signal)).toThrow()
  expect(successor).toBeDefined()
})
