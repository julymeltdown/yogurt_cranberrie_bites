import type { SessionController } from '../../features/auth/model/session'
import type { PartyAccessPort, PartyController } from '../../features/party-discovery'
import type { GiftContextPort, GiftDiscoveryController } from '../../features/gift-discovery'
import type { QualifiedPlace } from '../../features/party-authoring'
import type { ManagementOptions } from '../../features/party-management'
import type { GiveawayAuthoringOptions } from '../../features/giveaway-authoring'
import type { GiftPickupOptions, IntentJournal } from '../../features/gift-pickup'
import type { MeetingClockPort } from '../meeting-entry/ports'
import type { PrivacyScope } from '../../shared/query/privacy'

export type CommunityEntryAuth = Pick<SessionController, 'session' | 'privacy' | 'request'>
/** Borrowed objects: their existing shell owner alone starts/disposes them. */
export interface CommunityControllers {
  readonly parties: PartyController
  readonly gifts: GiftDiscoveryController
}
export interface CommunityEntryRelease extends CommunityControllers {
  readonly scope: PrivacyScope
  readonly signal: AbortSignal
  readonly access: PartyAccessPort
  readonly context: GiftContextPort
  /** No wall-clock or physical-place fallback when these sources are absent. */
  readonly clock?: MeetingClockPort
  readonly places?: {
    read(): { readonly scope: PrivacyScope; readonly items: readonly QualifiedPlace[] } | null
    subscribe(listener: () => void): () => void
  }
  readonly rally?: (partyId: string) => ManagementOptions['authority']
  readonly pickupPlace?: (giveawayId?: string, reservationId?: string) => GiftPickupOptions['placeAuthority']
  readonly pickupMeeting?: (giveawayId?: string) => GiveawayAuthoringOptions['meeting']
  /** Exact scoped media port; the caller retains cancellation/disposal ownership. */
  readonly image?: (giveawayId?: string) => GiveawayAuthoringOptions['image']
  readonly journal?: IntentJournal
}
