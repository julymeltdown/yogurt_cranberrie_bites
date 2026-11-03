import { afterEach, describe, expect, it, vi } from 'vitest'
import { webcrypto } from 'node:crypto'
import { createResourceByteCache } from '../resource-cache'
import { createAssetResources, type Resource } from '../resources'
import { sha256 } from '../json'

afterEach(() => { vi.unstubAllGlobals() })
const bytes = new TextEncoder().encode('{"static":"fixture"}')
async function setup() {
  vi.stubGlobal('crypto', webcrypto)
  const resource: Resource = { path: 'world/maps/cache-test.json', sha256: await sha256(bytes), kind: 'json' }
  const cache = createResourceByteCache()
  const response = (body = bytes, origin = 'https://example.test') => {
    const value = new Response(body, { headers: { 'Content-Type': 'application/json' } })
    Object.defineProperty(value, 'url', { value: `${origin}/${resource.path}` }); return value
  }
  return { cache, resource, response, signal: new AbortController().signal }
}
describe('release-scoped verified resource cache', () => {
  it('coalesces overlapping reads, reuses warm bytes, isolates caller mutation, and rechecks every allowlist', async () => {
    const h = await setup(), fetcher = vi.fn(async () => h.response())
    const reader = createAssetResources({ origin: 'https://example.test', allow: [h.resource], cache: h.cache, fetch: fetcher })
    const [first, second] = await Promise.all([reader.read(h.resource, h.signal), reader.read(h.resource, h.signal)])
    first[0] = 0
    expect(second).toEqual(bytes); expect(await reader.read(h.resource, h.signal)).toEqual(bytes)
    const denied = createAssetResources({ origin: 'https://example.test', allow: [], cache: h.cache, fetch: fetcher })
    await expect(denied.read(h.resource, h.signal)).rejects.toThrow()
    await expect(reader.read({ ...h.resource, sha256: `sha256:${'f'.repeat(64)}` }, h.signal)).rejects.toThrow()
    expect(fetcher).toHaveBeenCalledTimes(1)
    expect(h.cache.diagnostics()).toMatchObject({ loads: 1, coalesced: 1, hits: 1, residentBytes: bytes.length })
    h.cache.dispose()
  })
  it('does not cache hash failures and permits a clean explicit retry', async () => {
    const h = await setup(), fetcher = vi.fn().mockResolvedValueOnce(h.response(new TextEncoder().encode('corrupt'))).mockResolvedValueOnce(h.response())
    const reader = createAssetResources({ origin: 'https://example.test', allow: [h.resource], cache: h.cache, fetch: fetcher })
    await expect(reader.read(h.resource, h.signal)).rejects.toThrow()
    expect(h.cache.diagnostics().residentBytes).toBe(0)
    expect(await reader.read(h.resource, h.signal)).toEqual(bytes); expect(fetcher).toHaveBeenCalledTimes(2)
    h.cache.dispose()
  })
  it('one consumer cancellation preserves the other read; owner teardown aborts loads and rejects late bytes', async () => {
    const h = await setup(), first = new AbortController(), second = new AbortController()
    let finish!: (value: Response) => void, underlying: AbortSignal | undefined
    const fetcher = vi.fn((_input: RequestInfo | URL, init?: RequestInit) => {
      underlying = init?.signal ?? undefined
      return new Promise<Response>(resolve => { finish = resolve })
    })
    const reader = createAssetResources({ origin: 'https://example.test', allow: [h.resource], cache: h.cache, fetch: fetcher })
    const one = reader.read(h.resource, first.signal), two = reader.read(h.resource, second.signal)
    const cancelled = expect(one).rejects.toThrow(); await Promise.resolve(); first.abort(); await cancelled
    expect(underlying?.aborted).toBe(false)
    finish(h.response()); expect(await two).toEqual(bytes); expect(fetcher).toHaveBeenCalledTimes(1)
    h.cache.dispose(); expect(h.cache.diagnostics()).toMatchObject({ residentBytes: 0, entries: 0, disposed: true })
    await expect(reader.read(h.resource, h.signal)).rejects.toThrow()

    const owner = new AbortController(), cache = createResourceByteCache({ signal: owner.signal })
    let late!: (value: Uint8Array<ArrayBuffer>) => void, loadSignal: AbortSignal | undefined
    const pending = cache.read('late', h.signal, signal => { loadSignal = signal; return new Promise(resolve => { late = resolve }) })
    const rejected = expect(pending).rejects.toThrow(); await Promise.resolve(); owner.abort(); late(bytes); await rejected
    expect(loadSignal?.aborted).toBe(true); expect(cache.diagnostics().residentBytes).toBe(0)
  })
  it('bounds resident bytes and keeps distinct origins and hashes separate', async () => {
    const h = await setup(), cache = createResourceByteCache({ maxBytes: bytes.length, maxEntries: 2 })
    const fetcher = vi.fn(async (input: RequestInfo | URL) => h.response(bytes, new URL(String(input)).origin))
    const first = createAssetResources({ origin: 'https://one.test', allow: [h.resource], cache, fetch: fetcher })
    const second = createAssetResources({ origin: 'https://two.test', allow: [h.resource], cache, fetch: fetcher })
    await first.read(h.resource, h.signal); await second.read(h.resource, h.signal)
    expect(fetcher).toHaveBeenCalledTimes(2); expect(cache.diagnostics().residentBytes).toBe(bytes.length)
    await first.read(h.resource, h.signal); expect(fetcher).toHaveBeenCalledTimes(3)
    cache.dispose(); h.cache.dispose()
  })
})
