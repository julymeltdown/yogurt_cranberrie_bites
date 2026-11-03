import type { SessionController } from '../../features/auth/model/session'
import type { MuralAuthorityPort, MuralBinding, Clock } from '../../features/mural/sync'
import type { PrivacyScope } from '../../shared/query/privacy'
import type { MuralScenePort } from '../../world/mural'

export type MuralEntryAuth = Pick<SessionController, 'session' | 'privacy' | 'request'>
/** Supplied by the incumbent world after admission, scene/anchor and write-policy qualification.
 * This is an in-process capability, not a bootstrap field or public wire message.
 * Scene binding is exact. A new room/epoch/stream needs a replacement release.
 */
export interface MuralEntryRelease {
  readonly scope: PrivacyScope
  readonly signal: AbortSignal
  readonly binding: MuralBinding
  readonly authority: MuralAuthorityPort
  readonly scene: MuralScenePort & Required<Pick<MuralScenePort, 'onClose'>>
}
export interface MuralEntryOptions {
  readonly auth: MuralEntryAuth
  readonly scope: PrivacyScope
  /** Monotonic elapsed time for sync deadlines and cooldowns. */
  readonly clock?: Clock
  /** UTC epoch milliseconds for authenticated command-receipt expiry. */
  readonly receiptNow?: () => number
}
