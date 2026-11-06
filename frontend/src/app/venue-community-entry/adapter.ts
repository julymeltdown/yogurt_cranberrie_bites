import { createMeetingClient, meetingRequests, meetingSchemas, MeetingClientError,
  type MeetingContext, type MeetingSelection, type MeetingInvitationSelection, type MeetingInvitation,
  type MeetingMessage, type MeetingRequest, type MeetingDecision } from '../../shared/api/paradise-city-v1/meeting'
import type { PrivacyScope } from '../../shared/query/privacy'
import type { MeetingOpenIntent } from '../../features/paradise-city-map/ports'
import type { InboxDirection, InvitationDecision, InvitationInboxPage, InvitationStatus, InvitationView,
  LeafCommand, MeetingContextView, MeetingPairPresentation } from '../../features/venue-invitations/model'
import type { LeafRequest, LeafCommandResult, MeetingLeafAdapter } from './ports'

export type MeetingPresentation = MeetingPairPresentation
export interface MeetingAdapterOptions {
  readonly client: ReturnType<typeof createMeetingClient> | null
  readonly scope: PrivacyScope
  readonly signal: AbortSignal
  isCurrent(scope: PrivacyScope): boolean
  now(): number | null
  /** Canonical privacy cleanup registration; this owns no second privacy coordinator. */
  registerCleanup(withdraw: () => void): () => void
  /** Actual qualified provider metadata supplies NUMERIC wire revisions. Never parse ART IDs/revisions. */
  selectionFor(intent: MeetingOpenIntent): MeetingSelection | null
  /** Actual scoped venue/provider mapping for an authorized invitation-list row. */
  intentFor(selection: MeetingInvitationSelection): MeetingOpenIntent | null
  presentationFor(intent: MeetingOpenIntent): MeetingPresentation | null
  /** Optional original authorized navigation target; a bare ID is insufficient. */
  invitationTarget?(id: string): MeetingOpenIntent | null
}
export type MeetingInboxView = InvitationInboxPage
const equalScope = (a: PrivacyScope, b: PrivacyScope) => a.generation === b.generation && a.userId === b.userId
  && a.eventId === b.eventId && a.policyGeneration === b.policyGeneration && a.gate === b.gate
const sameWire = (a: MeetingSelection, b: MeetingSelection) => a.selectedFriendId === b.selectedFriendId
  && (a.landmark == null ? b.landmark == null : b.landmark != null && a.landmark.landmarkId === b.landmark.landmarkId
    && a.landmark.catalogRevision === b.landmark.catalogRevision && a.landmark.landmarkRevision === b.landmark.landmarkRevision)
const version = (dto: MeetingContext) => JSON.stringify([dto.relationshipRevision, dto.policyRevision])
const statuses: readonly InvitationStatus[] = ['pending', 'accepted', 'declined', 'cancelled', 'expired']
const viewInvitation = (dto: MeetingInvitation, nowMs: number): InvitationView => {
  const status = dto.state.toLowerCase() as InvitationStatus, expiresAtMs = Date.parse(dto.expiresAt)
  if (!statuses.includes(status) || !Number.isFinite(expiresAtMs)) throw new MeetingClientError('INVALID_RESPONSE')
  const allowedActions = dto.allowedInvitationActions.map(action => action.toLowerCase() as InvitationDecision)
  return { id: dto.id, version: String(dto.revision), status, expiresAtMs,
    allowedActions: expiresAtMs <= nowMs ? [] : allowedActions }
}
const decisions: Readonly<Record<'accept' | 'decline' | 'cancel', MeetingDecision>> = { accept: 'ACCEPT', decline: 'DECLINE', cancel: 'CANCEL' }
const MAX_WINDOW = 200
function rejection(error: unknown): LeafCommandResult {
  if (!(error instanceof MeetingClientError) || error.uncertain) return { state: 'unknown' }
  if (error.code === 'PERMISSION_DENIED') return { state: 'rejected', reason: 'denied' }
  if (error.code === 'REVISION_OR_KEY_CONFLICT') return { state: 'rejected', reason: 'conflict' }
  if (error.code === 'RESOURCE_UNAVAILABLE') return { state: 'rejected', reason: 'unavailable' }
  return { state: 'rejected' }
}

/** Own normalized projection only. Root's actual shared client owns all descriptors/decoding/transport. */
export function createNormalizedMeetingAdapter(options: MeetingAdapterOptions) {
  const scope = Object.freeze({ ...options.scope }), signal = options.signal
  let disposed = false, serial = 0
  let latest: { intent: MeetingOpenIntent; selected: MeetingSelection; dto: MeetingContext } | null = null
  let window: { contextId: string; conversationId: string | null; cursor: string; more: boolean; messages: readonly MeetingMessage[] } | null = null
  let selectedInvitation: { selected: MeetingSelection; id: string } | null = null
  let pending: { command: LeafCommand; selected: MeetingSelection; operation: string } | null = null
  const targets = new Map<string, MeetingOpenIntent>()
  function clear() { serial++; latest = null; window = null; selectedInvitation = null; pending = null; targets.clear() }
  function live(request: LeafRequest) {
    try { const now = options.now(); return now !== null && Number.isFinite(now) && !disposed && !signal.aborted && request.signal === signal && equalScope(request.scope, scope)
      && scope.gate === 'open' && scope.userId !== null && scope.eventId !== null && options.isCurrent(scope) } catch { return false }
  }
  function select(intent: MeetingOpenIntent): MeetingSelection | null {
    if (!equalScope(intent.scope, scope)) return null
    const supplied = options.selectionFor(intent)
    const parsed = meetingSchemas.Selection.safeParse(supplied)
    if (!parsed.success || parsed.data.selectedFriendId !== intent.targetUserId) return null
    const expected = intent.action === 'INVITE' ? intent : intent.landmark
    // Compare actual numeric producer revisions in their canonical display form;
    // no string revision or illustrated revision is converted into a wire number.
    if (expected ? !parsed.data.landmark || parsed.data.landmark.landmarkId !== expected.landmarkId
      || String(parsed.data.landmark.catalogRevision) !== expected.catalogRevision
      || String(parsed.data.landmark.landmarkRevision) !== expected.landmarkRevision : parsed.data.landmark != null) return null
    return parsed.data
  }
  async function executeRequest<T>(descriptor: MeetingRequest<T>, request: LeafRequest): Promise<T> {
    if (!live(request) || !options.client) throw new MeetingClientError('DEPENDENCY_UNAVAILABLE')
    const result = await options.client.execute(descriptor)
    if (!live(request) || result.scope.userId !== scope.userId || result.scope.eventId !== scope.eventId
      || result.scope.generation !== scope.generation) throw new MeetingClientError('STALE_SCOPE')
    return result.data
  }
  async function load(intent: MeetingOpenIntent, request: LeafRequest, invitationId: string | null): Promise<MeetingContextView | null> {
    if (!live(request)) { clear(); return null }
    const selected = select(intent)
    if (!selected) return null
    const token = ++serial
    if (!latest || !sameWire(latest.selected, selected)) { window = null; selectedInvitation = null; latest = null }
    const dto = await executeRequest(meetingRequests.resolveContext(scope.eventId!, selected), request)
    if (!live(request) || token !== serial) return null
    if (!dto.canRead) { clear(); return null }
    const presentation = options.presentationFor(intent)
    if (!presentation || presentation.self.userId !== scope.userId || presentation.friend.userId !== selected.selectedFriendId
      || (selected.landmark == null && (presentation.landmarkLabel !== null || presentation.landmarkDescription !== null))) return null
    if (!window || window.contextId !== dto.contextId || window.conversationId !== dto.conversationId) window = null
    const after = window?.cursor ?? '0'
    // Each explicit load reads at most one ascending page. No polling or auto-pagination.
    const page = await executeRequest(meetingRequests.listMessages(scope.eventId!, selected, after, dto.contextId), request)
    if (!live(request) || token !== serial) return null
    if (page.conversationId !== dto.conversationId || Number(page.cursor) < Number(after)
      || (page.hasMore && Number(page.cursor) <= Number(after))) return null
    const previous = page.gap ? [] : window?.messages ?? []
    const byId = new Map(previous.map(message => [message.id, message])), bySequence = new Map(previous.map(message => [message.sequence, message.id]))
    for (const item of page.items) {
      if (item.senderId !== scope.userId && item.senderId !== selected.selectedFriendId) return null
      const old = byId.get(item.id)
      if ((old && JSON.stringify(old) !== JSON.stringify(item)) || (bySequence.has(item.sequence) && bySequence.get(item.sequence) !== item.id)) return null
      if (!old && item.sequence <= Number(after)) return null
      byId.set(item.id, item); bySequence.set(item.sequence, item.id)
    }
    const now = options.now()
    if (now === null || !Number.isFinite(now)) return null
    const messages = [...byId.values()].filter(message => Date.parse(message.expiresAt) > now).sort((a, b) => a.sequence - b.sequence).slice(-MAX_WINDOW)
    let invitation: MeetingInvitation | null = null
    const id = invitationId ?? (selectedInvitation && sameWire(selectedInvitation.selected, selected) ? selectedInvitation.id : null)
    if (id) {
      const required = meetingSchemas.InvitationSelection.safeParse(selected)
      if (!required.success) return null
      invitation = await executeRequest(meetingRequests.getInvitation(scope.eventId!, id, required.data), request)
    }
    if (!live(request) || token !== serial) return null
    latest = { intent, selected, dto }
    window = { contextId: dto.contextId, conversationId: page.conversationId, cursor: page.cursor, more: page.hasMore, messages }
    if (invitation) selectedInvitation = { selected, id: invitation.id }
    const format = new Intl.DateTimeFormat('en', { timeZone: dto.eventTimezone, hour: 'numeric', minute: '2-digit' })
    const actions = invitation ? invitation.allowedInvitationActions.map(action => action.toLowerCase() as 'accept' | 'decline' | 'cancel')
      : dto.canInvite && dto.invitationExpiresAt !== null && Date.parse(dto.invitationExpiresAt) > now ? ['create' as const] : []
    return { contextId: dto.contextId, contextVersion: version(dto), selection: intent, ...presentation,
      validUntilMs: Math.min(Date.parse(dto.validUntil), Date.parse(dto.contentExpiresAt)), canRead: dto.canRead, canSend: dto.canSend,
      invitationActions: invitation && Date.parse(invitation.expiresAt) <= now ? [] : actions,
      invitation: invitation ? viewInvitation(invitation, now) : null,
      messages: messages.map(message => ({ id: message.id, authorId: message.senderId, body: message.body, timeLabel: format.format(Date.parse(message.createdAt)) })),
      sharedPlaceMessageId: invitation && selected.landmark ? messages.find(message => message.invitationId === invitation.id)?.id ?? null : null,
      conversation: page.conversationId === null ? null : { id: page.conversationId,
        earlierCursor: messages[0] && messages[0].sequence > 1 ? String(messages[0].sequence - 1) : null,
        nextCursor: page.hasMore ? page.cursor : null },
      messagePolicy: { hint: 'Up to 2,000 characters.', accepts: body => meetingSchemas.SendMessage.safeParse({ selection: selected, body }).success },
    }
  }
  async function guardedLoad(intent: MeetingOpenIntent, request: LeafRequest, id: string | null) {
    const work = load(intent, request, id), token = serial
    try { const value = await work; if (value === null && token === serial) clear(); return value }
    catch { if (token === serial) clear(); return null }
  }
  const adapter: MeetingLeafAdapter = {
    withdraw: clear,
    readSelection: (intent, request) => guardedLoad(intent, request, null),
    async readInvitation(id, request) {
      if (!live(request)) { clear(); return null }
      const intent = targets.get(id) ?? options.invitationTarget?.(id)
      return intent ? guardedLoad(intent, request, id) : null
    },
    async execute(command, request): Promise<LeafCommandResult> {
      if (!live(request) || !latest || latest.dto.contextId !== command.contextId || version(latest.dto) !== command.contextVersion) return { state: 'rejected' }
      if (pending?.command.key === command.key) return { state: 'unknown' } // Never re-execute an observed key.
      const selected = select(command.selection)
      if (!selected || !sameWire(selected, latest.selected)) return { state: 'rejected' }
      let descriptor: MeetingRequest<unknown>
      try {
        if (command.kind === 'send') {
          if (!latest.dto.canSend || command.body === null) return { state: 'rejected' }
          descriptor = meetingRequests.sendMessage(scope.eventId!, command.key, { selection: selected, body: command.body, invitationId: command.invitationId })
        } else {
          const required = meetingSchemas.InvitationSelection.parse(selected)
          if (command.kind === 'create') {
            if (!latest.dto.canInvite) return { state: 'rejected' }
            descriptor = meetingRequests.createInvitation(scope.eventId!, command.key, { selection: required })
          } else {
            if (command.invitationId === null || command.invitationVersion === null || !selectedInvitation || selectedInvitation.id !== command.invitationId) return { state: 'rejected' }
            // Use only the exact current server integer retained from its detail response.
            const current = await executeRequest(meetingRequests.getInvitation(scope.eventId!, command.invitationId, required), request)
            if (String(current.revision) !== command.invitationVersion || !current.allowedInvitationActions.includes(decisions[command.kind])) return { state: 'rejected' }
            descriptor = meetingRequests.decideInvitation(scope.eventId!, current.id, command.key, { selection: required, expectedRevision: current.revision, decision: decisions[command.kind] })
          }
        }
      } catch (error) {
        return error instanceof MeetingClientError ? rejection(error) : { state: 'rejected' }
      }
      pending = { command, selected, operation: descriptor.operationId }
      try {
        const result = await executeRequest(descriptor, request)
        if (meetingSchemas.AppliedReceipt.safeParse(result).success) {
          const receipt = meetingSchemas.AppliedReceipt.parse(result)
          if (receipt.result.kind === 'INVITATION') selectedInvitation = { selected, id: receipt.result.invitation.id }
          return { state: 'applied' }
        }
        return { state: 'unknown' }
      } catch (error) { return rejection(error) }
    },
    async status(command, request) {
      const original = pending
      if (!live(request) || !original || original.command !== command || original.command.key !== command.key) return { state: 'unknown' }
      try {
        const result = await executeRequest(meetingRequests.getCommand(scope.eventId!, command.key, original.selected), request)
        if (result.state !== 'APPLIED' || result.operation !== original.operation) return { state: 'unknown' }
        if (result.result.kind === 'INVITATION') {
          if (command.kind !== 'create' && result.result.invitation.id !== command.invitationId) return { state: 'unknown' }
          selectedInvitation = { selected: original.selected, id: result.result.invitation.id }
        }
        return { state: 'applied' }
      } catch { return { state: 'unknown' } }
    },
    loadMore(context, request) { return guardedLoad(context.selection, request, context.invitation?.id ?? null) },
    async listInvitations(direction, cursor, request) {
      if (!live(request)) { clear(); return null }
      const page = await executeRequest(meetingRequests.listInvitations(scope.eventId!, direction, cursor), request)
      if (!live(request)) return null
      const now = options.now()
      if (now === null || !Number.isFinite(now)) return null
      const items = page.items.map(row => {
        const selected = meetingSchemas.InvitationSelection.parse(row.selection), intent = options.intentFor(selected)
        const target = intent && select(intent) && sameWire(select(intent)!, selected) ? intent : null
        if (target) { if (targets.size >= 100) targets.delete(targets.keys().next().value!); targets.set(row.invitation.id, target) }
        return { invitation: viewInvitation(row.invitation, now), target, presentation: target ? options.presentationFor(target) : null }
      })
      if (!live(request)) { clear(); return null }
      // VENU-BE-M01 carries no party capacity. The client must not invent it.
      return { items, nextCursor: page.nextCursor, capacity: null }
    },
  }
  const stopCleanup = options.registerCleanup(clear)
  signal.addEventListener('abort', clear, { once: true })
  return {
    adapter,
    listInvitations: (direction: InboxDirection, cursor: string | undefined, request: LeafRequest) => adapter.listInvitations(direction, cursor, request),
    dispose() { if (disposed) return; disposed = true; clear(); stopCleanup(); signal.removeEventListener('abort', clear) },
  }
}
