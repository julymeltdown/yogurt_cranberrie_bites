import { describe, expect, it } from 'vitest'
import { decodeSiteplan, imagePoint, SiteplanError } from '../../spatial/contract'
import { createSiteplanModel } from '../../spatial/model'
import { spatialFixture } from './fixture'

describe('physical illustration model boundary', () => {
  it('freezes independent decoded snapshots and declares missing geographic calibration', async () => {
    const { document, registration } = await spatialFixture(), model = createSiteplanModel(document, registration)
    document.landmarks[0]!.displayName = 'Mutated caller data'
    expect(model.landmark('synthetic-gate').displayName).toBe('Synthetic gate')
    expect(Object.isFrozen(model.snapshot.siteplan.landmarks[0]!.anchor)).toBe(true)
    expect(model.snapshot.geographicProjection).toEqual({ state: 'unavailable', reason: 'UNCALIBRATED_ILLUSTRATION' })
  })
  it.each(['revision', 'variant', 'image', 'sources', 'landmark'] as const)('rejects mixed %s before adoption', async field => {
    const { document, registration } = await spatialFixture()
    if (field === 'revision') document.revision++
    if (field === 'variant') document.variantId = 'another-variant'
    if (field === 'image') document.image.path = 'venue/paradise-city/v1/another.jpg'
    if (field === 'sources') document.sourceRefs[0]!.id = document.landmarks[0]!.sourceRefs[0] = document.landmarks[1]!.sourceRefs[0] = 'another-source'
    if (field === 'landmark') document.landmarks[0]!.id = 'unknown-candidate'
    expect(() => createSiteplanModel(document, registration)).toThrowError(expect.objectContaining({ code: 'IDENTITY_MISMATCH' }))
  })
  it.each([NaN, Infinity, -1, 1024, 0.5])('rejects invalid source pixel anchor %s', async x => {
    const { document } = await spatialFixture(); document.landmarks[0]!.anchor.x = x
    expect(() => decodeSiteplan(document)).toThrow(SiteplanError)
  })
  it('rejects duplicate candidates, unknown source references and unrequested geographic fields', async () => {
    const { document } = await spatialFixture()
    const duplicate = structuredClone(document); duplicate.landmarks[1]!.id = duplicate.landmarks[0]!.id
    expect(() => decodeSiteplan(duplicate)).toThrow(SiteplanError)
    const unknown = structuredClone(document); unknown.landmarks[0]!.sourceRefs = ['unknown-source']
    expect(() => decodeSiteplan(unknown)).toThrow(SiteplanError)
    expect(() => decodeSiteplan({ ...document, latitude: 37, longitude: 126 })).toThrow(SiteplanError)
    expect(() => decodeSiteplan({ ...document, sourcePixelCoordinateSpace: 'world-px' })).toThrow(SiteplanError)
  })
  it.each(['https://example.com/a.jpg', '../a.jpg', 'venue/paradise-city/v1/a.jpg?private=1'])('rejects non-allowlisted image path %s', async path => {
    const { document } = await spatialFixture(); document.image.path = path
    expect(() => decodeSiteplan(document)).toThrow(SiteplanError)
  })
  it('keeps source geometry bounded and rejects anchors outside their own hit region', async () => {
    const { document } = await spatialFixture(); document.landmarks[0]!.hitRegion.width = 1024
    expect(() => decodeSiteplan(document)).toThrow(SiteplanError)
    document.landmarks[0]!.hitRegion.width = 20
    expect(() => decodeSiteplan(document)).toThrow(SiteplanError)
  })
  it.each(['\u0000', '\u001f', '\u007f'])('rejects internal control characters in display labels (%s)', async control => {
    const { document } = await spatialFixture(); document.landmarks[0]!.displayName = 'Label' + control + 'text'
    expect(() => decodeSiteplan(document)).toThrow(SiteplanError)
  })
  it('preserves ordinary Unicode display labels', async () => {
    const { document } = await spatialFixture(); document.landmarks[0]!.displayName = 'Étoile 🎵'
    expect(decodeSiteplan(document).landmarks[0]!.displayName).toBe('Étoile 🎵')
  })
  it('looks up accepted presentation keys and uses half-open source hit regions', async () => {
    const { document, registration } = await spatialFixture(), model = createSiteplanModel(document, registration)
    expect(model.hitTest(imagePoint(500, 900))?.id).toBe('synthetic-gate')
    expect(model.hitTest(imagePoint(550, 900))).toBeNull()
    expect(model.hitTest(imagePoint(-5, 900))).toBeNull()
    expect(() => model.landmark('server-place-uuid')).toThrowError(expect.objectContaining({ code: 'UNKNOWN_LANDMARK' }))
  })
})
