import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createCommunityClockRuntime, createProductionCommunityClock } from '..'
import { bootstrap, fixture, testProfile } from '../../community-adoption/__tests__/fixtures'
import type { CommunityClockProfile } from '../../community-adoption/bootstrap'
import { environment, syntheticInput } from './fixture'

const cleanups: (() => void)[] = []
beforeEach(() => { vi.useFakeTimers(); vi.stubGlobal('location', { origin: 'https://community.test' }) })
afterEach(() => { cleanups.reverse().splice(0).forEach(fn => fn()); vi.unstubAllGlobals(); vi.unstubAllEnvs(); vi.useRealTimers() })
function runtime() {
  const input = syntheticInput(), env = environment(input), clock = createCommunityClockRuntime(JSON.stringify(input), env.value)
  cleanups.push(clock.dispose); return { clock, env }
}
describe('runtime factory and original canonical response binding', () => {
  it('uses the one decoded response and clamps its age to the evidence server expiry', async () => {
    const { clock } = runtime(), source = await bootstrap(), profile = clock.profile!
    expect(profile.runtime.qualify(source)!.maxAgeMs).toBe(Math.floor(19000 / 1.2625))
    const f = fixture(source, profile); cleanups.push(f.dispose)
    const binding = await f.load()
    expect(f.requests).toHaveLength(1); expect(f.timelineCalls()).toBe(1)
    expect(binding.read()!.clock.read()).not.toBeNull()
    clock.dispose(); expect(binding.read()).toBeNull(); expect(vi.getTimerCount()).toBe(0)
  })
  it('withdraws a pending C03 and cannot resurrect after its late decoded response', async () => {
    const { clock } = runtime(), f = fixture(await bootstrap(), clock.profile, 'held'); cleanups.push(f.dispose)
    const pending = f.load()
    await vi.waitFor(() => expect(f.requests).toHaveLength(1))
    clock.dispose(); f.releaseResponse()
    const binding = await pending
    expect(binding.read()).toBeNull(); expect(f.requests).toHaveLength(1); expect(vi.getTimerCount()).toBe(0)
  })
  it.each(['old', 'expired', 'precision', 'event'] as const)('refuses %s server evidence at the SAME receipt', async kind => {
    const { clock } = runtime(), source = await bootstrap()
    const document = { ...source.document, server_time: kind === 'old' ? '2026-11-02T23:59:59Z'
      : kind === 'expired' ? '2026-11-03T00:00:21Z' : '2026-11-03T00:00:00.000Z' }
    const changed = kind === 'event' ? { ...source, event: { ...source.event, eventId: '00000000-0000-4000-8000-000000000202' } } : { ...source, document }
    expect(clock.profile!.runtime.qualify(changed)).toBeNull()
  })
  it.each(['interrupt', 'origin', 'browser', 'gap', 'backward', 'time-origin'] as const)('permanently withdraws on %s', kind => {
    const { clock, env } = runtime(), withdrawn = vi.fn(), profile = clock.profile!
    profile.runtime.subscribe(withdrawn)
    if (kind === 'interrupt') env.interrupt()
    if (kind === 'origin') env.setLocation('https://other.test')
    if (kind === 'browser') env.browser.platform = 'OTHER'
    if (kind === 'gap') env.setTime(1101)
    if (kind === 'backward') env.setTime(99)
    if (kind === 'time-origin') env.setOrigin(2)
    expect(profile.runtime.isCurrent()).toBe(false); expect(withdrawn).toHaveBeenCalledOnce()
    env.setTime(100); env.setOrigin(1); expect(profile.runtime.isCurrent()).toBe(false)
    expect(vi.getTimerCount()).toBe(0)
  })
  it('expires after the exercised duration despite continuous observations', () => {
    const { clock, env } = runtime()
    for (let ms = 150; ms <= 20200; ms += 50) { env.setTime(ms); vi.advanceTimersByTime(50) }
    expect(clock.profile!.runtime.isCurrent()).toBe(false); expect(vi.getTimerCount()).toBe(0)
  })
  it('notifies all borrowers even if an interruption cleanup or listener throws', () => {
    const input = syntheticInput(), env = environment(input)
    const clock = createCommunityClockRuntime(JSON.stringify(input), { ...env.value, onInterruption: () => () => { throw Error('adapter') } })
    const last = vi.fn(); clock.profile!.runtime.subscribe(() => { throw Error('borrower') }); clock.profile!.runtime.subscribe(last)
    clock.dispose(); expect(last).toHaveBeenCalledOnce(); expect(vi.getTimerCount()).toBe(0)
  })
  it('returns no profile without compiled production input or browser environment', () => {
    vi.stubEnv('VITE_MCR_COMMUNITY_CLOCK_INPUT', '')
    expect(createProductionCommunityClock(null).profile).toBeUndefined(); expect(vi.getTimerCount()).toBe(0)
  })
  it('cannot upgrade a rejected TEST_ONLY production profile through its runtime callback', async () => {
    vi.stubEnv('PROD', true)
    const { clock } = runtime(), qualify = vi.fn(() => testProfile)
    const f = fixture(await bootstrap(), { ...clock.profile!, classification: 'TEST_ONLY', runtime: { ...clock.profile!.runtime, qualify } }); cleanups.push(f.dispose)
    expect((await f.load()).read()).toBeNull(); expect(f.timelineCalls()).toBe(0); expect(qualify).not.toHaveBeenCalled()
  })
  it('rejects an unsupported fingerprint before starting timers', () => {
    const input = syntheticInput(), env = environment(input); env.browser.userAgent = 'DIFFERENT'
    expect(createCommunityClockRuntime(JSON.stringify(input), env.value).profile).toBeUndefined()
    expect(vi.getTimerCount()).toBe(0)
  })
  it.each(['hidden', 'freeze', 'resume', 'pagehide', 'bfcache'] as const)('production browser adapter withdraws on %s (controlled unit environment)', kind => {
    const input = syntheticInput(), window = Object.assign(new EventTarget(), {
      location: { origin: input.current.origin }, crossOriginIsolated: false,
      performance: { now: () => 100, timeOrigin: 1 },
    }), document = Object.assign(new EventTarget(), { visibilityState: 'visible' })
    vi.stubGlobal('window', window); vi.stubGlobal('document', document)
    vi.stubGlobal('navigator', input.measurement.browser)
    vi.stubEnv('VITE_MCR_COMMUNITY_CLOCK_INPUT', JSON.stringify(input))
    const clock = createProductionCommunityClock(input.current.eventId); cleanups.push(clock.dispose)
    expect(clock.profile!.runtime.isCurrent()).toBe(true)
    if (kind === 'hidden') { document.visibilityState = 'hidden'; document.dispatchEvent(new Event('visibilitychange')) }
    else if (kind === 'freeze' || kind === 'resume') document.dispatchEvent(new Event(kind))
    else window.dispatchEvent(kind === 'bfcache' ? Object.assign(new Event('pageshow'), { persisted: true }) : new Event('pagehide'))
    expect(clock.profile!.runtime.isCurrent()).toBe(false); expect(vi.getTimerCount()).toBe(0)
  })
  it.each(['isCurrent', 'subscribe', 'qualify', 'synchronous-withdrawal'] as const)('fails closed when runtime %s fails', async mode => {
    let subscribed = 0, unsubscribed = 0
    const runtime: NonNullable<CommunityClockProfile['runtime']> = {
      isCurrent() { if (mode === 'isCurrent') throw Error('lifetime'); return true },
      subscribe(fn) { subscribed++; if (mode === 'subscribe') throw Error('subscribe'); if (mode === 'synchronous-withdrawal') fn(); return () => { unsubscribed++ } },
      qualify() { if (mode === 'qualify') throw Error('qualify'); return testProfile },
    }
    const f = fixture(await bootstrap(), { ...testProfile, runtime }); cleanups.push(f.dispose)
    const binding = await f.load()
    expect(binding.read()).toBeNull(); expect(vi.getTimerCount()).toBe(0)
    if (mode === 'qualify' || mode === 'synchronous-withdrawal') expect(unsubscribed).toBe(subscribed)
  })
})
