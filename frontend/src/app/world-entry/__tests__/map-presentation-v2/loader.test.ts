import { expect, it, vi } from 'vitest'
import { compositionFixture } from './fixture'
import { loadedMap4 } from '../map-presentation-v1/fixture'
import { createMapCompositionProvider } from '../../map-presentation-v2/loader'
import { compositionDigest, createCompositionModel } from '../../../../world/reference-scene/map4-service-v2/model'
import { sha256 } from '../../../../world/map/json'

const origin = 'http://localhost:4443'
async function setup(theme: 'REVENGE' | 'PARADE' | 'DANGER' = 'PARADE') {
  const fixture = await compositionFixture(), map = await loadedMap4(theme)
  const sceneDispose = vi.fn(), labelDispose = vi.fn(), draw = vi.fn()
  const image = { naturalWidth: 768, naturalHeight: 1296 } as HTMLImageElement
  const decode = vi.fn(async () => ({ image, dispose: sceneDispose }))
  const labels = vi.fn(async () => ({ dispose: labelDispose, draw }))
  const fetch = vi.fn(async (input: RequestInfo | URL) => {
    const url = String(input), path = new URL(url).pathname.slice(1)
    const bytes = path.endsWith('layout.json') ? fixture.layoutBytes : fixture.images.get(path)!
    const response = new Response(bytes, { headers: { 'Content-Type': path.endsWith('.json') ? 'application/json' : 'image/png' } })
    Object.defineProperty(response, 'url', { value: url }); return response
  })
  return { ...fixture, map, image, sceneDispose, labelDispose, decode, labels, fetch }
}
it.each(['REVENGE', 'PARADE', 'DANGER'] as const)('loads only the selected %s scene and publishes full mode after layout, scene and labels qualify', async theme => {
  const s = await setup(theme), provider = createMapCompositionProvider(s.model, { origin, fetch: s.fetch, decode: s.decode, labels: s.labels })
  const paint = await provider(s.map, new AbortController().signal)
  expect(s.fetch.mock.calls.map(c => String(c[0]))).toEqual([origin + '/world/map-density-derived-v1/layout.json',
    origin + `/world/map-density-derived-v1/${theme.toLowerCase()}-scene.png`])
  expect(paint).toMatchObject({ mode: 'full-composition', scene: s.image, presentationHash: s.registration.presentationHash })
  paint!.dispose(); paint!.dispose(); expect(s.sceneDispose).toHaveBeenCalledOnce(); expect(s.labelDispose).toHaveBeenCalledOnce()
})
it('does not fetch for early cancellation or an unrelated release', async () => {
  const s = await setup(), provider = createMapCompositionProvider(s.model, { origin, fetch: s.fetch, decode: s.decode, labels: s.labels })
  const abort = new AbortController(); abort.abort()
  await expect(provider(s.map, abort.signal)).rejects.toThrow()
  const foreign = { ...s.map, map: { ...s.map.map, map_id: 'different-map' } }
  expect(await provider(foreign, new AbortController().signal)).toBeNull(); expect(s.fetch).not.toHaveBeenCalled()
})
it('rejects forged content/geometry and joint presentation pins before asset HTTP', async () => {
  const s = await setup(), map = { ...s.map, map: structuredClone(s.map.map) }
  map.map.geometry.collision_grid[0]![0] = 1 - map.map.geometry.collision_grid[0]![0]!
  const provider = createMapCompositionProvider(s.model, { origin, fetch: s.fetch })
  await expect(provider(map, new AbortController().signal)).rejects.toThrow(); expect(s.fetch).not.toHaveBeenCalled()
  const wrong = structuredClone(s.registration); wrong.presentationHash = 'sha256:' + 'f'.repeat(64)
  const bad = createMapCompositionProvider(createCompositionModel(s.document, wrong), { origin, fetch: s.fetch })
  await expect(bad(s.map, new AbortController().signal)).rejects.toThrow(); expect(s.fetch).not.toHaveBeenCalled()
})
it('rejects exact-layout or scene byte/hash corruption before decoding', async () => {
  for (const target of ['layout', 'scene']) {
    const s = await setup()
    if (target === 'layout') s.layoutBytes[12] = s.layoutBytes[12]! ^ 1
    else { const bytes = s.images.get(s.registration.assets.find(a => a.themeId === 'PARADE')!.path)!; bytes[30] = 99 }
    const provider = createMapCompositionProvider(s.model, { origin, fetch: s.fetch, decode: s.decode, labels: s.labels })
    await expect(provider(s.map, new AbortController().signal)).rejects.toThrow(); expect(s.decode).not.toHaveBeenCalled()
  }
})
it('rejects a pinned layout that differs from the compiled coordinates/source, even if its projected schema is valid', async () => {
  const s = await setup(), changed = { ...s.document, decorations: [] }
  s.layoutBytes = new TextEncoder().encode(JSON.stringify(changed))
  const registration = structuredClone(s.registration)
  registration.layout.bytes = s.layoutBytes.byteLength; registration.layout.sha256 = await sha256(s.layoutBytes)
  registration.presentationHash = await compositionDigest(registration)
  const fetch = vi.fn(async (input: RequestInfo | URL) => {
    const response = new Response(s.layoutBytes, { headers: { 'Content-Type': 'application/json' } })
    Object.defineProperty(response, 'url', { value: String(input) }); return response
  })
  const provider = createMapCompositionProvider(createCompositionModel(s.document, registration), { origin, fetch, decode: s.decode })
  await expect(provider(s.map, new AbortController().signal)).rejects.toThrow(); expect(s.decode).not.toHaveBeenCalled()
})
it('rejects wrong decoded dimensions and releases the image without preparing labels', async () => {
  const s = await setup(); Object.defineProperty(s.image, 'naturalWidth', { value: 767 })
  const provider = createMapCompositionProvider(s.model, { origin, fetch: s.fetch, decode: s.decode, labels: s.labels })
  await expect(provider(s.map, new AbortController().signal)).rejects.toThrow()
  expect(s.sceneDispose).toHaveBeenCalledOnce(); expect(s.labels).not.toHaveBeenCalled()
})
it('releases an abort-resistant late scene after cancellation and never publishes labels', async () => {
  const s = await setup(), abort = new AbortController()
  let finish!: (value: { image: HTMLImageElement; dispose(): void }) => void
  const decode = vi.fn(() => new Promise<{ image: HTMLImageElement; dispose(): void }>(resolve => { finish = resolve }))
  const provider = createMapCompositionProvider(s.model, { origin, fetch: s.fetch, decode, labels: s.labels })
  const pending = provider(s.map, abort.signal), rejected = expect(pending).rejects.toThrow()
  await vi.waitFor(() => expect(decode).toHaveBeenCalledOnce()); abort.abort(); await rejected
  finish({ image: s.image, dispose: s.sceneDispose }); await Promise.resolve(); await Promise.resolve()
  expect(s.sceneDispose).toHaveBeenCalledOnce(); expect(s.labels).not.toHaveBeenCalled()
})
it('releases the scene and abort-resistant late label lease after cancellation', async () => {
  const s = await setup(), abort = new AbortController()
  let finish!: (value: { draw(context: CanvasRenderingContext2D): void; dispose(): void }) => void
  const labels = vi.fn(() => new Promise<{ draw(context: CanvasRenderingContext2D): void; dispose(): void }>(resolve => { finish = resolve }))
  const provider = createMapCompositionProvider(s.model, { origin, fetch: s.fetch, decode: s.decode, labels })
  const pending = provider(s.map, abort.signal), rejected = expect(pending).rejects.toThrow()
  await vi.waitFor(() => expect(labels).toHaveBeenCalledOnce()); abort.abort(); await rejected
  finish({ draw() {}, dispose: s.labelDispose }); await Promise.resolve(); await Promise.resolve()
  expect(s.sceneDispose).toHaveBeenCalledOnce(); expect(s.labelDispose).toHaveBeenCalledOnce()
})
