import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { bytes, deferred, id, mediaId, metadata, setup } from '../fixtures'
const cleanups: (() => void)[] = []
function fixture(...args: Parameters<typeof setup>) { const h = setup(...args); cleanups.push(h.cleanup); return h }
beforeEach(() => vi.stubGlobal('location', { origin: 'https://mcr.example' }))
afterEach(() => { cleanups.splice(0).forEach(dispose => dispose()); vi.unstubAllGlobals() })
async function retain(h: ReturnType<typeof setup>, media = mediaId) {
  const lease = h.owner.acquire({ mediaId: media, purpose: 'POST' }); await lease.load()
  const state = lease.read(); lease.release(); return state
}
it('never creates an entry or a request for cold, invalid or already-aborted cache probes', () => {
  const h = fixture(), abort = new AbortController(); abort.abort()
  expect(h.owner.acquireCached({ mediaId, purpose: 'POST' })).toBeNull()
  expect(h.owner.acquireCached({ mediaId: 'https://untrusted.test/image', purpose: 'POST' })).toBeNull()
  expect(h.owner.acquireCached({ mediaId, purpose: 'POST', signal: abort.signal })).toBeNull()
  expect(h.owner.inspect()).toMatchObject({ entries: 0, consumers: 0 })
  expect(h.fetch).not.toHaveBeenCalled(); expect(h.previews.create).not.toHaveBeenCalled()
})
it('attaches to the same immutable ready state and its owning URL without loading or copying either', async () => {
  const h = fixture(), state = await retain(h), cached = h.owner.acquireCached({ mediaId, purpose: 'POST' })!
  expect(cached.read()).toBe(state); expect(h.owner.inspect()).toMatchObject({ consumers: 1, activeEntries: 1 })
  await cached.load(); await cached.load()
  expect(cached.read()).toBe(state); expect(h.fetch).toHaveBeenCalledTimes(2); expect(h.previews.create).toHaveBeenCalledTimes(1)
  cached.release(); cached.dispose(); await cached.load()
  expect(cached.read()).toMatchObject({ phase: 'cancelled', previewUrl: null })
  expect(h.owner.inspect()).toMatchObject({ entries: 1, consumers: 0 }); expect(h.previews.revoke).not.toHaveBeenCalled()
  h.owner.dispose(); expect(h.previews.revoke).toHaveBeenCalledExactlyOnceWith(state.previewUrl)
})
it('returns null for an in-flight entry without joining or promoting it, then can attach after explicit completion', async () => {
  const gate = deferred<Response>(), h = fixture({}, r => new URL(r.url).pathname.endsWith('/content') ? gate.promise : Response.json(metadata()))
  const reader = h.owner.acquire({ mediaId, purpose: 'POST' }), pending = reader.load()
  await vi.waitFor(() => expect(h.fetch).toHaveBeenCalledTimes(2))
  expect(h.owner.acquireCached({ mediaId, purpose: 'POST' })).toBeNull(); expect(h.owner.inspect().consumers).toBe(1)
  gate.resolve(new Response(bytes(), { headers: { 'Content-Type': 'image/png' } })); await pending
  const cached = h.owner.acquireCached({ mediaId, purpose: 'POST' })!
  expect(cached.read()).toBe(reader.read()); await cached.load(); expect(h.fetch).toHaveBeenCalledTimes(2)
})
it.each(['unavailable', 'invalid-metadata'] as const)('does not retry or expose a %s entry', async failure => {
  const h = fixture({}, () => failure === 'unavailable' ? new Response(null, { status: 503 }) : Response.json(metadata(id(99))))
  const reader = h.owner.acquire({ mediaId, purpose: 'POST' }); await reader.load()
  expect(reader.read().phase).toBe('failed')
  expect(h.owner.acquireCached({ mediaId, purpose: 'POST' })).toBeNull(); expect(h.fetch).toHaveBeenCalledTimes(1)
})
it('does not admit another media ID or purpose, even when a GIVEAWAY entry is ready', async () => {
  const h = fixture({}, r => new URL(r.url).pathname.endsWith('/content') ? new Response(bytes(), { headers: { 'Content-Type': 'image/png' } }) : Response.json(metadata(mediaId, 'GIVEAWAY')))
  const gift = h.owner.acquire({ mediaId, purpose: 'GIVEAWAY' }); await gift.load(); expect(gift.read().phase).toBe('ready')
  expect(h.owner.acquireCached({ mediaId, purpose: 'GIVEAWAY' })).toBeNull()
  expect(h.owner.acquireCached({ mediaId, purpose: 'POST' })).toBeNull()
  expect(h.owner.acquireCached({ mediaId: id(4), purpose: 'POST' })).toBeNull(); expect(h.fetch).toHaveBeenCalledTimes(2)
})
it('pins a ready reader against eviction until the cached consumer releases it', async () => {
  const h = fixture({ maxEntries: 1 }), state = await retain(h), cached = h.owner.acquireCached({ mediaId, purpose: 'POST' })!
  const next = h.owner.acquire({ mediaId: id(4), purpose: 'POST' }); await next.load()
  expect(next.read()).toMatchObject({ phase: 'failed', error: 'media-capacity' })
  expect(cached.read()).toBe(state); expect(h.previews.revoke).not.toHaveBeenCalled(); expect(h.fetch).toHaveBeenCalledTimes(2)
  cached.release(); await next.load()
  expect(next.read().phase).toBe('ready'); expect(h.previews.revoke).toHaveBeenCalledExactlyOnceWith(state.previewUrl)
  await cached.load(); expect(cached.read().previewUrl).toBeNull(); expect(h.fetch).toHaveBeenCalledTimes(4)
})
it('enforces consumer capacity and abort without evicting another ready consumer', async () => {
  const h = fixture({ maxConsumers: 1 }), reader = h.owner.acquire({ mediaId, purpose: 'POST' }); await reader.load()
  expect(h.owner.acquireCached({ mediaId, purpose: 'POST' })).toBeNull()
  reader.release(); const abort = new AbortController(), cached = h.owner.acquireCached({ mediaId, purpose: 'POST', signal: abort.signal })!
  abort.abort(); await cached.load(); expect(cached.read().previewUrl).toBeNull()
  expect(h.owner.inspect().consumers).toBe(0); expect(h.previews.revoke).not.toHaveBeenCalled(); expect(h.fetch).toHaveBeenCalledTimes(2)
})
it.each(['block', 'consent-revoked', 'suspended', 'event-closed', 'revalidated'] as const)('clears attached cached media synchronously on %s', async reason => {
  const h = fixture(), state = await retain(h), cached = h.owner.acquireCached({ mediaId, purpose: 'POST' })!, notify = vi.fn()
  cached.subscribe(notify); h.privacy.changePolicy(h.scope, reason)
  expect(cached.read()).toMatchObject({ phase: 'cancelled', previewUrl: null }); expect(notify).toHaveBeenCalled()
  expect(h.previews.revoke).toHaveBeenCalledExactlyOnceWith(state.previewUrl)
  expect(h.owner.acquireCached({ mediaId, purpose: 'POST' })).toBeNull(); await cached.load(); expect(h.fetch).toHaveBeenCalledTimes(2)
})
it.each(['identity', 'event', 'generation', 'signout'] as const)('cannot reuse a prior URL after %s changes', async change => {
  const h = fixture(); await retain(h); const cached = h.owner.acquireCached({ mediaId, purpose: 'POST' })!
  h.session.replace(h.session.read().generation, change === 'signout' ? null : { userId: change === 'identity' ? id(80) : id(1), eventId: change === 'event' ? id(81) : id(2), accessToken: 'new-memory' })
  expect(cached.read().previewUrl).toBeNull(); expect(h.owner.acquireCached({ mediaId, purpose: 'POST' })).toBeNull()
  await cached.load(); expect(h.fetch).toHaveBeenCalledTimes(2)
})
