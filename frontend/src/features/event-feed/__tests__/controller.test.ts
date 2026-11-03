import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { deferred, eventId, id, notice, post, setup } from './fixtures'
import { parse } from '../contract'
const cleanup: (() => void)[] = []
beforeEach(() => vi.stubGlobal('location', { origin: 'https://mcr.example' }))
afterEach(() => { cleanup.splice(0).forEach(fn => fn()); vi.unstubAllGlobals() })
function fixture(handler: (r: Request) => Response | Promise<Response>) {
  const fetch = vi.fn<typeof globalThis.fetch>(async input => handler(input as Request)), s = setup(fetch); cleanup.push(s.dispose); return { ...s, fetch }
}
const empty = () => Response.json({ items: [], next_cursor: null })
it('sends canonical protected feed/filter/cursor reads and appends stable pages', async () => {
  const s = fixture(r => {
    expect(r.headers.get('Authorization')).toBe('Bearer test-only-token'); expect(r.cache).toBe('no-store')
    const url = new URL(r.url)
    if (url.pathname.endsWith('/notices')) return empty()
    expect(url.searchParams.get('filter')).toBe('ALL')
    return Response.json(url.searchParams.has('cursor') ? { items: [{ ...post, post_id: id(2), created_at: '2020-01-01T00:00:00Z' }], next_cursor: null } : { items: [post], next_cursor: 'older' })
  })
  s.controller.start(); await vi.waitFor(() => expect(s.controller.read().feed.phase).toBe('ready'))
  await s.controller.loadFeed(true)
  expect(s.controller.read().feed.data?.items.map(p => p.post_id)).toEqual([post.post_id, id(2)])
})
it.each(['private', 'cross-event', 'duplicate', 'out-of-order', 'cursor-cycle'])('rejects %s without exposing previous list', async kind => {
  let call = 0
  const s = fixture(r => {
    if (r.url.includes('/notices')) return empty()
    call++
    let items: unknown[] = [post], next_cursor: string | null = null
    if (kind === 'private') items = [{ ...post, private_handle: 'secret' }]
    if (kind === 'cross-event') items = [{ ...post, event_id: id(999) }]
    if (kind === 'duplicate') items = [post, post]
    if (kind === 'out-of-order') items = [{ ...post, created_at: '2020-01-01T00:00:00Z' }, { ...post, post_id: id(3) }]
    if (kind === 'cursor-cycle') { items = call === 1 ? [post] : [{ ...post, post_id: id(3), created_at: '2020-01-01T00:00:00Z' }]; next_cursor = 'cycle' }
    return Response.json({ items, next_cursor })
  })
  s.controller.start()
  if (kind === 'cursor-cycle') { await vi.waitFor(() => expect(s.controller.read().feed.phase).toBe('ready')); await s.controller.loadFeed(true) }
  await vi.waitFor(() => expect(s.controller.read().feed.phase).toBe('error'))
  expect(s.controller.read().feed.data).toBeNull()
})
it('evicts synchronously on policy change and rejects late former-identity reads', async () => {
  const late = deferred<Response>()
  const s = fixture(r => r.url.includes('/notices') ? empty() : late.promise)
  s.controller.start(); await vi.waitFor(() => expect(s.fetch).toHaveBeenCalledTimes(2))
  s.auth.session.replace(s.auth.session.read().generation, null)
  expect(s.controller.read().feed.data).toBeNull(); expect(s.controller.read().available).toBe(false)
  late.resolve(Response.json({ items: [post], next_cursor: null }))
  await Promise.resolve(); expect(s.controller.read().feed.data).toBeNull()
})
it('withdrawn notice is removed from list, with safe error and no raw server prose', async () => {
  const s = fixture(r => r.url.endsWith('/notices') ? Response.json({ items: [notice], next_cursor: null }) : r.url.includes('/notices/') ? Response.json({ message: 'private raw detail' }, { status: 404 }) : empty())
  s.controller.start(); await vi.waitFor(() => expect(s.controller.read().notices.phase).toBe('ready'))
  await s.controller.selectNotice(notice.notice_id)
  expect(s.controller.read().notices.data?.items).toEqual([]); expect(s.controller.read().notice.failure).toBe('unavailable')
})
it('removes revoked linked target and read-only capabilities update immediately', async () => {
  const linked = { ...post, links: [{ target_type: 'PARTY' as const, target_id: id(80) }] }
  const s = fixture(r => r.url.includes('/notices') ? empty() : Response.json({ items: [linked], next_cursor: null }))
  s.controller.start(); await vi.waitFor(() => expect(s.controller.read().feed.phase).toBe('ready'))
  for (const link of linked.links) s.controller.removeLink(link)
  expect(s.controller.read().feed.data?.items[0]?.links).toEqual([])
  s.setAccess({ available: true, readOnly: true }); expect(s.controller.read().readOnly).toBe(true)
})
it('validates source HTTPS and exact public profile instead of accepting leaked fields', () => {
  expect(() => parse('Domain_feed_PublishedNotice', { ...notice, source_uri: 'javascript:alert(1)' })).toThrow()
  expect(() => parse('Domain_feed_PostPublic', { ...post, author: { ...post.author, handles: 'private' } })).toThrow()
  expect(eventId).toBe(post.event_id)
})
it('orders submillisecond timestamps and equivalent UTC precision without losing cursor ties', async () => {
  const { ordered } = await import('../contract')
  expect(ordered([{ created_at: '2026-11-01T00:00:00.000002Z', id: 'a' }, { created_at: '2026-11-01T00:00:00.000001Z', id: 'z' }], row => row.id)).toBe(true)
  expect(ordered([{ created_at: '2026-11-01T00:00:00Z', id: 'z' }, { created_at: '2026-11-01T00:00:00.000Z', id: 'a' }], row => row.id)).toBe(true)
})

it.each(['pagination', 'refresh'] as const)('never restores destination-denied links from in-flight %s', async mode => {
  const target = { target_type: 'PARTY' as const, target_id: id(80) }, linked = { ...post, links: [target] }, late = deferred<Response>()
  let calls = 0
  const s = fixture(r => {
    if (r.url.includes('/notices')) return empty()
    if (++calls === 1) return Response.json({ items: [linked], next_cursor: 'older' })
    return late.promise
  })
  s.controller.start(); await vi.waitFor(() => expect(s.controller.read().feed.phase).toBe('ready'))
  expect(s.controller.read().feed.data?.items[0]?.links).toEqual([target])
  const flight = s.controller.loadFeed(mode === 'pagination')
  await vi.waitFor(() => expect(calls).toBe(2)); s.controller.removeLink(target)
  late.resolve(Response.json({ items: mode === 'pagination' ? [{ ...linked, post_id: id(3), created_at: '2020-01-01T00:00:00Z' }] : [linked], next_cursor: null }))
  await flight
  expect(s.controller.read().feed.phase).toBe('ready')
  expect(s.controller.read().feed.data?.items.flatMap(row => row.links)).toEqual([])
})

it.each(['pagination', 'refresh'] as const)('never restores withdrawn notice fields from in-flight %s', async mode => {
  const late = deferred<Response>(); let noticeCalls = 0
  const s = fixture(r => {
    if (r.url.endsWith(`/notices/${notice.notice_id}`)) return Response.json({}, { status: 404 })
    if (r.url.includes('/notices')) return ++noticeCalls === 1 ? Response.json({ items: [notice], next_cursor: 'older-notices' }) : late.promise
    return empty()
  })
  s.controller.start(); await vi.waitFor(() => expect(s.controller.read().notices.phase).toBe('ready'))
  const flight = s.controller.loadNotices(mode === 'pagination')
  await vi.waitFor(() => expect(noticeCalls).toBe(2)); await s.controller.selectNotice(notice.notice_id)
  expect(s.controller.read().notices.data?.items ?? []).not.toContainEqual(notice)
  late.resolve(Response.json({ items: mode === 'pagination' ? [{ ...notice, notice_id: id(61) }] : [notice], next_cursor: null }))
  await flight
  expect(s.controller.read().notices.phase).toBe('ready')
  expect(s.controller.read().notices.data?.items.some(row => row.notice_id === notice.notice_id)).toBe(false)
})
