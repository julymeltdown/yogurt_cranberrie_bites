import type { ReactNode } from 'react'
import type { MeetingOpenIntent } from '../paradise-city-map/ports'

/** Presentation projections, NOT wire DTOs. Root's frozen client owns decoding. */
export interface MeetingParticipant {
  readonly userId: string
  readonly displayName: string
  /** Approved, scope-qualified portrait supplied by the shell; never an arbitrary URL. */
  readonly portrait: ReactNode
}
export type InvitationDecision = 'accept' | 'decline' | 'cancel'
export type InvitationAction = 'create' | InvitationDecision
export type InvitationStatus = 'pending' | 'accepted' | 'declined' | 'cancelled' | 'expired'
export interface InvitationView {
  readonly id: string
  readonly version: string
  readonly status: InvitationStatus
  readonly expiresAtMs: number
  /** Exact decision rights supplied by the producer for this revision. */
  readonly allowedActions: readonly InvitationDecision[]
}
export interface MeetingPairPresentation {
  readonly self: MeetingParticipant
  readonly friend: MeetingParticipant
  readonly landmarkLabel: string | null
  readonly landmarkDescription: string | null
}
export type InboxDirection = 'INCOMING' | 'OUTGOING'
export interface InvitationInboxItem {
  readonly invitation: InvitationView
  readonly target: MeetingOpenIntent | null
  readonly presentation: MeetingPairPresentation | null
}
export interface InvitationInboxPage {
  readonly items: readonly InvitationInboxItem[]
  readonly nextCursor: string | null
  /** Producer-supplied party capacity only. Null means the producer did not supply it. */
  readonly capacity: { readonly limit: number; readonly joined: number } | null
}
export interface InvitationInboxView {
  readonly direction: InboxDirection
  readonly phase: 'idle' | 'loading' | 'ready' | 'error'
  readonly items: readonly (InvitationInboxItem & { readonly direction: InboxDirection })[]
  readonly nextCursor: string | null
  readonly notice: 'rejected' | 'denied' | 'conflict' | 'unavailable' | 'unknown' | null
  readonly capacity: { readonly limit: number; readonly joined: number } | null
}
export const emptyInvitationInbox: InvitationInboxView = Object.freeze({ direction: 'INCOMING', phase: 'idle',
  items: [], nextCursor: null, notice: null, capacity: null })
/** The approved W04 party-invitation policy this client surfaces. It is copy, not authority. */
export const W04_INVITATION_POLICY = Object.freeze({ inviteeConsent: true, inviterGrantsPerParty: 1,
  capacity: 8, expiryHours: 24, hostRevoke: true })
export const invitationExpired = (invitation: InvitationView, nowMs: number | null) => invitation.status === 'pending'
  && nowMs !== null && Number.isFinite(nowMs) && Number.isFinite(invitation.expiresAtMs) && invitation.expiresAtMs <= nowMs
export interface MeetingMessage {
  readonly id: string
  readonly authorId: string
  readonly body: string
  readonly timeLabel: string
}
export interface MeetingContextView {
  readonly contextId: string
  readonly contextVersion: string
  readonly selection: MeetingOpenIntent
  /** Optional MESSAGE association; INVITE always requires a reviewed real place. */
  readonly landmarkLabel: string | null
  readonly landmarkDescription: string | null
  readonly self: MeetingParticipant
  readonly friend: MeetingParticipant
  readonly validUntilMs: number
  readonly canRead: boolean
  readonly canSend: boolean
  /** Current server capabilities, never inferred from invitation status. */
  readonly invitationActions: readonly InvitationAction[]
  readonly invitation: InvitationView | null
  /** A complete authorized display window in producer order, deduplicated by the adapter. */
  readonly messages: readonly MeetingMessage[]
  /** Only an actual server message associated with the current reviewed invitation. */
  readonly sharedPlaceMessageId?: string | null
  readonly conversation: { readonly id: string; readonly earlierCursor: string | null; readonly nextCursor?: string | null } | null
  /** Adapter translates the actual frozen server length/unit policy. No guessed UI limit. */
  readonly messagePolicy: { readonly hint: string; readonly accepts: (body: string) => boolean }
}
export type LeafSurface = 'invitation' | 'conversation'
export type LeafCommandKind = InvitationAction | 'send'
export interface LeafCommand {
  readonly key: string
  readonly kind: LeafCommandKind
  readonly contextId: string
  readonly contextVersion: string
  readonly invitationId: string | null
  readonly invitationVersion: string | null
  readonly body: string | null
  /** Exact original selected friend/provider tuple for status reads after uncertainty. */
  readonly selection: MeetingOpenIntent
}
export interface CommandView {
  readonly command: LeafCommand
  readonly phase: 'pending' | 'unknown' | 'checking' | 'applied' | 'rejected'
}
export interface MeetingLeafState {
  readonly phase: 'idle' | 'loading' | 'ready' | 'error' | 'withdrawn'
  readonly active: boolean
  readonly surface: LeafSurface
  readonly composerOpen: boolean
  readonly canViewOnMap: boolean
  readonly context: MeetingContextView | null
  readonly draft: string
  readonly command: CommandView | null
  readonly notice: 'unavailable' | 'changed' | 'invalid-message' | 'rejected' | 'denied' | 'conflict' | 'unknown' | null
  readonly requestedInvitationId: string | null
  /** Qualified server clock reading used for truthful expiry labels. */
  readonly nowMs: number | null
  readonly inbox: InvitationInboxView
}
export const emptyMeetingLeaf: MeetingLeafState = Object.freeze({ phase: 'idle', active: false,
  surface: 'invitation', composerOpen: false, canViewOnMap: false, context: null, draft: '', command: null, notice: null,
  requestedInvitationId: null, nowMs: null, inbox: emptyInvitationInbox })
export const commandUnresolved = (command: CommandView | null) => !!command
  && (command.phase === 'pending' || command.phase === 'unknown' || command.phase === 'checking')
