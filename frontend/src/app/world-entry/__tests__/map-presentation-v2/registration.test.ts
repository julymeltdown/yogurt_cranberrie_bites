import { readFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { expect, it, vi } from 'vitest'
import compactStyle from '../../../../world/reference-scene/map4-service-v2/style-source.json'
import originalStyle from '../../../../world/reference-scene/map4-service-v1/style-source.json'
import { currentComposition } from '../../../../world/reference-scene/map4-service-v2/current'
import { compositionDigest } from '../../../../world/reference-scene/map4-service-v2/model'
import { canonicalPresentationJson } from '../../../../world/reference-scene/map4-service-v2/presentation-json'
import { sha256 } from '../../../../world/map/json'
import { pngDimensions } from '../../../../world/map/resources'
import { createMapCompositionProvider } from '../../map-presentation-v2/loader'
import { loadedMap4 } from '../map-presentation-v1/fixture'
import { map4Parade, map4ThemeHashes } from '../../../../world/reference-scene/map4-service-v1/identity'
import { presentationHash, presentationVersion, getMap4ServiceAnnotations } from '../../../../world/reference-scene/map4-service-v1/annotations'

const bytes = async (path: string) => new Uint8Array(await readFile(resolve('public', path)))
it('binds the real corrected ART layout and three scene resources to the independent runtime presentation digest', async () => {
  const model = currentComposition()
  expect(model).not.toBeNull()
  if (!model) throw Error('Allocated registration must run before source qualification')
  expect(await compositionDigest(model.registration)).toBe(model.registration.presentationHash)
  const raw = await bytes(model.registration.layout.path)
  expect(raw.length).toBe(101203); expect(await sha256(raw)).toBe(model.registration.layout.sha256)
  expect(model.source).toBe(canonicalPresentationJson(JSON.parse(new TextDecoder().decode(raw))))
  const snapshot = await readFile('src/world/reference-scene/map4-service-v2/layout-source.json')
  expect(new Uint8Array(snapshot)).toEqual(raw)
  for (const pin of model.registration.assets) {
    const scene = await bytes(pin.path)
    expect(scene.length).toBe(pin.bytes); expect(await sha256(scene)).toBe(pin.sha256)
    expect(() => pngDimensions(scene, 768, 1296)).not.toThrow()
  }
  expect(Object.isFrozen(model.registration.assets[0])).toBe(true)
})

it.each(['REVENGE', 'PARADE', 'DANGER'] as const)('prepares only the real selected %s scene and layout without modifying World/canonical authority', async era => {
  const model = currentComposition()
  if (!model) throw Error('No qualified registration')
  const loaded = await loadedMap4(era), before = JSON.stringify({ map: loaded.map, theme: loaded.theme })
  const fetch = vi.fn(async (input: RequestInfo | URL) => {
    const url = String(input), path = new URL(url).pathname.slice(1)
    const response = new Response(await bytes(path), { headers: { 'Content-Type': path.endsWith('.json') ? 'application/json' : 'image/png' } })
    Object.defineProperty(response, 'url', { value: url }); return response
  })
  const dispose = vi.fn(), labelDispose = vi.fn()
  // Fake decoded handles only: browser PNG decode/pixel fidelity remains a native gate.
  const decode = vi.fn(async () => ({ image: { naturalWidth: 768, naturalHeight: 1296 } as HTMLImageElement, dispose }))
  const labels = vi.fn(async () => ({ draw: vi.fn(), dispose: labelDispose }))
  const owner = await createMapCompositionProvider(model, { origin: 'https://mcr.test', fetch, decode, labels })(loaded, new AbortController().signal)
  expect(fetch.mock.calls.map(call => String(call[0]))).toEqual(['https://mcr.test/world/map-reference-composition-v1/layout.json',
    `https://mcr.test/world/map-reference-composition-v1/${era.toLowerCase()}-scene.png`])
  expect(owner).toMatchObject({ mode: 'full-composition', presentationHash: model.registration.presentationHash })
  expect(JSON.stringify({ map: loaded.map, theme: loaded.theme })).toBe(before)
  owner!.dispose(); owner!.dispose(); expect(dispose).toHaveBeenCalledOnce(); expect(labelDispose).toHaveBeenCalledOnce()
})

it('keeps the real four canonical display mappings and virtual semantics separate from the qualified v1 comparison', () => {
  const model = currentComposition()
  if (!model) throw Error('No qualified registration')
  const identity = { ...map4Parade, themeId: 'PARADE', themeHash: map4ThemeHashes.PARADE,
    presentationVersion: model.registration.version, presentationHash: model.registration.presentationHash }
  expect(model.annotations(identity).map(a => a.path)).toEqual(['/wall', '/feed', '/meet', '/gifts'])
  expect(model.labels.every(label => label.placeId === null && label.interactionId === null)).toBe(true)
  expect(model.displayPoint(identity, 'gifts_info', 768, 1296)).toEqual({ x: 523, y: 1230 })
  expect(model.parsed.canonicalDisplayAnchors.map(a => a.worldCell)).toEqual([{ x: 30, y: 24 }, { x: 38, y: 27 }, { x: 13, y: 48 }, { x: 14, y: 61 }])
  const old = { ...map4Parade, presentationVersion, presentationHash }
  expect(model.annotations(old)).toEqual([]); expect(getMap4ServiceAnnotations(old)).toHaveLength(4)
  for (const width of [265.5, 329, 359, 399]) for (const target of model.targets(identity, width, width * 1296 / 768)) {
    expect(target.width).toBeGreaterThanOrEqual(44); expect(target.height).toBeGreaterThanOrEqual(44)
    expect(target.left).toBeGreaterThanOrEqual(0); expect(target.top + target.height).toBeLessThanOrEqual(width * 1296 / 768)
  }
})


it('keeps the historical v1 style and original font/icons exact while independently binding compact v2 paint', async () => {
  expect(compactStyle.font).toEqual(originalStyle.font)
  expect(compactStyle.icons).toEqual(originalStyle.icons); expect(compactStyle.icon_cutouts).toEqual(originalStyle.icon_cutouts)
  expect(originalStyle.style).toMatchObject({ font_size: 40, single_height: 78, icon_size: 55 })
  expect(compactStyle.style).toMatchObject({ font_size: 40, single_height: 64, icon_size: 44, paint_mode: 'MEASURED_CENTERED_V1' })
  const model = currentComposition(); if (!model) throw Error('Fresh compact-paint registration required')
  expect(model.registration.presentationHash).not.toBe('sha256:9fcddca91030aaff0f3c3efaa1018fdeddb4708c56c9eea80024deb0bee9f522')
  expect(await compositionDigest(model.registration)).toBe(model.registration.presentationHash)
})
