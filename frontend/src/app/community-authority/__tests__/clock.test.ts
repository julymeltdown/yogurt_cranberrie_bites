import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest'
import type { ShellBootstrap } from '../../ports'
import { createCommunityAuthority, type CommunityAuthorityOptions } from '..'
import { checkedBootstrap, fixture } from './fixtures'

let bootstrap: ShellBootstrap
const stops: (() => void)[] = []
beforeAll(async () => { bootstrap = await checkedBootstrap() })
beforeEach(() => vi.useFakeTimers())
afterEach(() => { stops.splice(0).reverse().forEach(fn => fn()); vi.useRealTimers() })
const setup = (patch: Partial<CommunityAuthorityOptions> = {}) => { const f = fixture(bootstrap, patch); stops.push(f.dispose); return f }
describe('canonical request server-time interval', () => {
  it('uses the full RTT upper bound, never receipt time or a midpoint', () => {
    const f = setup(), server = Date.parse(bootstrap.document!.server_time as string)
    expect(f.authority.readInterval()).toEqual({ scope: f.scope, lowerMs: server - 1000, upperMs: server + 1500, roundTripMs: 500 })
    f.set(900)
    expect(f.authority.clock.read()).toEqual({ scope: f.scope, nowMs: server + 1800 })
    // With millisecond precision and a 500ms RTT the receipt-only estimate is still 500ms too young.
    const document: Record<string, unknown> = { ...structuredClone(bootstrap.document!) }
    document.server_time = '2026-11-03T00:00:00.123Z'
    const deepFreeze = (value: unknown): void => { if (value && typeof value === 'object') { Object.values(value).forEach(deepFreeze); Object.freeze(value) } }
    deepFreeze(document)
    const precise = setup({ bootstrap: Object.freeze({ ...bootstrap, document }) })
    expect(precise.authority.readInterval()!.upperMs - Date.parse(document.server_time as string)).toBe(501)
  })
  it('includes the explicit clock uncertainty and rate envelope', () => {
    const f = setup({ qualification: { maxAgeMs: 10_000, uncertaintyMs: 20, maxElapsedRate: 1.1 } })
    f.set(1600)
    const server = Date.parse(bootstrap.document!.server_time as string), value = f.authority.readInterval()!
    expect(value.upperMs).toBe(server + 1020 + 1650)
    expect(value.lowerMs).toBe(Math.floor(server - 1020 + 1000 / 1.1))
  })
  it('does not consult the local calendar even when it jumps', () => {
    const f = setup(), initial = f.authority.clock.read()
    vi.spyOn(Date, 'now').mockImplementation(() => { throw Error('Calendar is not authority') })
    expect(f.authority.clock.read()).toEqual(initial)
    vi.restoreAllMocks()
  })
  it.each(['backward', 'gap', 'origin', 'nonfinite', 'interruption'] as const)('withdraws permanently and notifies on %s time', kind => {
    const f = setup(), listener = vi.fn(); f.authority.clock.subscribe(listener)
    if (kind === 'backward') f.set(599)
    if (kind === 'gap') f.set(3000)
    if (kind === 'origin') f.setOrigin(12346)
    if (kind === 'nonfinite') f.set(Number.NaN)
    if (kind === 'interruption') f.interrupt()
    expect(f.authority.clock.read()).toBeNull(); expect(f.authority.publicPlaces.read()).toBeNull()
    expect(listener).toHaveBeenCalledOnce()
    f.set(600); f.setOrigin(12345); expect(f.authority.clock.read()).toBeNull()
  })
  it.each(['identity', 'event', 'policy', 'abort', 'dispose'] as const)('withdraws synchronously on %s and cannot relabel its scope', kind => {
    const f = setup(), listener = vi.fn(); f.authority.places.subscribe(listener)
    if (kind === 'identity' || kind === 'event') f.session.replace(f.scope.generation, { userId: kind === 'identity' ? f.scope.eventId! : f.scope.userId!,
      eventId: kind === 'event' ? f.scope.userId! : f.scope.eventId!, accessToken: 'SYNTHETIC_ONLY' })
    if (kind === 'policy') f.privacy.changePolicy(f.scope, 'block')
    if (kind === 'abort') f.lifetime.abort()
    if (kind === 'dispose') f.authority.dispose()
    expect(listener).toHaveBeenCalledOnce(); expect(f.authority.readInterval()).toBeNull(); expect(f.authority.places.read()).toBeNull()
  })
  it('notifies maximum-age withdrawal even with no consumer reads and removes its timer', () => {
    const f = setup({ qualification: { maxAgeMs: 1000, uncertaintyMs: 0, maxElapsedRate: 1 } }), listener = vi.fn()
    f.authority.clock.subscribe(listener)
    vi.advanceTimersByTime(500)
    expect(listener).toHaveBeenCalledOnce(); expect(f.authority.clock.read()).toBeNull()
    f.dispose(); expect(vi.getTimerCount()).toBe(0)
  })
  it('rejects copied, reversed and foreign observations instead of guessing anchor age', () => {
    const f = setup()
    const make = (requestStarted = f.requestStarted, responseReceived = f.responseReceived) => createCommunityAuthority({ bootstrap,
      scope: f.scope, privacy: f.privacy, signal: f.lifetime.signal, timeline: f.timeline, requestStarted, responseReceived,
      qualification: { maxAgeMs: 10000, uncertaintyMs: 0, maxElapsedRate: 1 } })
    for (const owner of [make({ ...f.requestStarted }), make(f.responseReceived, f.requestStarted), make({ nowMs: 100, origin: 0 })]) {
      expect(owner.clock.read()).toBeNull(); owner.dispose()
    }
    expect(f.authority.clock.read()).not.toBeNull()
  })
  it.each([{ maxAgeMs: 0, uncertaintyMs: 0, maxElapsedRate: 1 }, { maxAgeMs: 1000, uncertaintyMs: -1, maxElapsedRate: 1 },
    { maxAgeMs: 1000, uncertaintyMs: 0, maxElapsedRate: 0.9 }, { maxAgeMs: Number.NaN, uncertaintyMs: 0, maxElapsedRate: 1 }])('rejects unqualified limits %j', qualification => {
    expect(setup({ qualification }).authority.clock.read()).toBeNull()
  })
  it('rejects missing or mutable decoded C03 and cross-user or event projections', () => {
    for (const value of [{ ...bootstrap, document: undefined }, { ...bootstrap, document: structuredClone(bootstrap.document) },
      { ...bootstrap, participant: { ...bootstrap.participant, userId: bootstrap.event.eventId } },
      { ...bootstrap, event: { ...bootstrap.event, eventId: bootstrap.participant.userId } }]) {
      expect(setup({ bootstrap: Object.freeze(value) as ShellBootstrap }).authority.clock.read()).toBeNull()
    }
  })
  it('tolerates throwing observers while every subscriber receives withdrawal', () => {
    const f = setup(), notified = vi.fn()
    f.authority.clock.subscribe(() => { throw Error('observer') }); f.authority.clock.subscribe(notified)
    f.lifetime.abort(); expect(notified).toHaveBeenCalledOnce()
    expect(f.authority.clock.read()).toBeNull()
  })
})
