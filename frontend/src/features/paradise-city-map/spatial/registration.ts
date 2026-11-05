import { landmarkId, requireSiteplan } from './contract'
import type { SiteplanBinding } from './model'

export interface ResourcePin { readonly path: string; readonly sha256: string; readonly bytes: number }
export interface SiteplanRegistration extends SiteplanBinding {
  readonly manifest: ResourcePin
  readonly producer: Readonly<{ kind: 'QUALIFIED_PRODUCER' | 'SYNTHETIC_TEST'; sourceRevision: string }>
}
/** Snapshot immutable pins at the boundary; never retain caller-owned mutable arrays. */
export function freezeSiteplanRegistration(value: SiteplanRegistration): SiteplanRegistration {
  requireSiteplan(value.venueId === 'paradise-city' && value.siteplanId === 'paradise-city-siteplan')
  requireSiteplan(Number.isSafeInteger(value.revision) && value.revision > 0)
  landmarkId(value.variantId)
  requireSiteplan(value.manifest.path === 'venue/paradise-city/v1/asset-interface.json' && /^[a-f0-9]{64}$/.test(value.manifest.sha256) && Number.isSafeInteger(value.manifest.bytes) && value.manifest.bytes > 0 && value.manifest.bytes <= 65536)
  requireSiteplan(value.producer.kind === 'QUALIFIED_PRODUCER' ? /^[a-f0-9]{40}$/.test(value.producer.sourceRevision) : value.producer.kind === 'SYNTHETIC_TEST' && value.producer.sourceRevision.length > 0 && value.producer.sourceRevision.length <= 128)
  requireSiteplan(value.acceptedLandmarkIds.length > 0 && value.acceptedLandmarkIds.length <= 128 && new Set(value.acceptedLandmarkIds).size === value.acceptedLandmarkIds.length)
  value.acceptedLandmarkIds.forEach(landmarkId)
  return Object.freeze({ venueId: value.venueId, siteplanId: value.siteplanId, revision: value.revision, variantId: value.variantId,
    image: Object.freeze({ ...value.image }), sourceRefs: Object.freeze(value.sourceRefs.map(source => Object.freeze({ ...source }))),
    acceptedLandmarkIds: Object.freeze([...value.acceptedLandmarkIds]), manifest: Object.freeze({ ...value.manifest }), producer: Object.freeze({ ...value.producer }) })
}
/** ART owner-qualified illustration pins; source geometry remains solely in its manifest.
 * Queue/P/R labels and medical artwork imply no physical right, capacity or service. */
export const productionSiteplanRegistration: SiteplanRegistration = freezeSiteplanRegistration({
  "venueId": "paradise-city",
  "siteplanId": "paradise-city-siteplan",
  "revision": 1,
  "variantId": "photo-5-provisional",
  "image": {
    "path": "venue/paradise-city/v1/siteplan-photo-5.jpg",
    "mime": "image/jpeg",
    "sha256": "d0713633289f87fc98603927f7cbc02df2054e989f6360332a93476d6c9da4e0",
    "bytes": 328720,
    "width": 1024,
    "height": 1280
  },
  "sourceRefs": [
    {
      "id": "photo-5",
      "sha256": "d0713633289f87fc98603927f7cbc02df2054e989f6360332a93476d6c9da4e0",
      "bytes": 328720
    },
    {
      "id": "photo-10",
      "sha256": "1f0e133be3a4bf999fb7871188cce67086d09106a095c266b74e648a67777f63",
      "bytes": 320653
    }
  ],
  "acceptedLandmarkIds": [
    "stage",
    "standing-a",
    "standing-b",
    "foh",
    "main-gate",
    "sub-gate",
    "station",
    "chroma",
    "studio-paradise",
    "spa-cimer",
    "wonderbox",
    "art-paradiso-plaza",
    "photo-zone",
    "queue-a",
    "queue-b",
    "queue-a-b-3001-plus",
    "p1",
    "p2",
    "r1",
    "r2",
    "toilets",
    "medical-symbol"
  ],
  "manifest": {
    "path": "venue/paradise-city/v1/asset-interface.json",
    "sha256": "a29a47add30fe9fe9cd20dcab5f5dd5004cf1290106794fb407991f1c5142dcd",
    "bytes": 8202
  },
  "producer": {
    "kind": "QUALIFIED_PRODUCER",
    "sourceRevision": "46b4d8670d31365a88b9006250befdcf2832135e"
  }
})
