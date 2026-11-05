/** Physical-site illustration coordinates only; never World or geographic coordinates. */
declare const pointBrand: unique symbol
declare const landmarkBrand: unique symbol
export type ImagePoint = Readonly<{ x: number; y: number }> & { readonly [pointBrand]: 'image-px' }
export type LandmarkId = string & { readonly [landmarkBrand]: 'presentation-landmark' }
export interface ImageRect { readonly x: number; readonly y: number; readonly width: number; readonly height: number }
export interface SiteplanIdentity { readonly venueId: string; readonly siteplanId: string; readonly revision: number; readonly variantId: string }
export interface SourceReference { readonly id: string; readonly sha256: string; readonly bytes: number }
export interface SiteplanImage { readonly path: string; readonly mime: 'image/jpeg'; readonly sha256: string; readonly bytes: number; readonly width: number; readonly height: number }
export interface Landmark {
  readonly id: LandmarkId; readonly displayName: string; readonly anchor: ImagePoint
  readonly hitRegion: ImageRect; readonly sourceRefs: readonly string[]; readonly review: 'ILLUSTRATED_UNCALIBRATED'
}
export interface Siteplan extends SiteplanIdentity {
  readonly schemaVersion: 'paradise-city-siteplan-v1'; readonly sourcePixelCoordinateSpace: 'image-px'; readonly imageOrigin: 'top-left'
  readonly image: SiteplanImage; readonly sourceRefs: readonly SourceReference[]; readonly landmarks: readonly Landmark[]
}
export interface PhysicalVenueSnapshot {
  readonly siteplan: Siteplan
  readonly geographicProjection: Readonly<{ state: 'unavailable'; reason: 'UNCALIBRATED_ILLUSTRATION' }>
}
export type SiteplanErrorCode = 'INVALID_CONTRACT' | 'IDENTITY_MISMATCH' | 'UNKNOWN_LANDMARK' | 'HASH_MISMATCH' | 'DIMENSION_MISMATCH' | 'NETWORK' | 'ABORTED' | 'STALE' | 'UNAVAILABLE'
export class SiteplanError extends Error {
  readonly code: SiteplanErrorCode
  constructor(code: SiteplanErrorCode, message: string = code) { super(message); this.name = 'SiteplanError'; this.code = code }
}
export function requireSiteplan(condition: unknown, code: SiteplanErrorCode = 'INVALID_CONTRACT'): asserts condition {
  if (!condition) throw new SiteplanError(code)
}
export function imagePoint(x: number, y: number): ImagePoint {
  requireSiteplan(Number.isFinite(x) && Number.isFinite(y))
  return Object.freeze({ x, y }) as ImagePoint
}
export function landmarkId(value: string): LandmarkId { requireSiteplan(validId(value)); return value as LandmarkId }
export function sameSiteplanIdentity(a: SiteplanIdentity, b: SiteplanIdentity): boolean {
  return a.venueId === b.venueId && a.siteplanId === b.siteplanId && a.revision === b.revision && a.variantId === b.variantId
}
function validId(value: unknown): value is string { return typeof value === 'string' && value.length <= 64 && /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/.test(value) }
function record(value: unknown, keys: readonly string[]): Record<string, unknown> {
  requireSiteplan(typeof value === 'object' && value !== null && !Array.isArray(value))
  requireSiteplan(Object.getPrototypeOf(value) === Object.prototype || Object.getPrototypeOf(value) === null)
  const row = value as Record<string, unknown>
  requireSiteplan(Object.keys(row).length === keys.length && keys.every(key => Object.hasOwn(row, key)))
  return row
}
function identifier(value: unknown): string { requireSiteplan(validId(value)); return value }
function integer(value: unknown, min: number, max: number): number {
  requireSiteplan(typeof value === 'number' && Number.isSafeInteger(value) && value >= min && value <= max); return value
}
function sha(value: unknown): string { requireSiteplan(typeof value === 'string' && /^[a-f0-9]{64}$/.test(value)); return value }
function array(value: unknown, max: number): unknown[] { requireSiteplan(Array.isArray(value) && value.length > 0 && value.length <= max); return value }
function unique(values: readonly string[]): void { requireSiteplan(new Set(values).size === values.length) }
export function decodeSiteplan(value: unknown): Siteplan {
  const row = record(value, ['schemaVersion', 'venueId', 'siteplanId', 'revision', 'variantId', 'sourcePixelCoordinateSpace', 'imageOrigin', 'image', 'sourceRefs', 'landmarks'])
  requireSiteplan(row.schemaVersion === 'paradise-city-siteplan-v1' && row.sourcePixelCoordinateSpace === 'image-px' && row.imageOrigin === 'top-left')
  const rawImage = record(row.image, ['path', 'mime', 'sha256', 'bytes', 'width', 'height'])
  requireSiteplan(typeof rawImage.path === 'string' && /^venue\/paradise-city\/v1\/[a-z0-9][a-z0-9.-]*\.jpe?g$/.test(rawImage.path) && !rawImage.path.includes('..') && rawImage.mime === 'image/jpeg')
  const image: SiteplanImage = Object.freeze({ path: rawImage.path, mime: 'image/jpeg', sha256: sha(rawImage.sha256), bytes: integer(rawImage.bytes, 1, 16 * 1024 * 1024), width: integer(rawImage.width, 1, 8192), height: integer(rawImage.height, 1, 8192) })
  const sourceRefs = Object.freeze(array(row.sourceRefs, 16).map(value => {
    const source = record(value, ['id', 'sha256', 'bytes'])
    return Object.freeze({ id: identifier(source.id), sha256: sha(source.sha256), bytes: integer(source.bytes, 1, 16 * 1024 * 1024) })
  }))
  unique(sourceRefs.map(source => source.id))
  requireSiteplan(sourceRefs.some(source => source.sha256 === image.sha256 && source.bytes === image.bytes))
  const landmarks = Object.freeze(array(row.landmarks, 128).map(value => {
    const raw = record(value, ['id', 'displayName', 'anchor', 'hitRegion', 'sourceRefs', 'review'])
    requireSiteplan(typeof raw.displayName === 'string' && raw.displayName.length >= 1 && raw.displayName.length <= 120 && raw.displayName.trim() === raw.displayName && !/[\u0000-\u001f\u007f]/.test(raw.displayName) && raw.review === 'ILLUSTRATED_UNCALIBRATED')
    const a = record(raw.anchor, ['x', 'y']), r = record(raw.hitRegion, ['x', 'y', 'width', 'height'])
    const anchor = imagePoint(integer(a.x, 0, image.width - 1), integer(a.y, 0, image.height - 1))
    const hitRegion = Object.freeze({ x: integer(r.x, 0, image.width - 1), y: integer(r.y, 0, image.height - 1), width: integer(r.width, 1, image.width), height: integer(r.height, 1, image.height) })
    requireSiteplan(hitRegion.x + hitRegion.width <= image.width && hitRegion.y + hitRegion.height <= image.height)
    requireSiteplan(anchor.x >= hitRegion.x && anchor.x < hitRegion.x + hitRegion.width && anchor.y >= hitRegion.y && anchor.y < hitRegion.y + hitRegion.height)
    const refs = Object.freeze(array(raw.sourceRefs, 16).map(identifier)); unique(refs)
    requireSiteplan(refs.every(id => sourceRefs.some(source => source.id === id)))
    return Object.freeze({ id: landmarkId(identifier(raw.id)), displayName: raw.displayName, anchor, hitRegion, sourceRefs: refs, review: 'ILLUSTRATED_UNCALIBRATED' as const })
  }))
  unique(landmarks.map(landmark => landmark.id))
  return Object.freeze({ schemaVersion: 'paradise-city-siteplan-v1', venueId: identifier(row.venueId), siteplanId: identifier(row.siteplanId), revision: integer(row.revision, 1, Number.MAX_SAFE_INTEGER), variantId: identifier(row.variantId), sourcePixelCoordinateSpace: 'image-px', imageOrigin: 'top-left', image, sourceRefs, landmarks })
}
