import type { ReactNode } from 'react'
import type { MeetingOpenIntent } from '../paradise-city-map/ports'

/** Presentation projections, NOT wire DTOs. Root's frozen client owns decoding. */
export interface MeetingParticipant {
  readonly userId: string
  readonly displayName: string
  /** Approved, scope-qualified portrait supplied by the shell; never an arbitrary URL. */
  readonly portrait: ReactNode
}
export type InvitationAction = 'create' | 'accept' | 'decline' | 'cancel'
export interface InvitationView {
  readonly id: string
  readonly version: string
  readonly status: 'pending' | 'accepted' | 'declined' | 'cancelled' | 'expired'
  readonly expiresAtMs: number
}
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
  readonly notice: 'unavailable' | 'changed' | 'invalid-message' | 'rejected' | 'unknown' | null
  readonly requestedInvitationId: string | null
}
export const emptyMeetingLeaf: MeetingLeafState = Object.freeze({ phase: 'idle', active: false,
  surface: 'invitation', composerOpen: false, canViewOnMap: false, context: null, draft: '', command: null, notice: null, requestedInvitationId: null })
export const commandUnresolved = (command: CommandView | null) => !!command
  && (command.phase === 'pending' || command.phase === 'unknown' || command.phase === 'checking')
