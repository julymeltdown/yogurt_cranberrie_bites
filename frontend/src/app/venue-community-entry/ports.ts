import type { MeetingOpenIntent, VenueMapFocusPort, VenueView } from '../../features/paradise-city-map/ports'
import type { LeafCommand, MeetingContextView } from '../../features/venue-invitations/model'
import type { PrivacyScope } from '../../shared/query/privacy'
import type { MeetingClockPort, MeetingEntryAuth } from '../meeting-entry/ports'

export interface LeafRequest {
  readonly scope: PrivacyScope
  /** The original shell-owned signal. Closing a route does not replace it. */
  readonly signal: AbortSignal
}
export type LeafCommandResult =
  | { readonly state: 'unknown' }
  | { readonly state: 'rejected' }
  | { readonly state: 'applied' }
/** Internal normalized adapter seam. No URLs, wire schema or transport implementation here. */
export interface MeetingLeafAdapter {
  readSelection(intent: MeetingOpenIntent, request: LeafRequest): Promise<MeetingContextView | null>
  readInvitation(invitationId: string, request: LeafRequest): Promise<MeetingContextView | null>
  execute(command: LeafCommand, request: LeafRequest): Promise<LeafCommandResult>
  /** Read status only; MUST NOT re-execute an unknown command. */
  status(command: LeafCommand, request: LeafRequest): Promise<LeafCommandResult>
}
export interface MeetingLeafRelease {
  readonly scope: PrivacyScope
  readonly signal: AbortSignal
  readonly clock: MeetingClockPort
  readonly adapter: MeetingLeafAdapter
  readonly classification: 'AUTHORITATIVE_ADAPTER' | 'CONTROLLED_FIXTURE'
  /** Every invalidation means withdraw first. A replacement requires a new shell release. */
  readonly invalidations: { subscribe(withdraw: () => void): () => void }
  /** Borrow the sole Map focus type. Root keeps this proxy identity stable. */
  readonly mapFocus?: VenueMapFocusPort
  /** Same qualified Map view; its explicit real-place associations map to art IDs. */
  readonly mapVenue?: VenueView
  /** Root-owned actual navigation after a successful Map selection/camera intent. */
  readonly onMapFocused?: () => void
}
export interface MeetingLeafOptions {
  readonly auth: Pick<MeetingEntryAuth, 'session' | 'privacy'> | null
  readonly release: MeetingLeafRelease | null
  /** Explicit command key factory. Only called once, on a user command. */
  readonly newCommandKey: () => string
}
