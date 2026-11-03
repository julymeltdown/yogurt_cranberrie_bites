import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { deferred, error, event, id, later, page, pending, setup, stamp, uid, update, user } from './fixtures/data'
const owners: ReturnType<typeof setup>[] = []
const own = (...args: Parameters<typeof setup>) => { const h = setup(...args); owners.push(h); return h }
const count = (total = 29) => ({ actor_id: user, event_id: event, unread_total: total, as_of: stamp })
const ready = async (h: ReturnType<typeof setup>) => { h.controller.start(); await vi.waitFor(() => expect(h.controller.read().count.phase).toBe('ready')) }
beforeEach(() => vi.stubGlobal('location', { origin: 'https://mcr.example' }))
afterEach(() => { owners.splice(0).forEach(h => h.dispose()); vi.unstubAllGlobals() })
it('reads the canonical complete own inbox count without cursor/filter/body, independently of loaded rows', async () => {
  const h = own(undefined, true, r => {
    expect(new URL(r.url).pathname).toBe(`/api/concert/events/${event}/me/notifications/unread-count`)
    expect(new URL(r.url).search).toBe(''); expect(r.method).toBe('GET'); expect(r.body).toBeNull()
    expect(r.cache).toBe('no-store'); expect(r.headers.get('Authorization')).toBe('Bearer memory-only-token')
    return Response.json(count(999))
  })
  await ready(h); expect(h.controller.read()).toMatchObject({ unreadShown: 1, count: { total: 999, asOf: stamp } })
  h.controller.setUnreadOnly(true); await vi.waitFor(() => expect(h.countFetch).toHaveBeenCalledTimes(2)); expect(h.controller.read().count.total).not.toBe(1)
})
it.each(['actor', 'event', 'negative', 'fraction', 'unsafe', 'extra', 'timestamp'] as const)('rejects %s count without substituting loaded unread rows', async mode => {
  const h = own(undefined, true, () => Response.json({ ...count(), ...(mode === 'actor' ? { actor_id: uid(99) } : mode === 'event' ? { event_id: uid(99) } : mode === 'negative' ? { unread_total: -1 } : mode === 'fraction' ? { unread_total: 1.5 } : mode === 'unsafe' ? { unread_total: Number.MAX_SAFE_INTEGER + 1 } : mode === 'extra' ? { payload: '@private' } : { as_of: '2026-02-30T00:00:00Z' }) }))
  h.controller.start(); await vi.waitFor(() => expect(h.controller.read().count.phase).toBe('error'))
  expect(h.controller.read().count).toMatchObject({ total: null, asOf: null }); expect(h.controller.read().rows).toHaveLength(1)
})
it.each([0, Number.MAX_SAFE_INTEGER])('accepts valid boundary %i independently of the current list observation', async total => {
  const h = own(undefined, true, () => Response.json(count(total))); await ready(h); expect(h.controller.read().count.total).toBe(total)
})
it('count producer outage leaves the real list usable and total unavailable', async () => {
  const h = own(undefined, true, () => error(503, 'DEPENDENCY_UNAVAILABLE')); h.controller.start()
  await vi.waitFor(() => expect(h.controller.read().count.phase).toBe('error'))
  expect(h.controller.read()).toMatchObject({ phase: 'ready', unreadShown: 1, count: { total: null } })
})
it('fences an older count response after explicit refresh', async () => {
  const late = deferred<Response>(); let calls = 0
  const h = own(undefined, true, () => ++calls === 1 ? late.promise : Response.json(count(37)))
  h.controller.start(); await vi.waitFor(() => expect(h.countFetch).toHaveBeenCalledTimes(1)); await h.controller.loadUnreadCount()
  late.resolve(Response.json(count(12))); await Promise.resolve(); expect(h.controller.read().count.total).toBe(37)
})
it('privacy revocation clears observation synchronously and late count never republishes', async () => {
  const late = deferred<Response>(); const h = own(undefined, true, () => late.promise)
  h.controller.start(); await vi.waitFor(() => expect(h.countFetch).toHaveBeenCalledTimes(1)); h.auth.session.replace(1, null)
  expect(h.controller.read().count).toMatchObject({ phase: 'idle', total: null, asOf: null })
  late.resolve(Response.json(count())); await Promise.resolve(); expect(h.controller.read().count.total).toBeNull()
})
it('invalidations clear total and require a fresh server observation', async () => {
  const h = own(); await ready(h); h.controller.invalidate(h.auth.privacy.capture())
  expect(h.controller.read().count.total).toBeNull(); expect(h.countFetch).toHaveBeenCalledTimes(1)
  await h.controller.load(); await vi.waitFor(() => expect(h.controller.read().count.total).toBe(29))
})
it.each(['single', 'all'] as const)('refreshes canonical total after %s read without local subtraction', async mode => {
  let committed = false
  const h = own(r => {
    if (r.method === 'PATCH') { committed = true; return Response.json(mode === 'single' ? update({ read_at: later, revision: 2 }) : { read_at: later, read_count: 1 }) }
    return Response.json(page([update(committed ? { read_at: later, revision: 2 } : {})]))
  }, true, () => Response.json(count(committed ? 91 : 29)))
  await ready(h); await (mode === 'single' ? h.controller.markRead(id) : h.controller.markAllRead())
  await vi.waitFor(() => expect(h.controller.read().count.total).toBe(91)); expect(h.countFetch).toHaveBeenCalledTimes(2)
})

it.each([0, 37])('read-all uses canonical total %i even when the loaded page has no unread rows', async total => {
  const h = own(r => r.method === 'PATCH' ? Response.json({ read_at: later, read_count: 37 }) : Response.json(page([])), true, () => Response.json(count(total)))
  await ready(h); await vi.waitFor(() => expect(h.controller.read().phase).toBe('ready'))
  expect(h.controller.read().unreadShown).toBe(0)
  await h.controller.markAllRead()
  expect(h.fetch.mock.calls.filter(([r]) => (r as Request).method === 'PATCH')).toHaveLength(total > 0 ? 1 : 0)
})

it.each(['uncertain', 'sending'] as const)('producer outage clears count/rows but retains the %s command for original-key recovery', async mode => {
  const late = deferred<Response>(); let commandKey = '', recovered = false
  const h = own(r => {
    if (r.method === 'PATCH') { commandKey = r.headers.get('Idempotency-Key')!; if (mode === 'sending') return late.promise; throw new TypeError('lost response') }
    if (new URL(r.url).pathname.endsWith('/commands')) {
      expect(new URL(r.url).searchParams.get('idempotency_key')).toBe(commandKey); recovered = true
      return Response.json(pending('markConcertNotification', commandKey, { state: 'SUCCEEDED', resource_id: id, resource_revision: 2, result_status: 200 }))
    }
    return Response.json(page([update(recovered ? { read_at: later, revision: 2 } : {})]))
  })
  await ready(h); await vi.waitFor(() => expect(h.controller.read().phase).toBe('ready'))
  const mark = h.controller.markRead(id)
  await vi.waitFor(() => expect(h.controller.read().command.phase).toBe(mode))
  h.setAvailable(false); await mark
  expect(h.controller.read()).toMatchObject({ available: false, rows: [], count: { total: null }, command: { phase: 'uncertain' } })
  h.setAvailable(true); await vi.waitFor(() => expect(h.controller.read().phase).toBe('ready'))
  await h.controller.markRead(id); expect(h.fetch.mock.calls.filter(([r]) => (r as Request).method === 'PATCH')).toHaveLength(1)
  await h.controller.recover(); expect(h.controller.read().command.phase).toBe('succeeded')
  late.resolve(Response.json(update({ read_at: later, revision: 2 }))); await Promise.resolve()
  expect(h.fetch.mock.calls.filter(([r]) => (r as Request).method === 'PATCH')).toHaveLength(1)
})
