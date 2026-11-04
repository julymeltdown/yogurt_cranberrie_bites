import { readFile } from 'node:fs/promises'
import { expect, it, vi } from 'vitest'
import { currentComposition } from '../map4-service-v2/current'
import { compositionDigest } from '../map4-service-v2/model'
import { validateCompositionRegistration, type CompositionRegistration } from '../map4-service-v2/contract'
import { loadedMap4 } from '../../../app/world-entry/__tests__/map-presentation-v1/fixture'
import { createMapCompositionProvider } from '../../../app/world-entry/map-presentation-v2/loader'

const model = currentComposition()
const oldHash = 'sha256:6efb8ec1098233533e8bf7013d9e4925e4afa6af515b506412dee87c33327683'

it('binds the exact qualified roof/street layout while keeping every incumbent authority and placement', async () => {
  const old = JSON.parse(await readFile('public/world/map-density-derived-v1/layout.json', 'utf8'))
  const raw = await readFile('public/world/map-roof-street-derived-v2/layout.json', 'utf8')
  const next = JSON.parse(model.source)
  expect(JSON.parse(raw)).toEqual(next)
  for (const key of ['originalMap4', 'authority', 'paths', 'gardenParcels', 'plantingParcels',
    'serviceLabelAnchors', 'canonicalDisplayAnchors', 'decorations']) expect(next[key], key).toEqual(old[key])
  expect(next.decorations).toHaveLength(285)
  expect(model.registration.version).toBe('map-roof-street-derived-v2')
  expect(await compositionDigest(model.registration)).toBe(model.registration.presentationHash)
  expect(model.registration.presentationHash).not.toBe(oldHash)
})

it.each(['density-layout', 'density-version', 'density-image', 'foreign-image', 'mixed-era'] as const)(
  'keeps a closed v2 registration and rejects %s', kind => {
    const pin = structuredClone(model.registration) as CompositionRegistration
    if (kind === 'density-layout') Object.assign(pin.layout, { path: 'world/map-density-derived-v1/layout.json' })
    if (kind === 'density-version') Object.assign(pin, { version: 'map-density-derived-v1' })
    if (kind === 'density-image') Object.assign(pin.assets[0]!, { path: 'world/map-density-derived-v1/revenge-scene.png' })
    if (kind === 'foreign-image') Object.assign(pin.assets[0]!, { path: 'https://foreign.test/revenge-scene.png' })
    if (kind === 'mixed-era') Object.assign(pin.assets[0]!, { path: 'world/map-roof-street-derived-v2/parade-scene.png' })
    expect(() => validateCompositionRegistration(pin)).toThrow()
  })

it('does not fetch the new illustration for a foreign authoritative Map5 release', async () => {
  const loaded = await loadedMap4('PARADE'), fetch = vi.fn()
  const foreign = { ...loaded, map: { ...loaded.map, map_revision: 5 } }
  expect(await createMapCompositionProvider(model, { origin: 'https://mcr.test', fetch })(foreign, new AbortController().signal)).toBeNull()
  expect(fetch).not.toHaveBeenCalled()
})
