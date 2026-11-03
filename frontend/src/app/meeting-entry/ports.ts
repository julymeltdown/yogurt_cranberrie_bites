import type { SessionController } from '../../features/auth/model/session'
import type { PlacesController } from '../../features/places/model/controller'
import type { MeetingAuthorityPort, MeetingPhotoPort } from '../../features/meet-beacon'
import type { PrivacyScope } from '../../shared/query/privacy'

export type MeetingEntryAuth = Pick<SessionController, 'session' | 'privacy' | 'request'>
export interface MeetingClockPort {
  /** A server-time estimate maintained by its existing owner using a monotonic anchor. */
  read(): { readonly scope: PrivacyScope; readonly nowMs: number } | null
  /** Notify withdrawal/replacement. FE20 also reads this estimate at expiry boundaries. */
  subscribe(listener: () => void): () => void
}
export interface MeetingInvalidation {
  readonly scope: PrivacyScope; readonly beaconId: string; readonly revision: number
  readonly state?: 'EXPIRED' | 'REVOKED'
}
/** Prepared by the owning shell from current qualified sources; no production source is invented here. */
export interface MeetingEntryRelease {
  readonly scope: PrivacyScope
  readonly signal: AbortSignal
  readonly places: Pick<PlacesController, 'read' | 'subscribe'>
  readonly authority: MeetingAuthorityPort
  readonly clock: MeetingClockPort
  readonly photos?: MeetingPhotoPort
  readonly invalidations?: { subscribe(listener: (event: MeetingInvalidation) => void): () => void }
}
