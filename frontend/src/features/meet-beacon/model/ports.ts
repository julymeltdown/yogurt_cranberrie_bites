import type { SessionController } from '../../auth/model/session'
import type { PlacesController } from '../../places/model/controller'
import type { PrivacyScope } from '../../../shared/query/privacy'
import type { Audience } from './contract'
import type { MeetingPhotoConfiguration } from './photo-client'

/** Supplied by the owner after current server policy checks. This is not a new endpoint or a policy implementation. */
export interface MeetingAuthority {
  readonly scope: PrivacyScope
  readonly expiresAt: string
  readonly canRead: boolean
  readonly canWrite: boolean
  readonly maxTtlSeconds: number
  readonly eventCutoff: string
  readonly audiences: readonly { readonly id: string; readonly label: string; readonly audience: Audience }[]
  /** FE19's catalog revision is NOT a per-place revision. Missing qualified revisions disable authoring. */
  readonly placeRevisions: readonly { readonly placeId: string; readonly revision: number }[]
}
export interface MeetingAuthorityPort { read(): MeetingAuthority | null; subscribe(listener: () => void): () => void }
export interface QualifiedMeetingPhoto {
  readonly scope: PrivacyScope; readonly beaconId: string; readonly beaconRevision: number; readonly placeId: string
  readonly audienceKey: string; readonly expiresAt: string; readonly permission: 'GRANTED'; readonly review: 'REVIEWED'
  readonly src: string; readonly alt: string
  /** Present only for authenticated metadata+binary Blob display. */
  readonly mediaId?: string
}
/** Optional already-authorized display only. No upload/attachment exists in the pinned beacon DTO. */
export interface MeetingPhotoPort { read(): QualifiedMeetingPhoto | null; subscribe(listener: () => void): () => void }
export interface MeetingBeaconOptions {
  readonly auth: Pick<SessionController, 'session' | 'privacy' | 'request'>
  readonly places: Pick<PlacesController, 'read' | 'subscribe'>
  readonly authority: MeetingAuthorityPort
  /** Server-time estimate anchored to a monotonic clock, scoped by the composition owner; null when unqualified. */
  readonly now: () => number | null
  readonly photos?: MeetingPhotoPort
  /** Agreed photo upload/attachment contract; absent preserves text-only behavior. */
  readonly photoClient?: MeetingPhotoConfiguration
  readonly newKey?: () => string
}
