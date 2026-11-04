import { expect, it } from 'vitest'
import { compositionFixture } from './fixture'
import { decodeCompositionLayout, validateCompositionRegistration } from '../../../../world/reference-scene/map4-service-v2/contract'
import { createCompositionModel } from '../../../../world/reference-scene/map4-service-v2/model'
import { map4Parade, map4ThemeHashes } from '../../../../world/reference-scene/map4-service-v1/identity'
import { canonicalJson } from '../../../../world/map/json'
import { canonicalPresentationJson } from '../../../../world/reference-scene/map4-service-v2/presentation-json'

it('supports fractional illustration clearances without widening the integer-only C06 wire contract', () => {
  expect(canonicalPresentationJson({ b: .5, a: 2 })).toBe('{"a":2,"b":0.5}')
  expect(() => canonicalJson({ b: .5 })).toThrow()
  expect(() => canonicalPresentationJson({ b: Infinity })).toThrow()
})

it('projects six independently placed semantic labels and four explicit canonical display mappings', async () => {
  const f = await compositionFixture(), p = decodeCompositionLayout(f.document, f.registration)
  expect(p.serviceLabelAnchors).toHaveLength(6); expect(p.canonicalDisplayAnchors).toHaveLength(4)
  expect(p.canonicalDisplayAnchors.find(a => a.interactionId === 'gifts_info')).toMatchObject({ x: 523, y: 1230, worldCell: { x: 13, y: 48 } })
  expect(f.model.labels.every(l => l.placeId === null && l.interactionId === null)).toBe(true)
})
it('rejects altered geometry, source era pins, extra/missing semantic keys and canonical server cells', async () => {
  const f = await compositionFixture()
  for (const mutate of [
    (d: typeof f.document) => { d.originalMap4.geometryHash = 'sha256:' + 'f'.repeat(64) },
    (d: typeof f.document) => { d.originalMap4.themes[0]!.themeRevision = 2 },
    (d: typeof f.document) => { d.serviceLabelAnchors[0]!.key = 'food' },
    (d: typeof f.document) => { d.serviceLabelAnchors[0]!.key = d.serviceLabelAnchors[1]!.key },
    (d: typeof f.document) => { d.canonicalDisplayAnchors[0]!.worldCell.x++ },
    (d: typeof f.document) => { d.canonicalDisplayAnchors[0]!.authority = 'PHYSICAL_ARRIVAL' },
    (d: typeof f.document) => { d.serviceLabelAnchors[0]!.exclusionRect.height = 78 },
    (d: typeof f.document) => { d.assets[0]!.sha256 = 'sha256:' + 'b'.repeat(64) },
  ]) {
    const document = structuredClone(f.document); mutate(document)
    expect(() => decodeCompositionLayout(document, f.registration)).toThrow()
  }
})
it('rejects foreign URLs, duplicate eras and oversized scene or layout descriptors', async () => {
  const f = await compositionFixture()
  for (const mutate of [
    (p: typeof f.registration) => { p.assets[0]!.path = 'https://foreign.test/scene.png' },
    (p: typeof f.registration) => { p.assets[0]!.bytes = 2_097_153 },
    (p: typeof f.registration) => { p.layout.bytes = 524_289 },
    (p: typeof f.registration) => { p.assets[0]!.themeId = p.assets[1]!.themeId },
  ]) {
    const pin = structuredClone(f.registration); mutate(pin)
    expect(() => validateCompositionRegistration(pin)).toThrow()
  }
})
it('owns immutable metadata and gates all links/display mappings on the complete new presentation identity', async () => {
  const f = await compositionFixture(), model = createCompositionModel(f.document, f.registration)
  f.document.serviceLabelAnchors[0]!.x = 20; f.registration.assets[0]!.bytes++
  expect(model.labels[0]!.x).toBe(410); expect(Object.isFrozen(model.registration.assets[0])).toBe(true)
  const identity = { ...map4Parade, presentationVersion: f.document.version, presentationHash: model.registration.presentationHash }
  expect(model.annotations(identity).map(a => a.path)).toEqual(['/wall', '/feed', '/meet', '/gifts'])
  expect(model.displayPoint(identity, 'gifts_info', 384, 648)).toEqual({ x: 261.5, y: 615 })
  const { presentationHash: omittedHash, ...missing } = identity
  expect(omittedHash).toBe(model.registration.presentationHash)
  expect(model.annotations(missing)).toEqual([])
  expect(model.displayPoint(missing, 'gifts_info', 384, 648)).toBeNull()
  for (const patch of [{ presentationVersion: 'map4-service-v1' }, { mapRevision: 7005 },
    { themeId: 'DANGER', themeHash: map4ThemeHashes.PARADE }]) {
    expect(model.annotations({ ...identity, ...patch })).toEqual([])
    expect(model.displayPoint({ ...identity, ...patch }, 'gifts_info', 384, 648)).toBeNull()
  }
})
it('keeps all four service targets fully inside normal and short local image bounds at 44px minimum', async () => {
  const f = await compositionFixture(), identity = { ...map4Parade,
    presentationVersion: f.document.version, presentationHash: f.registration.presentationHash }
  for (const width of [265.5, 329, 359, 399]) for (const target of f.model.targets(identity, width, width * 1296 / 768)) {
    expect(target.width).toBeGreaterThanOrEqual(44); expect(target.height).toBeGreaterThanOrEqual(44)
    expect(target.left).toBeGreaterThanOrEqual(0); expect(target.top).toBeGreaterThanOrEqual(0)
    expect(target.left + target.width).toBeLessThanOrEqual(width)
    expect(target.top + target.height).toBeLessThanOrEqual(width * 1296 / 768)
  }
})
