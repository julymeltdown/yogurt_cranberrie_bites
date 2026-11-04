import { expect, it, vi } from 'vitest'
import { readFile } from 'node:fs/promises'
import { createMapPaintProvider, type MapPaintDescriptor } from '../../map-presentation-v1/loader'
import { map4Parade, matchesMap4Release } from '../../map-presentation-v1/identity'
import { loadedMap4 } from './fixture'
import { map4PaintSource } from '../../../../world/reference-scene/map4-service-v1/paint-source'

async function setup(themeId: 'REVENGE' | 'PARADE' | 'DANGER' = 'PARADE') {
  // Exact external artist bytes; only native image decoding is injected in this source check.
  const bytes = new Uint8Array(await readFile(new URL('../../../../../public/world/map-firstview-cozy-v1/parade-ground.png', import.meta.url)))
  const descriptor: MapPaintDescriptor = { ...map4PaintSource }
  const url = 'https://mcr.test/' + descriptor.path
  const fetch = vi.fn(async () => {
    const response = new Response(bytes, { headers: { 'Content-Type': 'image/png', 'Content-Length': String(bytes.length) } })
    Object.defineProperty(response, 'url', { value: url }); return response
  })
  const dispose = vi.fn(), decode = vi.fn(async () => ({ image: { naturalWidth: 768, naturalHeight: 1296 } as HTMLImageElement, dispose }))
  const loaded = await loadedMap4(themeId)
  // Malicious-input cases below need writable clones of the otherwise frozen validated DTOs.
  const map = { ...loaded, map: structuredClone(loaded.map), theme: structuredClone(loaded.theme) }
  return { bytes, descriptor, fetch, decode, dispose, map }
}
it.each(['REVENGE', 'PARADE', 'DANGER'] as const)('accepts only the exact public Map4/%s tuple; never edits WORLD resources or manifests', async era => {
  const s = await setup(era), original = JSON.stringify({ map: s.map.map, theme: s.map.theme, scenery: s.map.scenery })
  expect(matchesMap4Release(s.map)).toBe(true); expect(s.map.map.content_hash).toBe(map4Parade.mapHash)
  const paint = await createMapPaintProvider(s.descriptor, { origin: 'https://mcr.test', fetch: s.fetch, decode: s.decode,
    labels: async () => ({ draw() {}, dispose() {} }) })(s.map, new AbortController().signal)
  expect(paint?.plazaFill).toBe('#9fba96'); expect(s.fetch).toHaveBeenCalledOnce()
  expect(JSON.stringify({ map: s.map.map, theme: s.map.theme, scenery: s.map.scenery })).toBe(original)
  expect(paint?.ground).not.toBe(s.map.image); paint?.dispose(); expect(s.dispose).toHaveBeenCalledOnce()
})
it.each(['mapRevision', 'mapHash', 'geometry', 'theme', 'themeRevision', 'themeHash', 'dimensions'] as const)('does not fetch foreign %s paint', async kind => {
  const s = await setup()
  if (kind === 'mapRevision') s.map.map.map_revision = 7005
  if (kind === 'mapHash') s.map.map.content_hash = 'sha256:' + '0'.repeat(64)
  if (kind === 'geometry') s.map.map.geometry_hash = 'sha256:' + '0'.repeat(64)
  if (kind === 'theme') s.map.theme.theme_id = 'DANGER'
  if (kind === 'themeRevision') s.map.theme.theme_revision++
  if (kind === 'themeHash') s.map.theme.content_hash = 'sha256:' + '0'.repeat(64)
  if (kind === 'dimensions') s.map.widthPx++
  expect(await createMapPaintProvider(s.descriptor, { origin: 'https://mcr.test', fetch: s.fetch })(s.map, new AbortController().signal)).toBeNull()
  expect(s.fetch).not.toHaveBeenCalled()
})
it.each(['hash', 'bytes', 'mime', 'redirect', 'body', 'contentTamper', 'geometryTamper'] as const)('fails closed on matching-tuple %s', async kind => {
  const s = await setup()
  if (kind === 'hash') s.descriptor = { ...s.descriptor, sha256: 'sha256:' + '0'.repeat(64) }
  if (kind === 'bytes') s.descriptor = { ...s.descriptor, bytes: 34 }
  if (kind === 'body') s.bytes[32] = 1
  if (kind === 'contentTamper') s.map.theme.palette.accent = '#ffffff'
  if (kind === 'geometryTamper') s.map.map.geometry.collision_grid[0]![0] = 0
  if (kind === 'mime' || kind === 'redirect') s.fetch.mockImplementation(async () => {
    const response = new Response(s.bytes, { headers: { 'Content-Type': kind === 'mime' ? 'text/plain' : 'image/png' } })
    Object.defineProperty(response, 'url', { value: kind === 'redirect' ? 'https://foreign.test/p.png' : 'https://mcr.test/' + s.descriptor.path }); return response
  })
  await expect(async () => createMapPaintProvider(s.descriptor, { origin: 'https://mcr.test', fetch: s.fetch, decode: s.decode })(s.map, new AbortController().signal)).rejects.toThrow()
  expect(s.decode).not.toHaveBeenCalled()
})
it('releases an abort-resistant late decoded paint and never returns it after cancellation', async () => {
  const s = await setup(), abort = new AbortController()
  let finish!: (value: Awaited<ReturnType<typeof s.decode>>) => void
  const decode = vi.fn(() => new Promise<Awaited<ReturnType<typeof s.decode>>>(resolve => { finish = resolve }))
  const pending = createMapPaintProvider(s.descriptor, { origin: 'https://mcr.test', fetch: s.fetch, decode })(s.map, abort.signal)
  const rejected = expect(pending).rejects.toThrow()
  await vi.waitFor(() => expect(decode).toHaveBeenCalledOnce()); abort.abort(); await rejected
  finish(await s.decode()); await Promise.resolve(); await Promise.resolve(); expect(s.dispose).toHaveBeenCalledOnce()
})
it('cancels before HTTP and rejects bad native dimensions with immediate decoded-image cleanup', async () => {
  const s = await setup(), abort = new AbortController(); abort.abort()
  const provider = createMapPaintProvider(s.descriptor, { origin: 'https://mcr.test', fetch: s.fetch, decode: s.decode })
  await expect(provider(s.map, abort.signal)).rejects.toThrow(); expect(s.fetch).not.toHaveBeenCalled()
  s.decode.mockResolvedValue({ image: { naturalWidth: 1, naturalHeight: 1296 } as HTMLImageElement, dispose: s.dispose })
  await expect(provider(s.map, new AbortController().signal)).rejects.toThrow(); expect(s.dispose).toHaveBeenCalledOnce()
})
it('releases decoded ground on failed label qualification; no presentation can be returned', async () => {
  const s = await setup()
  const provider = createMapPaintProvider(s.descriptor, { origin: 'https://mcr.test', fetch: s.fetch, decode: s.decode,
    labels: async () => { throw Error('Font pin mismatch') } })
  await expect(provider(s.map, new AbortController().signal)).rejects.toThrow(); expect(s.dispose).toHaveBeenCalledOnce()
})
it('releases abort-resistant late labels after privacy cancellation as well as the decoded ground', async () => {
  const s = await setup(), abort = new AbortController(), fontDispose = vi.fn()
  let finish!: (value: { draw(): void; dispose(): void }) => void
  const labels = vi.fn(() => new Promise<{ draw(): void; dispose(): void }>(resolve => { finish = resolve }))
  const pending = createMapPaintProvider(s.descriptor, { origin: 'https://mcr.test', fetch: s.fetch, decode: s.decode, labels })(s.map, abort.signal)
  const rejected = expect(pending).rejects.toThrow()
  await vi.waitFor(() => expect(labels).toHaveBeenCalledOnce()); abort.abort(); await rejected
  expect(s.dispose).toHaveBeenCalledOnce(); finish({ draw() {}, dispose: fontDispose })
  await Promise.resolve(); await Promise.resolve(); expect(fontDispose).toHaveBeenCalledOnce()
})
