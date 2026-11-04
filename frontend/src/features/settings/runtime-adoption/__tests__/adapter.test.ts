import { afterEach, describe, expect, it, vi } from 'vitest'
import { createRuntimePreferencesAdapter, type RuntimeAdoptionOptions, type RuntimeEffectLease, type MotionPreference } from '..'
import { defaults } from '../../../me/model/contract'
import { privacyOnly } from '../../../me/__tests__/fixtures'
import { createThemeController } from '../../../theme/model/controller'
import { MapError } from '../../../../world/map/json'

const deferred = <T,>() => { let resolve!: (value: T) => void, reject!: (error: Error) => void; const promise = new Promise<T>((r, j) => { resolve = r; reject = j }); return { promise, resolve, reject } }
const cleanups: (() => void)[] = []
afterEach(() => { cleanups.splice(0).reverse().forEach(fn => fn()); vi.useRealTimers() })
function setup(overrides: Partial<RuntimeAdoptionOptions> = {}) {
  const p = privacyOnly(), scope = p.privacy.capture(), lifetime = new AbortController(), caller = new AbortController()
  const disposeEffect = vi.fn(), disposeSound = vi.fn(), listeners = new Set<() => void>()
  let reduced = false
  const motion: MotionPreference = { get matches() { return reduced }, addEventListener: (_type, fn) => { listeners.add(fn) },
    removeEventListener: (_type, fn) => { listeners.delete(fn) } }
  const effects = { motion: true, lowPower: true, acquire: vi.fn(() => ({ dispose: disposeEffect })) }
  const sound = { acquire: vi.fn(() => ({ dispose: disposeSound })) }
  const theme = createThemeController({ initial: 'REVENGE', scope: { userId: scope.userId!, eventId: scope.eventId! },
    preferences: { read: () => null, write() {} }, runtime: { readSkin: () => 'REVENGE', replaceSkin: async () => { throw new MapError('unavailable') } },
    load: async () => { throw new MapError('unavailable') } })
  const adapter = createRuntimePreferencesAdapter({ privacy: p.privacy, scope, signal: lifetime.signal, motion, effects, sound, theme, ...overrides })
  cleanups.push(() => { adapter.dispose(); theme.dispose(); p.privacy.dispose(); p.client.clear() })
  return { ...p, adapter, scope, caller, lifetime, effects, sound, disposeEffect, disposeSound, theme, listeners,
    os(value: boolean) { reduced = value; [...listeners].forEach(fn => fn()) },
    apply: (values = defaults) => adapter.apply(values, scope, caller.signal) }
}
describe('same-shell FE34 runtime adoption', () => {
  it('is inert at construction and requires an explicit apply to acquire effects or audio', async () => {
    const f = setup()
    expect(f.effects.acquire).not.toHaveBeenCalled(); expect(f.sound.acquire).not.toHaveBeenCalled()
    expect(f.adapter.read()).toMatchObject({ phase: 'idle', soundEnabled: false })
    await f.apply()
    expect(f.effects.acquire).toHaveBeenCalledOnce(); expect(f.sound.acquire).not.toHaveBeenCalled()
    expect(f.adapter.read()).toMatchObject({ phase: 'applied', soundEnabled: false })
  })
  it('acquires optional sound synchronously inside the explicit action and releases it on caller abort after settlement', async () => {
    const f = setup(), apply = f.apply({ ...defaults, sound_enabled: true })
    expect(f.sound.acquire).toHaveBeenCalledOnce()
    await apply; expect(f.adapter.read().soundEnabled).toBe(true)
    f.caller.abort()
    expect(f.disposeSound).toHaveBeenCalledOnce(); expect(f.disposeEffect).toHaveBeenCalledOnce()
    expect(f.adapter.read()).toMatchObject({ phase: 'idle', soundEnabled: false })
  })
  it('rejects absent capabilities and missing audio without falsely applying or calling theme', async () => {
    const missingEffects = setup({ effects: undefined } as unknown as Partial<RuntimeAdoptionOptions>)
    await expect(missingEffects.apply()).rejects.toMatchObject({ reason: 'unavailable' })
    const missingSound = setup({ sound: undefined } as unknown as Partial<RuntimeAdoptionOptions>)
    await missingSound.apply()
    await expect(missingSound.apply({ ...defaults, sound_enabled: true })).rejects.toMatchObject({ reason: 'unavailable' })
    expect(missingSound.adapter.read()).toMatchObject({ phase: 'failed', soundEnabled: false })
  })
  it('reports requested unsupported motion/power while baseline false can remain available', async () => {
    const acquire = vi.fn(() => ({ dispose() {} })), f = setup({ effects: { motion: false, lowPower: false, acquire } })
    await f.apply()
    await expect(f.apply({ ...defaults, low_power: true })).rejects.toMatchObject({ reason: 'unavailable' })
    await expect(f.apply({ ...defaults, reduced_motion: true })).rejects.toMatchObject({ reason: 'unavailable' })
    expect(acquire).toHaveBeenCalledOnce()
  })
  it('honors OS reduced motion, updates on change, and never restarts sound on OS updates', async () => {
    const f = setup(); f.os(true)
    await f.apply({ ...defaults, low_power: true, sound_enabled: true })
    expect(f.effects.acquire.mock.calls[0]).toEqual([{ reducedMotion: true, lowPower: true }, expect.any(AbortSignal)])
    f.os(false)
    await vi.waitFor(() => expect(f.adapter.read().reducedMotion).toBe(false))
    expect(f.effects.acquire).toHaveBeenCalledTimes(2); expect(f.sound.acquire).toHaveBeenCalledOnce()
    expect(f.disposeEffect).toHaveBeenCalledOnce()
    f.adapter.dispose(); expect(f.listeners.size).toBe(0)
  })
  it('does not let an OS override disable explicit reduced motion', async () => {
    const f = setup(); await f.apply({ ...defaults, reduced_motion: true }); f.os(false)
    await vi.waitFor(() => expect(f.effects.acquire).toHaveBeenCalledTimes(2))
    expect(f.adapter.read().reducedMotion).toBe(true)
  })
  it('stops existing audio and reports an unavailable OS override if motion support is absent', async () => {
    const f = setup({ effects: { motion: false, lowPower: true, acquire: () => ({ dispose() {} }) } })
    await f.apply({ ...defaults, sound_enabled: true }); f.os(true)
    await vi.waitFor(() => expect(f.adapter.read().phase).toBe('failed'))
    expect(f.adapter.read()).toMatchObject({ reason: 'unavailable', soundEnabled: false }); expect(f.disposeSound).toHaveBeenCalledOnce()
  })
  it('rejects a failed sound source and malformed capability lease rather than claiming success', async () => {
    const f = setup({ sound: { acquire() { throw new Error('Audio blocked') } } })
    await expect(f.apply({ ...defaults, sound_enabled: true })).rejects.toMatchObject({ reason: 'failed' })
    expect(f.disposeEffect).toHaveBeenCalledOnce(); expect(f.adapter.read().phase).toBe('failed')
    const malformed = setup({ effects: { motion: true, lowPower: true, acquire: () => undefined as never } })
    await expect(malformed.apply()).rejects.toMatchObject({ reason: 'failed' })
  })
  it.each(['policy', 'identity', 'event', 'release', 'dispose'] as const)('withdraws effects/audio immediately on %s transfer and rejects stale apply', async kind => {
    const f = setup(); await f.apply({ ...defaults, sound_enabled: true })
    if (kind === 'policy') f.privacy.changePolicy(f.scope, 'block')
    if (kind === 'identity') f.session.replace(f.scope.generation, { userId: f.scope.eventId!, eventId: f.scope.eventId!, accessToken: 'FIXTURE_ONLY' })
    if (kind === 'event') f.session.replace(f.scope.generation, { userId: f.scope.userId!, eventId: f.scope.userId!, accessToken: 'FIXTURE_ONLY' })
    if (kind === 'release') f.lifetime.abort()
    if (kind === 'dispose') f.adapter.dispose()
    expect(f.disposeEffect).toHaveBeenCalledOnce(); expect(f.disposeSound).toHaveBeenCalledOnce()
    await expect(f.apply()).rejects.toMatchObject({ reason: 'cancelled' })
    expect(f.adapter.read().phase).toBe('disposed'); expect(f.listeners.size).toBe(0)
  })
  it('rejects different user/event/generation/policy/gate and malformed values', async () => {
    const f = setup()
    for (const field of ['userId', 'eventId', 'generation', 'policyGeneration', 'gate'] as const) {
      await expect(f.adapter.apply(defaults, { ...f.scope, [field]: 'foreign' }, f.caller.signal)).rejects.toMatchObject({ reason: 'cancelled' })
    }
    await expect(f.apply({ ...defaults, sound_enabled: 'yes' } as never)).rejects.toMatchObject({ reason: 'unavailable' })
    expect(f.effects.acquire).not.toHaveBeenCalled()
  })
  it('uses the actual existing ThemeController and rejects its swallowed asset failure', async () => {
    const f = setup()
    await expect(f.apply({ ...defaults, theme: 'PARADE', sound_enabled: true })).rejects.toMatchObject({ reason: 'failed' })
    expect(f.theme.read()).toMatchObject({ selected: 'REVENGE', phase: 'error' })
    expect(f.disposeEffect).toHaveBeenCalledOnce(); expect(f.disposeSound).toHaveBeenCalledOnce()
    expect(f.adapter.read()).toMatchObject({ phase: 'failed', soundEnabled: false })
  })
  it('does not let a stale or already-aborted caller revoke the current applied lease', async () => {
    const f = setup(); await f.apply({ ...defaults, sound_enabled: true })
    await expect(f.adapter.apply(defaults, { ...f.scope, generation: f.scope.generation + 1 }, f.caller.signal)).rejects.toMatchObject({ reason: 'cancelled' })
    const stopped = new AbortController(); stopped.abort()
    await expect(f.adapter.apply(defaults, f.scope, stopped.signal)).rejects.toMatchObject({ reason: 'cancelled' })
    expect(f.disposeSound).not.toHaveBeenCalled(); expect(f.disposeEffect).not.toHaveBeenCalled()
    expect(f.adapter.read()).toMatchObject({ phase: 'applied', soundEnabled: true })
  })
  it('fences ignored-abort acquisition and releases its eventual lease after a successor applies', async () => {
    const late = deferred<RuntimeEffectLease>(), oldDispose = vi.fn(), newDispose = vi.fn()
    const acquire = vi.fn().mockImplementationOnce(() => late.promise).mockImplementation(() => ({ dispose: newDispose }))
    const f = setup({ effects: { motion: true, lowPower: true, acquire } })
    const first = f.apply(), rejected = expect(first).rejects.toMatchObject({ reason: 'cancelled' })
    await f.apply({ ...defaults, reduced_motion: true }); await rejected
    late.resolve({ dispose: oldDispose }); await vi.waitFor(() => expect(oldDispose).toHaveBeenCalledOnce())
    expect(newDispose).not.toHaveBeenCalled(); expect(f.adapter.read()).toMatchObject({ phase: 'applied', reducedMotion: true })
  })
  it('bounds ignored-abort hangs, stops audio and releases eventual effect leases', async () => {
    vi.useFakeTimers()
    const late = deferred<RuntimeEffectLease>(), dispose = vi.fn(), f = setup({ timeoutMs: 40,
      effects: { motion: true, lowPower: true, acquire: () => late.promise } })
    const apply = f.apply({ ...defaults, sound_enabled: true }), rejected = expect(apply).rejects.toMatchObject({ reason: 'timeout' })
    await vi.advanceTimersByTimeAsync(41); await rejected
    expect(f.disposeSound).toHaveBeenCalledOnce(); expect(f.adapter.read()).toMatchObject({ phase: 'failed', reason: 'timeout' })
    late.resolve({ dispose }); await vi.advanceTimersByTimeAsync(0); expect(dispose).toHaveBeenCalledOnce()
  })
  it('keeps the latest OS effect when a superseded acquisition later fails or resolves', async () => {
    const stale = deferred<RuntimeEffectLease>(), latest = deferred<RuntimeEffectLease>(), dispose = vi.fn()
    const acquire = vi.fn().mockImplementationOnce(() => stale.promise).mockImplementationOnce(() => latest.promise)
    const f = setup({ effects: { motion: true, lowPower: true, acquire } })
    const applying = f.apply(); f.os(true)
    latest.resolve({ dispose }); stale.reject(new Error('Obsolete effect failure'))
    await applying
    expect(f.adapter.read()).toMatchObject({ phase: 'applied', reducedMotion: true }); expect(dispose).not.toHaveBeenCalled()
  })
  it('does not overwrite a reentrant successor started by an old effect disposal', async () => {
    let reenter = () => {}, next: Promise<void> | null = null
    const f = setup({ effects: { motion: true, lowPower: true, acquire: () => ({ dispose: () => reenter() }) } })
    await f.apply()
    reenter = () => { reenter = () => {}; next = f.apply({ ...defaults, reduced_motion: true }) }
    await expect(f.apply()).rejects.toMatchObject({ reason: 'cancelled' })
    await next
    expect(f.adapter.read()).toMatchObject({ phase: 'applied', reducedMotion: true })
  })
  it('withdraws applied state when releasing the old lease aborts the incoming caller', async () => {
    const incoming = new AbortController(), dispose = vi.fn(() => incoming.abort())
    const acquire = vi.fn(() => ({ dispose })), phases: string[] = []
    const f = setup({ effects: { motion: true, lowPower: true, acquire } })
    f.adapter.subscribe(() => phases.push(f.adapter.read().phase))
    await f.apply({ ...defaults, reduced_motion: true, sound_enabled: true })
    expect(f.adapter.read()).toMatchObject({ phase: 'applied', reducedMotion: true, soundEnabled: true })

    await expect(f.adapter.apply(defaults, f.scope, incoming.signal)).rejects.toMatchObject({ reason: 'cancelled' })

    expect(dispose).toHaveBeenCalledOnce(); expect(f.disposeSound).toHaveBeenCalledOnce()
    expect(acquire).toHaveBeenCalledOnce(); expect(f.sound.acquire).toHaveBeenCalledOnce()
    expect(f.adapter.read()).toMatchObject({ phase: 'idle', reducedMotion: false, soundEnabled: false })
    expect(phases.at(-1)).toBe('idle')
  })
  it('cancels a real same-theme selection before publication without disposing the borrowed theme owner', async () => {
    const f = setup(), select = vi.spyOn(f.theme, 'select'), apply = f.apply()
    f.caller.abort(); await expect(apply).rejects.toMatchObject({ reason: 'cancelled' })
    expect(select).not.toHaveBeenCalled(); expect(f.theme.read().phase).toBe('ready')
  })
  it('disposes audio that ignores abort and settles only after adapter teardown', async () => {
    const late = deferred<RuntimeEffectLease>(), dispose = vi.fn(), f = setup({ sound: { acquire: () => late.promise } })
    const apply = f.apply({ ...defaults, sound_enabled: true }), rejected = expect(apply).rejects.toMatchObject({ reason: 'cancelled' })
    f.adapter.dispose(); await rejected
    late.resolve({ dispose }); await vi.waitFor(() => expect(dispose).toHaveBeenCalledOnce())
    expect(f.adapter.read()).toMatchObject({ phase: 'disposed', soundEnabled: false })
  })
})

// The adapter uses public controller behavior, not a fabricated successful ThemeState.
// A switching fixture for caller-abort is added when the World owner delivers select(signal).
