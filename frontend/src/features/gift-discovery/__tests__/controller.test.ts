import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { CAPABILITIES } from '../../../app/capabilities'
import { giveawayStatus, reservationEligibility } from '../model/controller'
import { actorId, deferred, eventId, fixture, giveaway, now, otherActor, reservation, uid } from './fixtures/data'

const owned: ReturnType<typeof fixture>[] = []
const setup = (options: Parameters<typeof fixture>[0] = {}) => { const f = fixture(options); owned.push(f); return f }
const ready = async (f: ReturnType<typeof fixture>) => { f.controller.start(); await vi.waitFor(() => expect(f.controller.read().list.phase).toBe('ready')) }
beforeEach(() => vi.stubGlobal('location', { origin: 'https://mcr.example' }))
afterEach(() => { owned.splice(0).forEach(f => f.dispose()); vi.unstubAllGlobals(); vi.useRealTimers() })
it('uses only canonical FE09 protected domain reads and retains all seven capability reasons', async () => {
  const f = setup(); await ready(f); f.controller.select(giveaway().giveaway_id); await f.controller.loadActivity()
  await vi.waitFor(() => expect(f.controller.read().detail.phase).toBe('ready'))
  expect(f.requests).toHaveLength(3)
  for (const request of f.requests) { expect(request.headers.get('Authorization')).toBe('Bearer synthetic-memory-token'); expect(request.cache).toBe('no-store'); expect(request.credentials).toBe('same-origin'); expect(request.method).toBe('GET') }
  const capabilities = { ...f.context()!.capabilities }
  for (const key of CAPABILITIES) capabilities[key] = { available: false, reason: 'DEPENDENCY_UNAVAILABLE' }
  f.publish({ ...f.context()!, capabilities })
  expect(f.controller.read().context?.capabilities).toEqual(capabilities)
  expect(f.controller.read().list.data).toBeNull(); expect(f.requests).toHaveLength(3)
})
it('does not infer eligibility from partial, absent or terminal receiving pages', async () => {
  const f = setup(); await ready(f)
  expect(reservationEligibility(giveaway(), f.controller.read())).toMatch(/not been confirmed/)
  await f.controller.loadActivity()
  expect(reservationEligibility(giveaway(), f.controller.read())).toMatch(/already have/)
  expect(reservationEligibility(giveaway({ giveaway_id: uid(88) }), f.controller.read())).toMatch(/not been confirmed/)
  expect(reservationEligibility(giveaway({ owner: { ...giveaway().owner, user_id: actorId } }), f.controller.read())).toMatch(/giver/)
})
it('preserves independently paged giving/receiving and empty continuation without automatic loops', async () => {
  const f = setup({ handler: request => {
    const url = new URL(request.url)
    if (!url.pathname.includes('/me/')) return Response.json({ items: [], next_cursor: null })
    const givingNext = url.searchParams.get('giving_cursor'), receivingNext = url.searchParams.get('receiving_cursor')
    return Response.json({ giving: { items: givingNext ? [] : [giveaway({ owner: { ...giveaway().owner, user_id: actorId } })], next_cursor: givingNext ? 'giving_3' : 'giving_2' },
      receiving: { items: receivingNext ? [] : [reservation()], next_cursor: receivingNext ? null : 'receiving_2' } })
  } }); await ready(f); await f.controller.loadActivity()
  const initialReceiving = f.controller.read().receiving
  expect(f.controller.pageActivity('giving', 'next')).toBe(true)
  await vi.waitFor(() => expect(f.controller.read().giving.phase).toBe('ready'))
  expect(f.controller.read().giving.data).toEqual({ items: [], next_cursor: 'giving_3' })
  expect(f.controller.read().receiving).toBe(initialReceiving)
  const giving = f.controller.read().giving
  expect(f.controller.pageActivity('receiving', 'next')).toBe(true)
  await vi.waitFor(() => expect(f.controller.read().receiving.phase).toBe('ready'))
  expect(f.controller.read().giving).toBe(giving); expect(f.controller.read().receiving.page).toBe(2)
  expect(f.requests).toHaveLength(4)
  expect(f.controller.pageActivity('giving', 'previous')).toBe(true)
  await vi.waitFor(() => expect(f.controller.read().giving.page).toBe(1))
})
it('rejects a repeated ID across pages and resets cursor history on new filters', async () => {
  const f = setup({ handler: request => Response.json({ items: [giveaway()], next_cursor: new URL(request.url).searchParams.has('cursor') ? null : 'more' }) })
  await ready(f); expect(f.controller.pageList('next')).toBe(true)
  await vi.waitFor(() => expect(f.controller.read().list.phase).toBe('error'))
  expect(f.controller.read().list.data).toBeNull()
  f.controller.setFilters({}); await vi.waitFor(() => expect(f.controller.read().list.phase).toBe('ready'))
  expect(f.controller.read().list.page).toBe(1)
})
it('rejects a later page that moves forward in the frozen created_at/id ordering', async () => {
  const f = setup({ handler: request => {
    const later = new URL(request.url).searchParams.has('cursor')
    return Response.json({ items: [giveaway(later ? { giveaway_id: uid(99), created_at: '2026-11-03T02:00:00Z' } : {})], next_cursor: later ? null : 'older' })
  } })
  await ready(f); f.controller.pageList('next')
  await vi.waitFor(() => expect(f.controller.read().list.phase).toBe('error'))
  expect(f.controller.read().list.failure).toBe('invalid')
})
it.each(['logout', 'account', 'event', 'block', 'suspended', 'event-closed', 'consent-revoked'])('synchronously evicts and ignores abort-resistant late results after %s', async mode => {
  const pending = deferred<Response>(), f = setup({ handler: () => pending.promise }); f.controller.start()
  await vi.waitFor(() => expect(f.requests).toHaveLength(1))
  if (mode === 'logout') f.auth.session.replace(f.auth.session.read().generation, null)
  else if (mode === 'account' || mode === 'event') f.auth.session.replace(f.auth.session.read().generation, { userId: otherActor, eventId: mode === 'event' ? uid(900) : eventId, accessToken: 'next-token' })
  else f.auth.privacy.changePolicy(f.auth.privacy.capture(), mode as 'block')
  expect(f.controller.read().context).toBeNull(); expect(f.controller.read().list.data).toBeNull()
  pending.resolve(Response.json({ items: [giveaway()], next_cursor: null })); await new Promise(done => setTimeout(done, 5))
  expect(f.controller.read().list.phase).toBe('idle')
  expect(f.controller.setFilters({ in_stock: true })).toBe(false)
  expect(f.requests).toHaveLength(1)
})
it('requires freshly bound decisions after a policy change and recovers with current canonical scope', async () => {
  const f = setup(); await ready(f)
  f.auth.privacy.changePolicy(f.auth.privacy.capture(), 'block'); expect(f.controller.read().context).toBeNull()
  f.publish(f.context()); expect(f.controller.read().context).toBeNull()
  f.rebind(); await vi.waitFor(() => expect(f.controller.read().list.phase).toBe('ready'))
  expect(f.requests).toHaveLength(2)
})
it('cancels a stale filter result and rejects an invalid filter without dispatch', async () => {
  const pending = deferred<Response>(), f = setup({ handler: request => new URL(request.url).searchParams.has('in_stock')
    ? Response.json({ items: [giveaway({ available: 0 })], next_cursor: null }) : pending.promise })
  f.controller.start(); await vi.waitFor(() => expect(f.requests).toHaveLength(1))
  f.controller.setFilters({ in_stock: false }); await vi.waitFor(() => expect(f.controller.read().list.phase).toBe('ready'))
  pending.resolve(Response.json({ items: [giveaway()], next_cursor: null })); await new Promise(done => setTimeout(done, 5))
  expect(f.controller.read().list.data?.items[0]?.available).toBe(0)
  expect(f.controller.setFilters({ pickup_after: '2026-11-07T09:00:00Z', pickup_before: '2026-11-07T08:00:00Z' })).toBe(false)
  expect(f.requests).toHaveLength(2)
})
it('clears old detail on target change and never dispatches an invalid route ID', async () => {
  const f = setup(); await ready(f); f.controller.select(giveaway().giveaway_id)
  await vi.waitFor(() => expect(f.controller.read().detail.phase).toBe('ready'))
  f.controller.select('invalid'); expect(f.controller.read().detail.data).toBeNull(); expect(f.controller.read().detail.failure).toBe('missing')
  expect(f.requests).toHaveLength(2)
})
it('enforces pickup boundary timers, stock/state eligibility and releases all timers/listeners on disposal', async () => {
  vi.useFakeTimers(); vi.setSystemTime(now)
  const f = setup({ clock: { now: Date.now, setTimeout: (callback, delay) => setTimeout(callback, delay), clearTimeout: handle => clearTimeout(handle as ReturnType<typeof setTimeout>) } })
  f.controller.start(); await vi.advanceTimersByTimeAsync(1)
  expect(f.controller.read().list.phase).toBe('ready')
  expect(giveawayStatus(giveaway(), f.controller.read().now)).toBe('Open · pickup later')
  await vi.advanceTimersByTimeAsync(60 * 60 * 1000)
  expect(giveawayStatus(giveaway(), f.controller.read().now)).toBe('Open · pickup now')
  await vi.advanceTimersByTimeAsync(60 * 60 * 1000)
  expect(giveawayStatus(giveaway(), f.controller.read().now)).toBe('Ended')
  for (const patch of [{ available: 0 }, { state: 'PAUSED' as const }, { state: 'CLOSED' as const }, { state: 'CANCELLED' as const }]) {
    expect(reservationEligibility(giveaway(patch), f.controller.read())).toMatch(/unavailable/)
  }
  f.controller.dispose(); expect(f.listeners()).toBe(0); expect(vi.getTimerCount()).toBe(0)
  expect(f.controller.read().disposed).toBe(true)
})
it.each(['EVENT_READ_ONLY', 'DEPENDENCY_UNAVAILABLE', 'PROVIDER_UNAVAILABLE', 'FEATURE_DISABLED', 'NOT_ELIGIBLE'] as const)('retains the current mutation denial reason %s and fails closed', async reason => {
  const f = setup(); await ready(f)
  f.publish({ ...f.context()!, capabilities: { ...f.context()!.capabilities, mutations: { available: false, reason } } })
  expect(reservationEligibility(giveaway(), f.controller.read())).toContain(reason)
})
it('suppresses reads when a cleanup subscriber disposes reentrantly during loading', async () => {
  const f = setup()
  f.controller.subscribe(() => { if (f.controller.read().list.phase === 'loading') f.controller.dispose() })
  f.controller.start(); await new Promise(done => setTimeout(done, 5))
  expect(f.requests).toHaveLength(0); expect(f.listeners()).toBe(0); expect(f.controller.read().disposed).toBe(true)
})
it('honors the registered rate-limit delay without adding automatic retry', async () => {
  vi.useFakeTimers(); vi.setSystemTime(now)
  const f = setup({ clock: { now: Date.now, setTimeout: (cb, delay) => setTimeout(cb, delay), clearTimeout: handle => clearTimeout(handle as ReturnType<typeof setTimeout>) },
    handler: () => Response.json({ code: 'RATE', message: 'private', details: { reason: 'RATE_LIMITED', retry_after_ms: 1000 } }, { status: 429 }) })
  f.controller.start(); await vi.advanceTimersByTimeAsync(1)
  expect(f.controller.read().list.failure).toBe('rate-limited')
  await f.controller.retryList(); expect(f.requests).toHaveLength(1)
  await vi.advanceTimersByTimeAsync(1000); expect(f.requests).toHaveLength(1)
  await f.controller.retryList(); expect(f.requests).toHaveLength(2)
})
it('bounds explicit continuation sessions at twenty pages and never auto-follows empty pages', async () => {
  let count = 0
  const f = setup({ handler: () => Response.json({ items: [], next_cursor: `page_${++count}` }) })
  await ready(f); expect(f.requests).toHaveLength(1)
  for (let page = 2; page <= 20; page++) {
    expect(f.controller.pageList('next')).toBe(true)
    await vi.waitFor(() => expect(f.controller.read().list.phase).toBe('ready'))
    expect(f.controller.read().list.page).toBe(page)
  }
  expect(f.controller.pageList('next')).toBe(false); expect(f.requests).toHaveLength(20)
})
