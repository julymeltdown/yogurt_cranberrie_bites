import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { deferred, error, event, id, later, page, pending, setup, uid, update, user } from './fixtures/data'
const owners: ReturnType<typeof setup>[] = []
const own = (...args: Parameters<typeof setup>) => { const h = setup(...args); owners.push(h); return h }
const ready = async (h: ReturnType<typeof setup>) => { h.controller.start(); await vi.waitFor(() => expect(h.controller.read().phase).toBe('ready')) }
beforeEach(() => vi.stubGlobal('location', { origin: 'https://mcr.example' }))
afterEach(() => { owners.splice(0).forEach(h => h.dispose()); vi.unstubAllGlobals() })
describe('private bounded inbox', () => {
  it('uses only canonical identity, event and qualified no-store list; count describes loaded rows', async () => {
    const h = own(r => {
      expect(r.headers.get('Authorization')).toBe('Bearer memory-only-token'); expect(r.cache).toBe('no-store'); expect(r.credentials).toBe('same-origin')
      const u = new URL(r.url); expect(u.pathname).toBe(`/api/concert/events/${event}/me/notifications`)
      expect(u.searchParams.get('limit')).toBe('20'); expect(u.searchParams.get('unread_only')).toBe('false')
      return Response.json(page([update(), update({ notification_id: uid(5), read_at: later })], 'next_page'))
    })
    await ready(h); expect(h.controller.read()).toMatchObject({ unreadShown: 1, nextCursor: 'next_page', scope: { userId: user, eventId: event } })
  })
  it('deduplicates repeated notifications and overlapping pages by ID/revision', async () => {
    const h = own(r => Response.json(new URL(r.url).searchParams.has('cursor') ? page([update(), update({ notification_id: uid(5) })]) : page([update(), update()], 'page_2')))
    await ready(h); expect(h.controller.read().rows).toHaveLength(1)
    await h.controller.load(true); expect(h.controller.read().rows).toHaveLength(2); expect(h.controller.read().unreadShown).toBe(2)
  })
  it('rejects conflicting same-revision duplicate projections without showing private data', async () => {
    const h = own(() => Response.json(page([update(), update({ target: null })])))
    h.controller.start(); await vi.waitFor(() => expect(h.controller.read().phase).toBe('error')); expect(h.controller.read().rows).toHaveLength(0)
  })
  it.each(['event', 'unknown-field', 'raw-payload', 'bad-time', 'oversized', 'unread-filter'] as const)('rejects %s list', async mode => {
    const h = own(() => Response.json(page(mode === 'oversized' ? Array.from({ length: 21 }, (_, n) => update({ notification_id: uid(n + 10) })) : [{ ...update(),
      ...(mode === 'event' ? { event_id: uid(99) } : mode === 'unknown-field' ? { owner_id: user } : mode === 'raw-payload' ? { payloadJson: { handle: '@secret', location: 'private' } } : mode === 'bad-time' ? { created_at: '2026-02-30T00:00:00Z' } : mode === 'unread-filter' ? { read_at: later } : {}) }])))
    if (mode === 'unread-filter') { await ready(h); h.controller.setUnreadOnly(true) } else h.controller.start()
    await vi.waitFor(() => expect(h.controller.read().phase).toBe('error')); expect(h.controller.read().rows).toHaveLength(0)
  })
  it('rejects repeated cursor and bounds duplicate-only page chains', async () => {
    const h = own(() => Response.json(page([update()], 'same_cursor')))
    await ready(h); await h.controller.load(true); expect(h.controller.read().phase).toBe('error'); expect(h.controller.read().rows).toHaveLength(0)
  })
  it('unavailable producer never synthesizes notifications or dispatches requests', async () => {
    const h = own(undefined, false); h.controller.start(); await h.controller.load(); expect(h.fetch).not.toHaveBeenCalled(); expect(h.controller.read().available).toBe(false)
    h.setAvailable(true); await vi.waitFor(() => expect(h.controller.read().phase).toBe('ready'))
    h.setAvailable(false); expect(h.controller.read()).toMatchObject({ available: false, rows: [], scope: null })
  })
  it('invalidation accepts only current privacy scope, drops old targets, never ingests event payloads', async () => {
    const h = own(); await ready(h)
    const captured = h.auth.privacy.capture(); h.controller.invalidate({ ...captured, userId: uid(99) }); expect(h.controller.read().rows).toHaveLength(1)
    h.controller.invalidate(captured); h.controller.invalidate(captured); expect(h.controller.read()).toMatchObject({ rows: [], stale: true, unreadShown: 0 })
    await h.controller.load(); expect(h.controller.read().rows).toHaveLength(1); expect(h.fetch).toHaveBeenCalledTimes(2)
  })
  it.each(['identity', 'event', 'logout', 'block', 'suspended', 'dispose'] as const)('synchronously clears private inbox on %s and fences delayed reads', async mode => {
    const blocked = deferred<Response>(); let request = 0
    const h = own(() => {
      request++; if (request === 1) return Response.json(page())
      if (request === 2) return blocked.promise.then(() => Response.json(page()))
      return mode === 'block' ? Response.json(page()) : error(403, 'PERMISSION_DENIED')
    }); await ready(h)
    const policyBefore = h.auth.privacy.capture().policyGeneration
    const reading = h.controller.load()
    await vi.waitFor(() => expect(h.fetch).toHaveBeenCalledTimes(2))
    if (mode === 'dispose') h.controller.dispose()
    else if (mode === 'block' || mode === 'suspended') h.auth.privacy.changePolicy(h.auth.privacy.capture(), mode)
    else h.auth.session.replace(1, mode === 'logout' ? null : { userId: mode === 'identity' ? uid(99) : user, eventId: mode === 'event' ? uid(99) : event, accessToken: 'other' })
    expect(h.controller.read().rows).toHaveLength(0); await reading
    blocked.resolve(Response.json(page())); await Promise.resolve(); await Promise.resolve()
    if (mode === 'block') { await vi.waitFor(() => expect(h.controller.read().phase).toBe('ready')); expect(h.controller.read().scope?.policyGeneration).toBe(policyBefore + 1) }
    else if (mode === 'identity' || mode === 'event') { await vi.waitFor(() => expect(h.controller.read().phase).toBe('error')); expect(h.controller.read().rows).toHaveLength(0) }
    else expect(h.controller.read().rows).toHaveLength(0)
  })
  it('discarded filter reads cannot overwrite newer results', async () => {
    const blocked = deferred<Response>(); let calls = 0
    const h = own(r => ++calls === 2 ? blocked.promise : Response.json(page(new URL(r.url).searchParams.get('unread_only') === 'true' ? [] : [update()])))
    await ready(h); const old = h.controller.load(); await vi.waitFor(() => expect(h.fetch).toHaveBeenCalledTimes(2))
    h.controller.setUnreadOnly(true); await vi.waitFor(() => expect(h.controller.read().phase).toBe('ready')); expect(h.controller.read().rows).toHaveLength(0)
    blocked.resolve(Response.json(page())); await old; expect(h.controller.read()).toMatchObject({ unreadOnly: true, rows: [], phase: 'ready' })
  })
})
describe('server-confirmed read commands', () => {
  it.each(['old-revision', 'unread'] as const)('rejects a current row contradicting recovered success: %s', async mode => {
    let key = '', recovered = false
    const h = own(r => {
      if (r.method === 'PATCH') { key = r.headers.get('Idempotency-Key')!; return Response.json(pending('markConcertNotification', key), { status: 202 }) }
      if (new URL(r.url).pathname.endsWith('/commands')) { recovered = true; return Response.json(pending('markConcertNotification', key, { state: 'SUCCEEDED', result_status: 200, resource_id: id, resource_revision: 2 })) }
      return Response.json(page([update(recovered ? { revision: mode === 'old-revision' ? 1 : 2, read_at: mode === 'unread' ? null : later } : {})]))
    })
    await ready(h); await h.controller.markRead(id)
    const observed: number[] = []; h.controller.subscribe(() => { if (recovered && h.controller.read().phase === 'ready') observed.push(h.controller.read().rows.length) })
    await h.controller.recover(); expect(observed).not.toContain(1)
    expect(h.controller.read()).toMatchObject({ rows: [], phase: 'error', failure: 'invalid', command: { phase: 'uncertain' } })
    await h.controller.markRead(id); expect(h.fetch.mock.calls.filter(([r]) => (r as Request).method === 'PATCH')).toHaveLength(1)
  })
  it.each(['failed-response', 'failed-receipt'] as const)('keeps %s mark-all locked across pagination and filters', async mode => {
    let key = ''
    const h = own(r => {
      const u = new URL(r.url)
      if (r.method === 'PATCH') { key = r.headers.get('Idempotency-Key')!; return mode === 'failed-response' ? error(409, 'VERSION_CONFLICT') : Response.json(pending('markAllConcertNotifications', key), { status: 202 }) }
      if (u.pathname.endsWith('/commands')) return Response.json(pending('markAllConcertNotifications', key, { state: 'FAILED', failure_reason: 'VERSION_CONFLICT' }))
      return Response.json(u.searchParams.has('cursor') ? page([update({ notification_id: uid(7) })]) : page([update()], 'older'))
    })
    await ready(h); await h.controller.markAllRead(); if (mode === 'failed-receipt') await h.controller.recover()
    await h.controller.load(true); await h.controller.markAllRead()
    h.controller.setUnreadOnly(true); await vi.waitFor(() => expect(h.controller.read().phase).toBe('ready')); await h.controller.markAllRead()
    expect(h.fetch.mock.calls.filter(([r]) => (r as Request).method === 'PATCH')).toHaveLength(1)
  })
  it('allows a new explicit mark-all only after previous confirmed success, for later updates', async () => {
    let counter = 3
    const h = own(r => r.method === 'PATCH' ? (++counter, Response.json({ read_count: 1, read_at: later })) : Response.json(page([update({ notification_id: uid(counter) })])))
    await ready(h); await h.controller.markAllRead(); await h.controller.markAllRead()
    expect(h.fetch.mock.calls.filter(([r]) => (r as Request).method === 'PATCH')).toHaveLength(2)
  })
  it.each(['direct', 'receipt'] as const)('fences an older refresh before %s command reread', async mode => {
    const delayed = deferred<Response>(), command = deferred<Response>(); let lists = 0, key = '', committed = false
    const h = own(r => {
      if (r.method === 'PATCH') { key = r.headers.get('Idempotency-Key')!; return mode === 'receipt' ? Response.json(pending('markConcertNotification', key), { status: 202 }) : command.promise }
      if (new URL(r.url).pathname.endsWith('/commands')) { committed = true; return Response.json(pending('markConcertNotification', key, { state: 'SUCCEEDED', resource_id: id, resource_revision: 2, result_status: 200 })) }
      lists++; if (lists === 2) return delayed.promise
      return Response.json(page([update(committed ? { read_at: later, revision: 2 } : {})]))
    })
    await ready(h); const marking = h.controller.markRead(id)
    await vi.waitFor(() => expect(h.fetch.mock.calls.filter(([r]) => (r as Request).method === 'PATCH')).toHaveLength(1))
    if (mode === 'receipt') await marking
    const refreshing = h.controller.load(); await vi.waitFor(() => expect(lists).toBe(2))
    if (mode === 'direct') { committed = true; command.resolve(Response.json(update({ read_at: later, revision: 2 }))); await marking }
    else await h.controller.recover()
    expect(h.controller.read()).toMatchObject({ phase: 'ready', unreadShown: 0, command: { phase: 'succeeded' } })
    delayed.resolve(Response.json(page())); await refreshing
    expect(h.controller.read()).toMatchObject({ phase: 'ready', unreadShown: 0 })
  })
  it('bounds cursor chains even if every page repeats a single notification', async () => {
    let cursor = 0
    const h = own(() => Response.json(page([update()], `cursor_${++cursor}`))); await ready(h)
    for (let i = 0; i < 10; i++) await h.controller.load(true)
    expect(h.controller.read()).toMatchObject({ atLimit: true, unreadShown: 1 }); expect(h.fetch).toHaveBeenCalledTimes(11)
    await h.controller.load(true); expect(h.fetch).toHaveBeenCalledTimes(11)
  })
  it('privacy change during a late read command clears its identity and receipt state', async () => {
    const late = deferred<Response>(); const h = own(r => r.method === 'PATCH' ? late.promise : Response.json(page()))
    await ready(h); const marking = h.controller.markRead(id)
    await vi.waitFor(() => expect(h.fetch).toHaveBeenCalledTimes(2)); h.auth.session.replace(1, null)
    expect(h.controller.read()).toMatchObject({ rows: [], command: { phase: 'idle' }, scope: null })
    await marking; late.resolve(Response.json(update({ read_at: later, revision: 2 }))); await Promise.resolve()
    expect(h.controller.read()).toMatchObject({ rows: [], command: { phase: 'idle' }, scope: null })
  })
  it('sends one explicit mark command with canonical key and revision, never optimistic read state', async () => {
    const blocked = deferred<Response>(); let stored = update()
    const h = own(async r => {
      if (r.method === 'PATCH') {
        expect(await r.json()).toEqual({ expected_revision: 1 }); expect(r.headers.get('Idempotency-Key')).toBe(uid(100)); expect(r.headers.get('X-MCR-Intent')).toBe('browser')
        const response = await blocked.promise; stored = update({ read_at: later, revision: 2 }); return response
      }
      return Response.json(page([stored]))
    })
    await ready(h); const command = h.controller.markRead(id); await h.controller.markRead(id)
    expect(h.controller.read().unreadShown).toBe(1); blocked.resolve(Response.json(update({ read_at: later, revision: 2 }))); await command
    expect(h.controller.read()).toMatchObject({ unreadShown: 0, command: { phase: 'succeeded' } }); expect(h.fetch.mock.calls.filter(([r]) => (r as Request).method === 'PATCH')).toHaveLength(1)
  })
  it('read-all honors the server cutoff and reloads newly arriving unread notifications', async () => {
    let marked = false
    const h = own(async r => {
      if (r.method === 'PATCH') { expect(new URL(r.url).pathname.endsWith('/read-all')).toBe(true); expect(await r.json()).toEqual({}); marked = true; return Response.json({ read_count: 1, read_at: later }) }
      return Response.json(page(marked ? [update({ read_at: later, revision: 2 }), update({ notification_id: uid(9), created_at: '2026-11-04T00:02:00Z' })] : [update()]))
    })
    await ready(h); await h.controller.markAllRead(); expect(h.controller.read().unreadShown).toBe(1)
    expect(h.controller.read().rows.find(n => n.notification_id === uid(9))?.read_at).toBeNull()
  })
  it.each(['network', 'pending', 'malformed', 'failed'] as const)('handles %s command without false success or replay', async mode => {
    const h = own(r => {
      if (r.method === 'PATCH') {
        if (mode === 'network') throw new TypeError('private transport detail')
        if (mode === 'failed') return error(409, 'VERSION_CONFLICT')
        if (mode === 'malformed') return Response.json({ ...update(), read_at: null })
        return Response.json(pending('markConcertNotification', r.headers.get('Idempotency-Key')!), { status: 202 })
      }
      return Response.json(page())
    })
    await ready(h); await h.controller.markRead(id); expect(h.controller.read().command.phase).toBe(mode === 'failed' ? 'failed' : 'uncertain')
    expect(h.controller.read().unreadShown).toBe(1); await h.controller.markRead(id); await h.controller.markAllRead()
    // Even a definitive failure does not authorize a new key for the identical mark.
    expect(h.fetch.mock.calls.filter(([r]) => (r as Request).method === 'PATCH')).toHaveLength(mode === 'failed' ? 2 : 1)
  })
  it('successful receipt still rereads current authorized list; missing page row is not fabricated', async () => {
    let key = '', recovered = false
    const h = own(r => {
      const u = new URL(r.url)
      if (r.method === 'PATCH') { key = r.headers.get('Idempotency-Key')!; return Response.json(pending('markConcertNotification', key), { status: 202 }) }
      if (u.pathname.endsWith('/commands')) { expect(u.searchParams.get('idempotency_key')).toBe(key); recovered = true; return Response.json(pending('markConcertNotification', key, { state: 'SUCCEEDED', resource_id: id, resource_revision: 2, result_status: 200 })) }
      return Response.json(recovered ? page([], 'other_page') : page())
    })
    await ready(h); await h.controller.markRead(id); await h.controller.recover(); expect(h.controller.read()).toMatchObject({ command: { phase: 'succeeded' }, rows: [], nextCursor: 'other_page' })
    expect(h.fetch.mock.calls.filter(([r]) => (r as Request).method === 'PATCH')).toHaveLength(1)
  })
  it.each(['404', 'foreign', 'wrong-key', 'wrong-operation', 'wrong-result', 'retired', 'expired', 'failure'] as const)('does not infer replay/success from %s receipt', async mode => {
    let key = ''
    const h = own(r => {
      if (r.method === 'PATCH') { key = r.headers.get('Idempotency-Key')!; return Response.json(pending('markConcertNotification', key), { status: 202 }) }
      if (new URL(r.url).pathname.endsWith('/commands')) {
        if (mode === '404') return error(404, 'RESOURCE_UNAVAILABLE')
        return Response.json(pending('markConcertNotification', key, mode === 'retired' ? { state: 'RETIRED' } : mode === 'expired' ? { created_at: '2026-11-03T00:00:00Z', expires_at: '2026-11-03T00:01:00Z' } : mode === 'failure' ? { state: 'FAILED', failure_reason: 'VERSION_CONFLICT' }
          : { state: 'SUCCEEDED', resource_id: mode === 'foreign' ? uid(99) : id, resource_revision: 2, result_status: mode === 'wrong-result' ? 202 : 200,
            ...(mode === 'wrong-key' ? { idempotency_key: uid(99) } : mode === 'wrong-operation' ? { operation_id: 'markAllConcertNotifications' } : {}) }))
      }
      return Response.json(page())
    })
    await ready(h); await h.controller.markRead(id); await h.controller.recover(); expect(h.controller.read().command.phase).toBe(mode === 'retired' || mode === 'expired' ? 'retired' : mode === 'failure' ? 'failed' : 'uncertain')
    await h.controller.markRead(id); expect(h.fetch.mock.calls.filter(([r]) => (r as Request).method === 'PATCH')).toHaveLength(1)
  })
})
