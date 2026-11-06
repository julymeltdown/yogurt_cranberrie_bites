import type { ReactNode } from 'react'
import type { PrivacyScope } from '../../shared/query/privacy'
import type { SiteplanIdentity, LandmarkId, SourceReference } from './spatial/contract'

/** Consumer projections and injected capabilities, never HTTP DTOs or endpoint authority. */
export interface ReadPort<T> { read(): T; subscribe(listener: () => void): () => void }
export interface MapPoint { readonly x: number; readonly y: number }
export interface LandmarkView {
  readonly id: LandmarkId; readonly label: string; readonly point: MapPoint
  readonly illustration?: { readonly sourceRefs: readonly SourceReference[]; readonly review: 'ILLUSTRATED_UNCALIBRATED' }
  /** Current real-place provider association, never inferred from the illustrated ID. */
  readonly meetingContext: MeetingLandmarkContext | null
}
export interface VenueView {
  readonly identity: SiteplanIdentity; readonly title: string
  readonly image: { readonly src: string; readonly width: number; readonly height: number; readonly description: string }
  readonly landmarks: readonly LandmarkView[]
  readonly classification: 'ILLUSTRATED_UNCALIBRATED'
}
export interface PersonView {
  readonly userId: string; readonly displayName: string; readonly avatar: unknown
  readonly point: MapPoint | null; readonly landmarkId: LandmarkId | null
  readonly locationSource: 'phone' | 'check-in' | 'last-known' | 'unavailable'
  readonly updatedAtMs: number | null; readonly freshUntilMs: number | null
  readonly visible: boolean; readonly canRequest: boolean; readonly canMessage: boolean; readonly canMeet: boolean
}
export type NoteAudience = 'FRIENDS' | 'EVERYONE'
export type NoteExpiry = 'ONE_HOUR' | 'END_OF_DAY'
export interface PlaceReply {
  readonly id: string; readonly author: Pick<PersonView, 'userId' | 'displayName' | 'avatar'>
  readonly body: string; readonly createdAtMs: number
}
export interface PlaceNote extends PlaceReply {
  readonly landmarkId: LandmarkId; readonly audience: NoteAudience; readonly expiresAtMs: number
  readonly revision: number; readonly replyCount: number
  readonly canReply: boolean; readonly canMeet: boolean; readonly canWithdraw: boolean
  readonly replies: readonly PlaceReply[]
}
/** Current authorized server count for one place. Geometry never invents a marker. */
export interface NoteCluster { readonly landmarkId: LandmarkId; readonly count: number }
export type SharingState = 'off' | 'active' | 'paused' | 'revoked'
/** Why the current authoritative friends snapshot cannot be shown as current. */
export type VenueUnavailableReason = 'friend-gate-unavailable' | 'producer-refused' | 'offline' | 'access-unavailable'
export interface VenueAvailability {
  readonly state: 'ready' | 'stale' | 'unavailable'
  /** Non-null when the last read failed or the snapshot outlived policy freshness. */
  readonly reason: VenueUnavailableReason | null
  /** Server-clock instant of the last successful snapshot read. */
  readonly readAtMs: number | null
  /** Server-clock age of that snapshot; never computed from the device clock. */
  readonly ageMs: number | null
  /** Server-clock instant at which the snapshot stops being current. */
  readonly staleAtMs: number | null
  readonly refreshing: boolean
  readonly canRefresh: boolean
}
export interface VenueLocationRequest {
  readonly id: string; readonly requesterId: string; readonly subjectId: string
  readonly revision: number; readonly expiresAtMs: number
}
export interface CapabilitySnapshot {
  readonly scope: PrivacyScope; readonly venue: SiteplanIdentity
  readonly classification: 'AUTHORITATIVE_ADAPTER' | 'CONTROLLED_FIXTURE'
  /** Qualified server clock and current access expiry; never filled from Date.now(). */
  readonly serverNowMs: number | null; readonly accessUntilMs: number | null
  readonly self: PersonView | null; readonly friends: readonly PersonView[]; readonly notes: readonly PlaceNote[]
  /** Server-authorized per-landmark counts; absent rows are not guessed from loaded pages. */
  readonly noteClusters: readonly NoteCluster[]; readonly notesVisibleCount: number | null
  readonly sharing: SharingState
  /** Current pending requests only, independently withdrawn with their L lease. */
  readonly locationRequests?: { readonly incoming: readonly VenueLocationRequest[]; readonly outgoing: readonly VenueLocationRequest[] }
  readonly rights: {
    readonly sharePhone: boolean; readonly pause: boolean; readonly revoke: boolean
    readonly checkIn: readonly LandmarkId[]; readonly postNote: readonly LandmarkId[]
    readonly noteAudiences: readonly NoteAudience[]; readonly noteExpiries: readonly NoteExpiry[]
  }
}
export interface GeoFix {
  readonly latitude: number; readonly longitude: number; readonly accuracy: number; readonly capturedAtMs: number
}
export type CommandResult = { readonly state: 'applied' | 'rejected' | 'unknown' }
export interface CommandContext { readonly key: string; readonly scope: PrivacyScope; readonly venue: SiteplanIdentity; readonly signal: AbortSignal }
export type VenueReadContext = Omit<CommandContext, 'key'>
export interface VenueCapabilities extends ReadPort<CapabilitySnapshot | null> {
  loadNotes?(landmarkId: LandmarkId, context: VenueReadContext): Promise<boolean>
  loadNoteThread?(noteId: string, context: VenueReadContext): Promise<boolean>
  /** Truthful freshness/availability of the Location owner; absent means unavailable. */
  availability?(): VenueAvailability
  /** Re-reads the authoritative snapshot; never reuses a prior disclosure. */
  refresh?(): Promise<boolean>
  loadNoteSummary?(context: VenueReadContext): Promise<boolean>
  beginPhoneSharing(context: CommandContext): Promise<CommandResult>
  publishPhoneFix(fix: GeoFix, context: CommandContext): Promise<CommandResult>
  pause(reason: 'user' | 'background' | 'withdrawal', context: CommandContext): Promise<CommandResult>
  revoke?(context: CommandContext): Promise<CommandResult>
  requestLocation(userId: string, context: CommandContext): Promise<CommandResult>
  decideLocationRequest?(id: string, decision: 'APPROVE' | 'DENY' | 'CANCEL', context: CommandContext): Promise<CommandResult>
  checkIn(landmarkId: LandmarkId, context: CommandContext): Promise<CommandResult>
  postNote(input: { readonly landmarkId: LandmarkId; readonly body: string; readonly audience: NoteAudience; readonly expiry: NoteExpiry }, context: CommandContext): Promise<CommandResult>
  reply(input: { readonly noteId: string; readonly body: string }, context: CommandContext): Promise<CommandResult>
  withdraw(input: { readonly noteId: string; readonly expectedRevision: number }, context: CommandContext): Promise<CommandResult>
  resolve(key: string, context: CommandContext): Promise<CommandResult>
}
/** SINGLE primary-surface meeting entry contract. Developer204 borrows these types. */
interface MeetingOpenContext {
  readonly scope: PrivacyScope; readonly venue: SiteplanIdentity; readonly targetUserId: string
  readonly origin: 'map-friend' | 'map-landmark' | 'map-note' | 'people-profile' | 'notification'
}
/** Reviewed real-place selection supplied by the authoritative adapter, not art identity. */
export interface MeetingLandmarkContext { readonly landmarkId: string; readonly catalogRevision: string; readonly landmarkRevision: string }
export type MeetingOpenIntent = MeetingOpenContext & (
  | ({ readonly action: 'INVITE' } & MeetingLandmarkContext)
  /** Current authorized event/friend context suffices. Association never grants permission. */
  | { readonly action: 'MESSAGE'; readonly landmark?: MeetingLandmarkContext }
)
export type MeetingOpenResult = { readonly kind: 'opened' | 'unavailable' | 'stale' }
export interface MeetingLeafSnapshot { readonly active: boolean; readonly locked: boolean; readonly content: ReactNode }
export interface MeetingConsumerPort extends ReadPort<MeetingLeafSnapshot | null> {
  /** Opens the separately owned leaf. Never creates/sends/accepts a meeting or message. */
  open(intent: MeetingOpenIntent): Promise<MeetingOpenResult>
  close(): boolean
}
export interface VenueMapFocusIntent {
  readonly scope: PrivacyScope; readonly venue: SiteplanIdentity; readonly landmarkId: LandmarkId
}
/** Camera/selection intent only. Root retains actual route navigation. */
export interface VenueMapFocusPort { focus(intent: VenueMapFocusIntent): boolean }
export type PhoneStatus = 'idle' | 'requesting' | 'watching' | 'paused' | 'denied' | 'unavailable'
export interface PhoneLocationPort extends ReadPort<PhoneStatus> {
  /** Call ONLY from an explicit user action, after current server sharing permission. */
  start(onFix: (fix: GeoFix) => void, onWithdraw: (reason: 'background' | 'denied' | 'unavailable') => void): void
  stop(): void
  dispose(): void
}
export interface VenueAuthority {
  readonly scope: PrivacyScope; readonly signal: AbortSignal
  isCurrent(): boolean
  subscribe(withdraw: () => void): () => void
}
