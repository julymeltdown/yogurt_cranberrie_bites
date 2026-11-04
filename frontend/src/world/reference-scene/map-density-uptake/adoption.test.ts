import { readFile } from 'node:fs/promises'
import { expect, it, vi } from 'vitest'
import { currentComposition } from '../map4-service-v2/current'
import { compositionDigest } from '../map4-service-v2/model'
import { validateCompositionRegistration, type CompositionRegistration } from '../map4-service-v2/contract'
import { map4Parade, map4ThemeHashes } from '../map4-service-v1/identity'
import { loadedMap4 } from '../../../app/world-entry/__tests__/map-presentation-v1/fixture'
import { createMapCompositionProvider } from '../../../app/world-entry/map-presentation-v2/loader'
import muralPlacement from '../../../app/world-entry/mural-production/placement.json'

const model = currentComposition()
const priorHash = 'sha256:d8437dda63dd5f0cb554bcfcc1c267782db6630fab39c541291bb3c368d154b6'

it('replaces presentation only, keeping exact ART06 authority, paths, labels and canonical anchors', async () => {
  const old = JSON.parse(await readFile('public/world/map-reference-composition-v1/layout.json', 'utf8'))
  const next = JSON.parse(model.source)
  for (const key of ['originalMap4', 'authority', 'paths', 'gardenParcels', 'serviceLabelAnchors', 'canonicalDisplayAnchors']) {
    expect(next[key], key).toEqual(old[key])
  }
  expect(next.decorations).toHaveLength(old.decorations.length + 15)
  for (const row of old.decorations) expect(next.decorations.find((d: { id: string }) => d.id === row.id)).toEqual(row)
  expect(model.registration.version).toBe('map-roof-street-derived-v2')
  expect(await compositionDigest(model.registration)).toBe(model.registration.presentationHash)
  expect(model.registration.presentationHash).not.toBe(priorHash)
})

it.each(['REVENGE', 'PARADE', 'DANGER'] as const)('keeps %s World release and the four real service links independent of the new scenery', async themeId => {
  const loaded = await loadedMap4(themeId)
  const before = JSON.stringify(loaded)
  const identity = { ...map4Parade, themeId, themeHash: map4ThemeHashes[themeId],
    presentationVersion: model.registration.version, presentationHash: model.registration.presentationHash }
  expect(model.matches(identity)).toBe(true)
  expect(model.annotations(identity).map(a => a.path)).toEqual(['/wall', '/feed', '/meet', '/gifts'])
  expect(model.labels.every(a => a.placeId === null && a.interactionId === null)).toBe(true)
  expect(model.annotations({ ...identity, presentationHash: priorHash })).toEqual([])
  expect(model.annotations({ ...identity, geometryHash: 'sha256:' + '0'.repeat(64) })).toEqual([])
  expect(JSON.stringify(loaded)).toBe(before)
})

it.each(['old-path', 'old-version', 'external-path', 'mixed-era'] as const)('rejects %s registration rather than widening the loader allowlist', kind => {
  const pin = structuredClone(model.registration) as CompositionRegistration
  if (kind === 'old-path') Object.assign(pin.layout, { path: 'world/map-reference-composition-v1/layout.json' })
  if (kind === 'old-version') Object.assign(pin, { version: 'map4-reference-composition-v1' })
  if (kind === 'external-path') Object.assign(pin.assets[0]!, { path: 'https://foreign.test/revenge-scene.png' })
  if (kind === 'mixed-era') Object.assign(pin.assets[0]!, { path: 'world/map-roof-street-derived-v2/parade-scene.png' })
  expect(() => validateCompositionRegistration(pin)).toThrow()
})

it('does not read density resources for a mismatched authoritative release', async () => {
  const loaded = await loadedMap4('PARADE'), fetch = vi.fn()
  const foreign = { ...loaded, map: { ...loaded.map, map_revision: 5 } }
  expect(await createMapCompositionProvider(model, { origin: 'https://mcr.test', fetch })(foreign, new AbortController().signal)).toBeNull()
  expect(fetch).not.toHaveBeenCalled()
})

it('keeps the existing virtual mural binding on its original authoritative tuple', () => {
  const document = JSON.parse(model.source)
  expect(muralPlacement.map).toEqual({ id: document.originalMap4.mapId, revision: document.originalMap4.mapRevision,
    contentHash: document.originalMap4.mapContentHash, geometryHash: document.originalMap4.geometryHash,
    widthPx: 768, heightPx: 1296 })
})
