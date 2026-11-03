import type { SessionController } from '../../../auth/model/session'
import type { PrivacyScope } from '../../../../shared/query/privacy'
import type { Place, SetPlace } from '../../places/contract'

export type OperationsAuth = Pick<SessionController, 'session' | 'privacy' | 'request'>
/** Qualified externally by the server role owner. Not a role-query DTO or a bootstrap projection. */
export interface OperatorLease {
  readonly scope: PrivacyScope
  readonly actorId: string
  readonly eventId: string
  readonly role: 'OPERATOR' | 'MODERATOR' | 'MEMBER'
  readonly revision: number
  readonly checkedAt: string
  readonly expiresAt: string
}
export interface OperatorAuthorityPort { read(): OperatorLease | null; subscribe(listener: () => void): () => void }
export interface OperationsClockPort {
  /** Current server-time estimate anchored to a monotonic clock; no wall-clock fallback. */
  read(): { readonly scope: PrivacyScope; readonly nowMs: number } | null
  subscribe(listener: () => void): () => void
}
/** Delivery/BE08's explicit current-map/current-state write seam, absent by default.
 * Closure-only edits preserve the current immutable provenance. Other edits must
 * exactly match an independently reviewed new-release place supplied by its owner.
 */
export interface PlaceWriteRelease {
  readonly scope: PrivacyScope
  readonly mapRevision: number
  readonly expiresAt: string
  readonly targets: readonly { readonly placeId: string; readonly expectedRevision: number; readonly reviewedPlace: Place | null }[]
}
export interface PlaceWritePort {
  read(): PlaceWriteRelease | null
  subscribe(listener: () => void): () => void
  /** Adapter uses canonical Auth, frozen setConcertPlace and the actual owning service.
   * No default transport or invented endpoint is installed by this feature.
   */
  write(input: { readonly scope: PrivacyScope; readonly mapRevision: number; readonly placeId: string; readonly body: SetPlace;
    readonly key: string; readonly signal: AbortSignal }): Promise<{ readonly scope: PrivacyScope; readonly status: 200 | 202; readonly data: unknown }>
}
export interface OperationsOptions {
  readonly auth: OperationsAuth
  readonly authority: OperatorAuthorityPort | null
  readonly clock: OperationsClockPort | null
  readonly places?: PlaceWritePort
  readonly newKey?: () => string
}
