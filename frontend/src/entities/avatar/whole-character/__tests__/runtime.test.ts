import { afterEach, expect, it, vi } from 'vitest'
import type { DecodedImage } from '../../../../world/map/resources'
import { createWholeCharacterRuntime } from '../runtime'
import { config, deferred, fixture, manifest, pin, resource } from './fixture'
const owned: ReturnType<typeof fixture>[] = []
const setup = (patch: Parameters<typeof fixture>[0] = {}) => { const f = fixture(patch); owned.push(f); return f }
afterEach(() => { owned.splice(0).forEach(f => f.dispose()) })
it('shares exactly one reviewed compact atlas across identities/consumers and keeps caller cancellation independent', async () => {
  const f = setup(), a = new AbortController(), b = new AbortController()
  const [one, two] = await Promise.all([f.runtime.acquire(config, { signal: a.signal }), f.runtime.acquire(manifest.presets[5]!.config, { signal: b.signal })])
  expect(f.fetcher).toHaveBeenCalledTimes(2); expect(f.decode).toHaveBeenCalledTimes(1)
  expect(one.read()?.pose).toBe('front_idle'); expect(two.read()?.source).toEqual({ x: 320, y: 0, width: 64, height: 64 })
  const clear = vi.fn(); one.subscribe(clear); a.abort(); expect(one.read()).toBeNull(); expect(clear).toHaveBeenCalledOnce()
  expect(two.read()?.config.preset_id).toBe('photo01_front_06'); expect(f.disposed[0]).not.toHaveBeenCalled()
  two.dispose(); expect(f.runtime.diagnostics().leases).toBe(0)
  f.runtime.dispose(); expect(f.disposed[0]).toHaveBeenCalledOnce()
})
it('choices verify metadata without PNG; explicit detail loads only256, same identity and logical foot anchor', async () => {
  const f = setup(), choices = await f.runtime.choices(f.signal()); expect(choices).toHaveLength(6); expect(f.fetcher).toHaveBeenCalledTimes(1); expect(f.decode).not.toHaveBeenCalled()
  const lease = await f.runtime.acquire(choices[2]!.config, { signal: f.signal(), detail: true })
  expect(f.fetcher).toHaveBeenCalledTimes(2); expect(String(f.fetcher.mock.calls[1]![0])).toContain('256-')
  expect(lease.read()?.source).toEqual({ x: 512, y: 0, width: 256, height: 256 }); expect(lease.read()?.anchor).toEqual({ x: 32, y: 60 })
  expect(lease.read()?.config.preset_id).toBe('photo01_front_03'); expect(f.runtime.diagnostics().decodedAtlases).toBe(1)
})
it('canonical policy loss synchronously revokes every sink even when another subscriber throws and disposes decoded resources', async () => {
  const f = setup(), lease = await f.runtime.acquire(config, { signal: f.signal() }), clear = vi.fn()
  lease.subscribe(() => { throw Error('bad sink') }); lease.subscribe(clear)
  f.auth.privacy.changePolicy(f.auth.privacy.capture(), 'block')
  expect(clear).toHaveBeenCalledOnce(); expect(lease.read()).toBeNull(); expect(f.runtime.isCurrent()).toBe(false); expect(f.disposed[0]).toHaveBeenCalledOnce()
  await expect(f.runtime.acquire(config, { signal: f.signal() })).rejects.toThrow()
})
it('rejects invalid config before fetch, owner construction without identity, unreviewed pin and exhausted bounded leases', async () => {
  const f = setup(); await expect(f.runtime.acquire({ ...config, item_ids: {} }, { signal: f.signal() })).rejects.toThrow(); expect(f.fetcher).not.toHaveBeenCalled()
  expect(() => createWholeCharacterRuntime({ privacy: { ...f.auth.privacy, capture: () => ({ ...f.auth.privacy.capture(), userId: null }) }, manifest, pin, origin: 'https://fixture.invalid' })).toThrow()
  const leases = await Promise.all(Array.from({ length: 256 }, () => f.runtime.acquire(config, { signal: f.signal() })))
  await expect(f.runtime.acquire(config, { signal: f.signal() })).rejects.toMatchObject({ reason: 'capacity' })
  leases.forEach(l => l.dispose()); expect(f.runtime.diagnostics().leases).toBe(0)
})
it.each(['hash', 'mime', 'redirect', 'length', 'oversize'])('fails closed on corrupted/misdirected resource %s without decoding', async mode => {
  const f = setup({ fetch: async input => {
    const response = await resource(String(input))
    if (mode === 'redirect') Object.defineProperty(response, 'url', { value: 'https://other.invalid/no.png', configurable: true })
    if (mode === 'mime') response.headers.set('content-type', 'text/html')
    if (mode === 'length') response.headers.set('content-length', '999999999')
    if (mode === 'hash' || mode === 'oversize') {
      const data = new Uint8Array(await response.arrayBuffer()), result = new Response(mode === 'hash' ? data.fill(1) : new Uint8Array(data.length + 1), { headers: { 'content-type': response.headers.get('content-type')! } }); Object.defineProperty(result, 'url', { value: String(input) }); return result
    }
    return response
  } })
  await expect(f.runtime.acquire(config, { signal: f.signal() })).rejects.toThrow(); expect(f.decode).not.toHaveBeenCalled()
})
it('aborts the final cancelled waiter and disposes an injected decoder that completes after owner loss', async () => {
  const pending = deferred<DecodedImage>(), started = deferred<void>(), close = vi.fn()
  const f = setup({ decode: async () => { started.resolve(); return pending.promise } }), signal = new AbortController()
  const acquisition = f.runtime.acquire(config, { signal: signal.signal }); const rejected = expect(acquisition).rejects.toThrow(); await started.promise
  signal.abort(); await rejected; f.runtime.dispose()
  pending.resolve({ image: { naturalWidth: 384, naturalHeight: 64 } as HTMLImageElement, dispose: close })
  await vi.waitFor(() => expect(close).toHaveBeenCalledOnce()); expect(f.runtime.diagnostics().leases).toBe(0)
})
it('one cancelled in-flight caller cannot abort another caller sharing the decoder', async () => {
  const pending = deferred<DecodedImage>(), started = deferred<void>()
  const f = setup({ decode: async () => { started.resolve(); return pending.promise } }), cancelled = new AbortController()
  const a = f.runtime.acquire(config, { signal: cancelled.signal }), b = f.runtime.acquire(config, { signal: f.signal() }), rejection = expect(a).rejects.toThrow()
  await started.promise; cancelled.abort(); await rejection
  pending.resolve({ image: { naturalWidth: 384, naturalHeight: 64 } as HTMLImageElement, dispose() {} })
  const live = await b; expect(live.read()?.config).toEqual(config); expect(f.runtime.diagnostics().leases).toBe(1)
})
it('never shares a prior owner cache across actor replacement and rejects wrong decoded dimensions', async () => {
  const f = setup(); const lease = await f.runtime.acquire(config, { signal: f.signal() })
  f.auth.session.replace(f.auth.session.read().generation, { eventId: f.auth.session.read().eventId!, userId: '00000000-0000-4000-8000-000000000099', accessToken: 'other-fixture-memory-token' })
  expect(lease.read()).toBeNull(); expect(f.runtime.diagnostics().disposed).toBe(true)
  const bad = setup({ decode: async () => ({ image: { naturalWidth: 1, naturalHeight: 1 } as HTMLImageElement, dispose() {} }) }); await expect(bad.runtime.acquire(config, { signal: bad.signal() })).rejects.toThrow()
})
it('deadline cancellation cleans an ignored late decoder and a failing disposer cannot prevent cleanup of the other size', async () => {
  const pending = deferred<DecodedImage>(), started = deferred<void>(), lateDispose = vi.fn()
  vi.useFakeTimers()
  try {
    const f = setup({ timeoutMs: 100, decode: async () => { started.resolve(); return pending.promise } })
    const work = f.runtime.acquire(config, { signal: f.signal() }), rejection = expect(work).rejects.toThrow()
    await started.promise; await vi.advanceTimersByTimeAsync(100); await rejection
    pending.resolve({ image: { naturalWidth: 384, naturalHeight: 64 } as HTMLImageElement, dispose: lateDispose }); await vi.advanceTimersByTimeAsync(0)
    expect(lateDispose).toHaveBeenCalledOnce(); expect(f.runtime.diagnostics().decodedAtlases).toBe(0)
  } finally { vi.useRealTimers() }
  const cleanup = vi.fn(), f = setup({ decode: async bytes => {
    const v = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength), large = v.getUint32(20) === 256
    return { image: { naturalWidth: v.getUint32(16), naturalHeight: v.getUint32(20) } as HTMLImageElement, dispose() { if (large) cleanup(); else throw Error('bad decoder cleanup') } }
  } })
  await f.runtime.acquire(config, { signal: f.signal() }); await f.runtime.acquire(config, { signal: f.signal(), detail: true })
  f.runtime.dispose(); expect(cleanup).toHaveBeenCalledOnce(); expect(f.runtime.diagnostics()).toMatchObject({ cleanupFailures: 1, decodedAtlases: 0, leases: 0 })
})
