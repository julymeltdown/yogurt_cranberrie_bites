import { describe, expect, it, vi } from 'vitest'
import { createSiteplanLoader, siteplanSHA256, type DecodedSiteplanImage } from '../../spatial/image-resource'
import { spatialFixture } from './fixture'

type Fixture = Awaited<ReturnType<typeof spatialFixture>>
function requestFor(fixture: Fixture, manifest = fixture.manifestBytes) {
  return vi.fn<typeof fetch>(async input => {
    const url = new URL(String(input)); const bytes = url.pathname.endsWith('/asset-interface.json') ? manifest : fixture.imageBytes
    return new Response(new Uint8Array(bytes).buffer)
  })
}
function decoded(dispose = vi.fn(), width = 1024): DecodedSiteplanImage {
  return { source: {} as CanvasImageSource, naturalWidth: width, naturalHeight: 1280, dispose }
}
function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(done => { resolve = done }); return { promise, resolve } }

describe('finite siteplan resources and ownership', () => {
  it('fetches only pinned same-origin manifest/image and disposes exactly once', async () => {
    const fixture = await spatialFixture(), request = requestFor(fixture), dispose = vi.fn(), loader = createSiteplanLoader(fixture.registration, { origin: 'https://mcr.example.test', fetch: request, decode: async () => decoded(dispose) })
    const result = await loader.load()
    expect(request.mock.calls.map(call => String(call[0]))).toEqual(['https://mcr.example.test/venue/paradise-city/v1/asset-interface.json', 'https://mcr.example.test/venue/paradise-city/v1/synthetic-test.jpg'])
    expect(result.producer.kind).toBe('SYNTHETIC_TEST'); expect(result.snapshot.siteplan.landmarks).toHaveLength(2)
    result.dispose(); result.dispose(); loader.dispose(); expect(dispose).toHaveBeenCalledTimes(1)
  })
  it('keeps a delivered image beyond the request deadline until its actual owner releases it', async () => {
    const fixture = await spatialFixture(), dispose = vi.fn(), external = new AbortController()
    const loader = createSiteplanLoader(fixture.registration, { origin: 'https://mcr.example.test', fetch: requestFor(fixture), decode: async () => decoded(dispose) })
    vi.useFakeTimers()
    try {
      await loader.load(external.signal)
      await vi.advanceTimersByTimeAsync(16000)
      expect(dispose).not.toHaveBeenCalled()
      external.abort(); expect(dispose).toHaveBeenCalledTimes(1)
    } finally { loader.dispose(); vi.useRealTimers() }
  })
  it('aborts an unfinished decoder at the request deadline and disposes its late result', async () => {
    const fixture = await spatialFixture(), value = deferred<DecodedSiteplanImage>(), started = deferred<void>(), dispose = vi.fn()
    const loader = createSiteplanLoader(fixture.registration, { origin: 'https://mcr.example.test', fetch: requestFor(fixture), decode: () => { started.resolve(); return value.promise } })
    vi.useFakeTimers()
    try {
      const load = loader.load(), rejected = expect(load).rejects.toMatchObject({ code: 'ABORTED' })
      await started.promise; await vi.advanceTimersByTimeAsync(15000); await rejected
      value.resolve(decoded(dispose)); await Promise.resolve(); await Promise.resolve()
      expect(dispose).toHaveBeenCalledTimes(1)
    } finally { loader.dispose(); vi.useRealTimers() }
  })
  it('rejects valid-looking geometry tampering against the exact manifest hash', async () => {
    const fixture = await spatialFixture(), changed = structuredClone(fixture.document); changed.landmarks[0]!.anchor.x++
    const bytes = new TextEncoder().encode(JSON.stringify(changed))
    const loader = createSiteplanLoader(fixture.registration, { origin: 'https://mcr.example.test', fetch: requestFor(fixture, bytes), decode: async () => decoded() })
    await expect(loader.load()).rejects.toMatchObject({ code: 'HASH_MISMATCH' }); loader.dispose()
  })
  it('rejects a correctly hashed manifest for the wrong revision before image/decode', async () => {
    const fixture = await spatialFixture(); fixture.document.revision++
    const bytes = new TextEncoder().encode(JSON.stringify(fixture.document)), request = requestFor(fixture, bytes)
    const registration = { ...fixture.registration, manifest: { ...fixture.registration.manifest, bytes: bytes.byteLength, sha256: await siteplanSHA256(bytes) } }
    const decode = vi.fn(async () => decoded()), loader = createSiteplanLoader(registration, { origin: 'https://mcr.example.test', fetch: request, decode })
    await expect(loader.load()).rejects.toMatchObject({ code: 'IDENTITY_MISMATCH' }); expect(request).toHaveBeenCalledTimes(1); expect(decode).not.toHaveBeenCalled(); loader.dispose()
  })
  it('bounds streamed bytes even without Content-Length', async () => {
    const fixture = await spatialFixture(), extra = new Uint8Array(fixture.manifestBytes.byteLength + 1); extra.set(fixture.manifestBytes)
    const loader = createSiteplanLoader(fixture.registration, { origin: 'https://mcr.example.test', fetch: requestFor(fixture, extra), decode: async () => decoded() })
    await expect(loader.load()).rejects.toMatchObject({ code: 'INVALID_CONTRACT' }); loader.dispose()
  })
  it('cleans a late decoder after external abort', async () => {
    const fixture = await spatialFixture(), value = deferred<DecodedSiteplanImage>(), started = deferred<void>(), dispose = vi.fn(), external = new AbortController()
    const loader = createSiteplanLoader(fixture.registration, { origin: 'https://mcr.example.test', fetch: requestFor(fixture), decode: () => { started.resolve(); return value.promise } })
    const load = loader.load(external.signal); await started.promise; external.abort()
    await expect(load).rejects.toMatchObject({ code: 'ABORTED' }); value.resolve(decoded(dispose)); await Promise.resolve(); await Promise.resolve()
    expect(dispose).toHaveBeenCalledTimes(1); loader.dispose()
  })
  it('rejects stale first loads while a later generation wins without leaking the late image', async () => {
    const fixture = await spatialFixture(), value = deferred<DecodedSiteplanImage>(), started = deferred<void>(), firstDispose = vi.fn(), secondDispose = vi.fn()
    let count = 0
    const loader = createSiteplanLoader(fixture.registration, { origin: 'https://mcr.example.test', fetch: requestFor(fixture), decode: () => { if (++count === 1) { started.resolve(); return value.promise }; return Promise.resolve(decoded(secondDispose)) } })
    const first = loader.load(); await started.promise
    const rejected = expect(first).rejects.toMatchObject({ code: 'STALE' }), second = await loader.load(); await rejected
    value.resolve(decoded(firstDispose)); await Promise.resolve(); await Promise.resolve(); expect(firstDispose).toHaveBeenCalledTimes(1)
    second.dispose(); loader.dispose(); expect(secondDispose).toHaveBeenCalledTimes(1)
  })
  it('rejects mismatched natural dimensions and releases the decoded image', async () => {
    const fixture = await spatialFixture(), dispose = vi.fn(), loader = createSiteplanLoader(fixture.registration, { origin: 'https://mcr.example.test', fetch: requestFor(fixture), decode: async () => decoded(dispose, 2000) })
    await expect(loader.load()).rejects.toMatchObject({ code: 'DIMENSION_MISMATCH' }); expect(dispose).toHaveBeenCalledTimes(1); loader.dispose()
  })
  it('takes immutable pin copies and releases a delivered lease when the next load begins', async () => {
    const fixture = await spatialFixture(), firstDispose = vi.fn(), nextDispose = vi.fn(); let count = 0
    const loader = createSiteplanLoader(fixture.registration, { origin: 'https://mcr.example.test', fetch: requestFor(fixture), decode: async () => decoded(++count === 1 ? firstDispose : nextDispose) })
    ;(fixture.registration.manifest as { sha256: string }).sha256 = '0'.repeat(64)
    await loader.load(); const next = await loader.load(); expect(firstDispose).toHaveBeenCalledTimes(1); next.dispose(); loader.dispose(); expect(nextDispose).toHaveBeenCalledTimes(1)
    await expect(loader.load()).rejects.toMatchObject({ code: 'UNAVAILABLE' })
  })
})
