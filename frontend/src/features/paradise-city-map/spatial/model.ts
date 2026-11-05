import { decodeSiteplan, requireSiteplan, sameSiteplanIdentity, type ImagePoint, type Landmark, type LandmarkId, type PhysicalVenueSnapshot, type Siteplan, type SiteplanIdentity } from './contract'

export interface SiteplanBinding extends SiteplanIdentity {
  readonly image: Siteplan['image']
  readonly sourceRefs: Siteplan['sourceRefs']
  /** Explicitly adopted source candidate keys; geometry is never copied into a second registry. */
  readonly acceptedLandmarkIds: readonly string[]
}
export interface SiteplanModel {
  readonly snapshot: PhysicalVenueSnapshot
  landmark(id: LandmarkId | string): Landmark
  hitTest(point: ImagePoint): Landmark | null
}
export function createSiteplanModel(input: unknown, binding: SiteplanBinding): SiteplanModel {
  const siteplan = decodeSiteplan(input)
  requireSiteplan(sameSiteplanIdentity(siteplan, binding), 'IDENTITY_MISMATCH')
  const image = binding.image
  requireSiteplan(siteplan.image.path === image.path && siteplan.image.sha256 === image.sha256 && siteplan.image.bytes === image.bytes && siteplan.image.mime === image.mime && siteplan.image.width === image.width && siteplan.image.height === image.height, 'IDENTITY_MISMATCH')
  requireSiteplan(siteplan.sourceRefs.length === binding.sourceRefs.length && siteplan.sourceRefs.every(source => binding.sourceRefs.some(expected => expected.id === source.id && expected.sha256 === source.sha256 && expected.bytes === source.bytes)), 'IDENTITY_MISMATCH')
  requireSiteplan(binding.acceptedLandmarkIds.length > 0 && new Set(binding.acceptedLandmarkIds).size === binding.acceptedLandmarkIds.length && siteplan.landmarks.length === binding.acceptedLandmarkIds.length && siteplan.landmarks.every(landmark => binding.acceptedLandmarkIds.includes(landmark.id)), 'IDENTITY_MISMATCH')
  const byId = new Map(siteplan.landmarks.map(landmark => [landmark.id as string, landmark]))
  const ordered = [...siteplan.landmarks].sort((a, b) => a.hitRegion.width * a.hitRegion.height - b.hitRegion.width * b.hitRegion.height)
  const snapshot: PhysicalVenueSnapshot = Object.freeze({ siteplan, geographicProjection: Object.freeze({ state: 'unavailable', reason: 'UNCALIBRATED_ILLUSTRATION' }) })
  return Object.freeze({
    snapshot,
    landmark(id: LandmarkId | string) { const value = byId.get(id); requireSiteplan(value, 'UNKNOWN_LANDMARK'); return value },
    hitTest(point: ImagePoint) {
      requireSiteplan(Number.isFinite(point.x) && Number.isFinite(point.y))
      return ordered.find(({ hitRegion: r }) => point.x >= r.x && point.x < r.x + r.width && point.y >= r.y && point.y < r.y + r.height) ?? null
    },
  })
}
