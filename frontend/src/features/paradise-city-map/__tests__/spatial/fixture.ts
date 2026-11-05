import { siteplanSHA256 } from '../../spatial/image-resource'
import type { SiteplanRegistration } from '../../spatial/registration'

/** Deliberately synthetic bytes and injected decoder; not a JPEG/render/GPS proof. */
export async function spatialFixture() {
  const imageBytes = new TextEncoder().encode('EXPLICIT SYNTHETIC TEST IMAGE BYTES; DECODER IS INJECTED')
  const imageSHA = await siteplanSHA256(imageBytes)
  const document = { schemaVersion: 'paradise-city-siteplan-v1', venueId: 'paradise-city', siteplanId: 'paradise-city-siteplan', revision: 1, variantId: 'synthetic-fixture-v1', sourcePixelCoordinateSpace: 'image-px', imageOrigin: 'top-left',
    image: { path: 'venue/paradise-city/v1/synthetic-test.jpg', mime: 'image/jpeg', sha256: imageSHA, bytes: imageBytes.byteLength, width: 1024, height: 1280 },
    sourceRefs: [{ id: 'synthetic-illustration', sha256: imageSHA, bytes: imageBytes.byteLength }],
    landmarks: [
      { id: 'synthetic-gate', displayName: 'Synthetic gate', anchor: { x: 500, y: 900 }, hitRegion: { x: 450, y: 850, width: 100, height: 100 }, sourceRefs: ['synthetic-illustration'], review: 'ILLUSTRATED_UNCALIBRATED' },
      { id: 'synthetic-stage', displayName: 'Synthetic stage', anchor: { x: 500, y: 300 }, hitRegion: { x: 400, y: 200, width: 200, height: 200 }, sourceRefs: ['synthetic-illustration'], review: 'ILLUSTRATED_UNCALIBRATED' },
    ] }
  const manifestBytes = new TextEncoder().encode(JSON.stringify(document))
  const registration: SiteplanRegistration = { venueId: document.venueId, siteplanId: document.siteplanId, revision: document.revision, variantId: document.variantId,
    image: { ...document.image, mime: 'image/jpeg' }, sourceRefs: document.sourceRefs.map(value => ({ ...value })), acceptedLandmarkIds: document.landmarks.map(value => value.id),
    manifest: { path: 'venue/paradise-city/v1/asset-interface.json', sha256: await siteplanSHA256(manifestBytes), bytes: manifestBytes.byteLength }, producer: { kind: 'SYNTHETIC_TEST', sourceRevision: 'synthetic-fixture-v1' } }
  return { document, registration, manifestBytes, imageBytes }
}
