import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import type { MediaPreviewPort } from '../../../features/event-feed/FeedImage'
import { createMediaOwner } from '../index'
import { bytes, deferred, id, mediaId, metadata, setup } from './fixtures'
const cleanups: (() => void)[] = []
function harness(...args: Parameters<typeof setup>) { const h = setup(...args); cleanups.push(h.cleanup); return h }
beforeEach(() => vi.stubGlobal('location', { origin: 'https://mcr.example' }))
afterEach(() => { cleanups.splice(0).forEach(cleanup => cleanup()); vi.unstubAllGlobals() })
it('does no read on acquire; shares a cold read and retains ready URL across route lifetimes', async () => {
  const h = harness(), a = h.owner.acquire({ mediaId, purpose: 'POST' }), b = h.owner.acquire({ mediaId, purpose: 'POST' })
  const compatible: MediaPreviewPort = a
  expect(compatible.read().phase).toBe('idle'); expect(h.fetch).not.toHaveBeenCalled()
  await Promise.all([a.load(), b.load()]); const url = a.read().previewUrl
  expect(b.read().previewUrl).toBe(url); expect(h.fetch).toHaveBeenCalledTimes(2); expect(h.previews.create).toHaveBeenCalledTimes(1)
  a.dispose(); a.release(); expect(h.previews.revoke).not.toHaveBeenCalled(); expect(b.read().phase).toBe('ready')
  b.release(); expect(h.owner.inspect()).toMatchObject({ entries: 1, consumers: 0, activeEntries: 0 })
  const warm = h.owner.acquire({ mediaId, purpose: 'POST' }); expect(warm.read().previewUrl).toBeNull()
  await warm.load(); expect(warm.read().previewUrl).toBe(url); expect(h.fetch).toHaveBeenCalledTimes(2)
  for (const [r] of h.fetch.mock.calls) { expect((r as Request).headers.get('Authorization')).toBe('Bearer memory'); expect((r as Request).cache).toBe('no-store') }
  h.owner.dispose(); expect(h.previews.revoke).toHaveBeenCalledExactlyOnceWith(url); expect(warm.read().previewUrl).toBeNull()
})
it('caller abort settles its lease without aborting another consumer or its shared request', async () => {
  const gate = deferred<Response>(), h = harness({}, r => new URL(r.url).pathname.endsWith('/content') ? gate.promise : Response.json(metadata()))
  const abort = new AbortController(), a = h.owner.acquire({ mediaId, purpose: 'POST', signal: abort.signal }), b = h.owner.acquire({ mediaId, purpose: 'POST' })
  const first = a.load(), second = b.load(); await vi.waitFor(() => expect(h.fetch).toHaveBeenCalledTimes(2))
  abort.abort(); await first
  expect(a.read().phase).toBe('cancelled'); expect(b.read().phase).toBe('loading'); expect((h.fetch.mock.calls[1]![0] as Request).signal.aborted).toBe(false)
  gate.resolve(new Response(bytes(), { headers: { 'Content-Type': 'image/png' } })); await second
  expect(b.read().phase).toBe('ready'); expect(h.previews.create).toHaveBeenCalledTimes(1); expect(h.owner.inspect().consumers).toBe(1)
})
it('last release aborts pending bytes, releases capacity and fences ignored late completion', async () => {
  const gate = deferred<Response>(), h = harness({ maxEntries: 1 }, r => new URL(r.url).pathname.endsWith('/content') ? gate.promise : Response.json(metadata()))
  const lease = h.owner.acquire({ mediaId, purpose: 'POST' }), pending = lease.load()
  await vi.waitFor(() => expect(h.fetch).toHaveBeenCalledTimes(2)); lease.release(); await pending
  expect((h.fetch.mock.calls[1]![0] as Request).signal.aborted).toBe(true); expect(h.owner.inspect().entries).toBe(0)
  gate.resolve(new Response(bytes(), { headers: { 'Content-Type': 'image/png' } })); await new Promise(resolve => setTimeout(resolve, 0))
  expect(h.previews.create).not.toHaveBeenCalled(); expect(lease.read()).toMatchObject({ phase: 'cancelled', previewUrl: null, media: null })
})
it('can retry a failed load in the same live lease', async () => {
  let fail = true
  const h = harness({}, r => new URL(r.url).pathname.endsWith('/content') ? new Response(bytes(), { headers: { 'Content-Type': 'image/png' } }) : fail ? new Response(null, { status: 503 }) : Response.json(metadata()))
  const lease = h.owner.acquire({ mediaId, purpose: 'POST' }); await lease.load(); expect(lease.read().phase).toBe('failed')
  fail = false; await lease.load(); expect(lease.read().phase).toBe('ready'); expect(h.fetch).toHaveBeenCalledTimes(3)
})
it('does not reuse POST data under GIVEAWAY, including the same media ID', async () => {
  const h = harness(); const post = h.owner.acquire({ mediaId, purpose: 'POST' }); await post.load(); post.release()
  const gift = h.owner.acquire({ mediaId, purpose: 'GIVEAWAY' }); await gift.load()
  expect(gift.read().phase).toBe('failed'); expect(h.fetch).toHaveBeenCalledTimes(3); expect(h.previews.create).toHaveBeenCalledTimes(1)
  h.owner.dispose()
})
it('accepts a real GIVEAWAY metadata contract through the same verified byte reader', async () => {
  const h = harness({}, r => new URL(r.url).pathname.endsWith('/content') ? new Response(bytes(), { headers: { 'Content-Type': 'image/png' } }) : Response.json(metadata(mediaId, 'GIVEAWAY')))
  const gift = h.owner.acquire({ mediaId, purpose: 'GIVEAWAY' }); await gift.load()
  expect(gift.read()).toMatchObject({ phase: 'ready', media: { purpose: 'GIVEAWAY', media_id: mediaId } }); expect(h.fetch).toHaveBeenCalledTimes(2)
})
it('evicts least recently used retained entry, revokes once and never evicts held entries', async () => {
  const h = harness({ maxEntries: 2 })
  const a = h.owner.acquire({ mediaId: id(3), purpose: 'POST' }); await a.load(); const old = a.read().previewUrl; a.release()
  const b = h.owner.acquire({ mediaId: id(4), purpose: 'POST' }); await b.load(); const held = b.read().previewUrl
  const c = h.owner.acquire({ mediaId: id(5), purpose: 'POST' }); await c.load()
  expect(h.previews.revoke).toHaveBeenCalledExactlyOnceWith(old); expect(b.read().previewUrl).toBe(held)
  expect(h.owner.inspect()).toMatchObject({ entries: 2, activeEntries: 2, maxResidentBlobBytes: 20 * 1024 * 1024 })
  const blocked = h.owner.acquire({ mediaId: id(6), purpose: 'POST' }); await blocked.load()
  expect(blocked.read()).toMatchObject({ phase: 'failed', error: 'media-capacity' }); expect(h.fetch).toHaveBeenCalledTimes(6)
  b.release(); await blocked.load(); expect(blocked.read().phase).toBe('ready'); expect(h.fetch).toHaveBeenCalledTimes(8)
})
it('a warm touch refreshes LRU ordering and an idempotent release does not decrement others', async () => {
  const h = harness({ maxEntries: 2 })
  async function visit(media: string) { const lease = h.owner.acquire({ mediaId: media, purpose: 'POST' }); await lease.load(); const url = lease.read().previewUrl; lease.release(); lease.dispose(); return url }
  const a = await visit(id(3)), b = await visit(id(4)); expect(await visit(id(3))).toBe(a); await visit(id(5))
  expect(h.previews.revoke).toHaveBeenCalledExactlyOnceWith(b); expect(h.owner.inspect().consumers).toBe(0)
})
it.each(['block', 'consent-revoked', 'suspended', 'event-closed', 'revalidated'] as const)('synchronously removes retained and visible private URLs on %s policy change', async reason => {
  const h = harness(); const a = h.owner.acquire({ mediaId, purpose: 'POST' }), b = h.owner.acquire({ mediaId: id(4), purpose: 'POST' })
  await a.load(); await b.load(); a.release()
  b.subscribe(() => { throw new Error('faulty observer') })
  expect(h.privacy.changePolicy(h.scope, reason)).toBe(true)
  expect(h.owner.inspect()).toMatchObject({ disposed: true, entries: 0, consumers: 0 }); expect(h.previews.revoke).toHaveBeenCalledTimes(2)
  expect(b.read()).toMatchObject({ phase: 'cancelled', previewUrl: null, media: null, scope: null }); expect(() => h.owner.acquire({ mediaId, purpose: 'POST' })).toThrow()
  expect(() => createMediaOwner({ auth: h.auth, scope: h.scope })).toThrow()
})
it.each(['logout', 'account', 'event'] as const)('identity %s change clears current state and prevents cross-identity reuse', async mode => {
  const h = harness(), lease = h.owner.acquire({ mediaId, purpose: 'POST' }); await lease.load()
  h.session.replace(1, mode === 'logout' ? null : { userId: id(mode === 'account' ? 9 : 1), eventId: id(mode === 'event' ? 9 : 2), accessToken: 'next' })
  expect(h.owner.inspect().disposed).toBe(true); expect(lease.read().previewUrl).toBeNull(); expect(h.previews.revoke).toHaveBeenCalledTimes(1)
})
it('policy cancellation during a metadata response prevents any subsequent byte read', async () => {
  const gate = deferred<Response>(), h = harness({}, () => gate.promise), lease = h.owner.acquire({ mediaId, purpose: 'POST' })
  const pending = lease.load(); await vi.waitFor(() => expect(h.fetch).toHaveBeenCalledTimes(1)); h.privacy.changePolicy(h.scope, 'block'); await pending
  gate.resolve(Response.json(metadata())); await new Promise(resolve => setTimeout(resolve, 0))
  expect(h.fetch).toHaveBeenCalledTimes(1); expect(h.previews.create).not.toHaveBeenCalled(); expect(lease.read().previewUrl).toBeNull()
})
it('cancel can restart a consumer; pre-aborted and disposed facades never load', async () => {
  const h = harness(), abort = new AbortController(); abort.abort()
  const stopped = h.owner.acquire({ mediaId, purpose: 'POST', signal: abort.signal }); await stopped.load(); expect(h.fetch).not.toHaveBeenCalled()
  const live = h.owner.acquire({ mediaId, purpose: 'POST' }); await live.load(); const url = live.read().previewUrl; live.cancel()
  expect(live.read().previewUrl).toBeNull(); await live.load(); expect(live.read().previewUrl).toBe(url); expect(h.fetch).toHaveBeenCalledTimes(2)
  live.dispose(); await live.load(); expect(live.read().previewUrl).toBeNull()
})
it('bounds facade registrations and rejects invalid/stale/non-open acquisition without reads', () => {
  const h = harness({ maxConsumers: 1 }); const first = h.owner.acquire({ mediaId, purpose: 'POST' })
  expect(() => h.owner.acquire({ mediaId, purpose: 'POST' })).toThrow(RangeError); first.release()
  expect(() => h.owner.acquire({ mediaId: 'guessed', purpose: 'POST' })).toThrow()
  expect(() => createMediaOwner({ auth: h.auth, scope: h.scope, maxEntries: 0 })).toThrow(RangeError)
  expect(() => createMediaOwner({ auth: h.auth, scope: h.scope, maxEntries: 33 })).toThrow(RangeError)
  h.privacy.changePolicy(h.scope, 'suspended'); expect(() => createMediaOwner({ auth: h.auth, scope: h.privacy.capture() })).toThrow()
  expect(h.fetch).not.toHaveBeenCalled()
})
it('coalesces reentrant subscriber loads and lets synchronous observers release safely', async () => {
  const h = harness(), lease = h.owner.acquire({ mediaId, purpose: 'POST' })
  lease.subscribe(() => { void lease.load() })
  await lease.load(); expect(lease.read().phase).toBe('ready'); expect(h.fetch).toHaveBeenCalledTimes(2)
  lease.release()
  const next = h.owner.acquire({ mediaId: id(4), purpose: 'POST' })
  next.subscribe(() => { if (next.read().phase === 'loading') next.release() })
  await next.load(); expect(next.read().phase).toBe('cancelled'); expect(h.owner.inspect().activeEntries).toBe(0)
})
it('a new policy owner performs fresh reads even when session identity and media ID are unchanged', async () => {
  const h = harness(), old = h.owner.acquire({ mediaId, purpose: 'POST' }); await old.load(); const oldUrl = old.read().previewUrl
  h.privacy.changePolicy(h.scope, 'block')
  const owner = createMediaOwner({ auth: h.auth, scope: h.privacy.capture(), previews: h.previews }); cleanups.push(owner.dispose)
  const fresh = owner.acquire({ mediaId, purpose: 'POST' }); await fresh.load()
  expect(fresh.read().phase).toBe('ready'); expect(fresh.read().previewUrl).not.toBe(oldUrl); expect(h.fetch).toHaveBeenCalledTimes(4)
})
