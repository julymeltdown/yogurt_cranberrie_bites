import type { MeetingOpenIntent, MeetingOpenResult } from '../../features/paradise-city-map/ports'
import { sameSiteplanIdentity } from '../../features/paradise-city-map/spatial/contract'
import { commandUnresolved, emptyMeetingLeaf, type LeafCommand, type LeafCommandKind,
  type LeafSurface, type MeetingContextView, type MeetingLeafState } from '../../features/venue-invitations/model'
import type { PrivacyScope } from '../../shared/query/privacy'
import { canonicalId } from '../routes'
import type { LeafCommandResult, MeetingLeafOptions } from './ports'

const sameScope = (a: PrivacyScope, b: PrivacyScope) => a.userId === b.userId && a.eventId === b.eventId
  && a.generation === b.generation && a.policyGeneration === b.policyGeneration && a.gate === b.gate
const association = (intent: MeetingOpenIntent) => intent.action === 'INVITE' ? intent : intent.landmark
const sameRecipient = (a: MeetingOpenIntent, b: MeetingOpenIntent) => sameScope(a.scope, b.scope)
  && sameSiteplanIdentity(a.venue, b.venue) && a.targetUserId === b.targetUserId
const sameSelection = (a: MeetingOpenIntent, b: MeetingOpenIntent) => sameRecipient(a, b)
  && association(a)?.landmarkId === association(b)?.landmarkId
  && association(a)?.catalogRevision === association(b)?.catalogRevision
  && association(a)?.landmarkRevision === association(b)?.landmarkRevision

/** Retain once per shell release, outside Map and detail route lifetimes. No HTTP implementation. */
export function createMeetingLeafOwner({ auth, release, newCommandKey }: MeetingLeafOptions) {
  let state = emptyMeetingLeaf, disposed = false, withdrawn = false, serial = 0
  let selection: MeetingOpenIntent | null = null
  let flight: Promise<MeetingOpenResult> | null = null
  let expiry: ReturnType<typeof setTimeout> | undefined
  const listeners = new Set<() => void>(), stops: (() => void)[] = []
  const scope = release ? Object.freeze({ ...release.scope }) : null
  const request = release && scope ? Object.freeze({ scope, signal: release.signal }) : null
  function notify() {
    for (const listener of Array.from(listeners)) { try { listener() } catch { /* Notify all owners, including on withdrawal. */ } }
  }
  function publish(patch: Partial<MeetingLeafState>) { state = Object.freeze({ ...state, ...patch }); notify() }
  function live() {
    return !disposed && !withdrawn && !!auth && !!release && !!scope && !!request && !request.signal.aborted
      && scope.gate === 'open' && canonicalId(scope.userId ?? '') && canonicalId(scope.eventId ?? '')
      && auth.session.isCurrent(scope) && auth.privacy.isCurrent(scope)
  }
  function now(): number | null {
    if (!live()) return null
    try {
      const clock = release!.clock.read()
      return clock && sameScope(clock.scope, scope!) && Number.isFinite(clock.nowMs) && clock.nowMs >= 0 && live() ? clock.nowMs : null
    } catch { return null }
  }
  function withdraw() {
    if (withdrawn) return
    withdrawn = true; ++serial; flight = null; selection = null
    clearTimeout(expiry); expiry = undefined
    // Scrub before abort/port cleanup/subscriber callbacks can re-enter the owner.
    state = Object.freeze({ ...emptyMeetingLeaf, phase: 'withdrawn' })
    try { release?.adapter.withdraw?.() } catch { /* Owner state is already scrubbed. */ }
    request?.signal.removeEventListener('abort', withdraw)
    for (const stop of stops.splice(0).reverse()) { try { stop() } catch { /* No source can retain private data. */ } }
    notify()
  }
  function usable() {
    if (!live() || now() === null) { withdraw(); return false }
    if (state.context && now()! >= state.context.validUntilMs) { withdraw(); return false }
    return true
  }
  function armExpiry() {
    clearTimeout(expiry); expiry = undefined
    if (!state.context || !usable()) return
    // Scheduling is only a wakeup. Authority comes from the borrowed qualified clock.
    const invitation = state.context.invitation
    if (invitation && now()! >= invitation.expiresAtMs && state.context.invitationActions.length) {
      // Withdraw expired action capabilities; keep independent conversation permission.
      publish({ context: Object.freeze({ ...state.context, invitationActions: [] }) })
      if (!usable() || !state.context) return
    }
    const inviteBoundary = invitation && state.context.invitationActions.length && invitation.expiresAtMs > now()!
      ? invitation.expiresAtMs : state.context.validUntilMs
    const delay = Math.max(1, Math.min(state.context.validUntilMs - now()!, inviteBoundary - now()!, 2_147_483_647))
    expiry = setTimeout(() => { if (usable()) armExpiry() }, delay)
  }
  function register(make: () => () => void) {
    if (withdrawn) return
    const stop = make(); if (withdrawn) stop(); else stops.push(stop)
  }
  function validContext(value: MeetingContextView | null, intent: MeetingOpenIntent | null, id: string | null) {
    if (!value || !scope || !value.canRead || !sameScope(value.selection.scope, scope)
      || value.self.userId !== scope.userId || value.friend.userId !== value.selection.targetUserId
      || value.friend.userId === scope.userId || !canonicalId(value.friend.userId)
      || !value.contextId || !value.contextVersion
      || (value.selection.action === 'INVITE' && (!association(value.selection)?.landmarkId
        || !association(value.selection)?.catalogRevision || !association(value.selection)?.landmarkRevision))
      || !Number.isFinite(value.validUntilMs) || now() === null || value.validUntilMs <= now()!
      || (intent && !(sameSelection(value.selection, intent)
        || (intent.action === 'MESSAGE' && !association(value.selection) && sameRecipient(value.selection, intent))))
      || (id && value.invitation?.id !== id)) return false
    if (value.invitation && (!canonicalId(value.invitation.id) || !value.invitation.version
      || !Number.isFinite(value.invitation.expiresAtMs))) return false
    const ids = new Set<string>()
    for (const message of value.messages) {
      if (!message.id || ids.has(message.id) || (message.authorId !== value.self.userId && message.authorId !== value.friend.userId)) return false
      ids.add(message.id)
    }
    return true
  }
  function mappedLandmark(context: MeetingContextView) {
    const place = association(context.selection), venue = release?.mapVenue
    if (!place || !venue || !sameSiteplanIdentity(venue.identity, context.selection.venue)) return null
    const matches = venue.landmarks.filter(item => item.meetingContext?.landmarkId === place.landmarkId
      && item.meetingContext.catalogRevision === place.catalogRevision && item.meetingContext.landmarkRevision === place.landmarkRevision)
    return matches.length === 1 ? matches[0]!.id : null
  }
  async function load(intent: MeetingOpenIntent | null, id: string | null): Promise<MeetingOpenResult> {
    if (!usable() || !request || !release) return { kind: 'unavailable' }
    if (commandUnresolved(state.command)) return { kind: 'unavailable' }
    const attempt = ++serial
    selection = intent
    clearTimeout(expiry)
    // No previous target, message or draft appears during a new authorized read.
    publish({ phase: 'loading', active: true, context: null, canViewOnMap: false, draft: '', command: null, notice: null, requestedInvitationId: id })
    if (!usable() || attempt !== serial) return { kind: 'stale' }
    try {
      const value = id ? await release.adapter.readInvitation(id, request) : await release.adapter.readSelection(intent!, request)
      if (!usable() || attempt !== serial) return { kind: 'stale' }
      if (!validContext(value, intent, id)) { publish({ phase: 'error', context: null, notice: 'unavailable' }); return { kind: 'unavailable' } }
      selection = value!.selection
      publish({ phase: 'ready', context: value, canViewOnMap: !!release.mapFocus && !!mappedLandmark(value!) }); armExpiry()
      return { kind: 'opened' }
    } catch {
      if (!usable() || attempt !== serial) return { kind: 'stale' }
      publish({ phase: 'error', context: null, notice: 'unavailable' }); return { kind: 'unavailable' }
    }
  }
  function locked() { return commandUnresolved(state.command) }
  function surface(next: LeafSurface, compose = next === 'conversation') {
    if (!usable()) return false
    if (next === 'invitation' && state.context && !association(state.context.selection) && !state.context.invitation) return false
    publish({ surface: next, composerOpen: compose, active: true }); return true
  }
  function beginLoad(intent: MeetingOpenIntent | null, id: string | null) {
    selection = intent
    state = Object.freeze({ ...state, requestedInvitationId: id })
    // Reserve before any notification or adapter call can re-enter open().
    const pending = Promise.resolve().then(() => load(intent, id)); flight = pending
    void pending.finally(() => { if (flight === pending) flight = null })
    return pending
  }
  function open(intent: MeetingOpenIntent): Promise<MeetingOpenResult> {
    if (!usable() || !scope || !sameScope(intent.scope, scope) || !canonicalId(intent.targetUserId)
      || intent.targetUserId === scope.userId) return Promise.resolve({ kind: 'unavailable' })
    const place = association(intent)
    if (intent.action === 'INVITE' && (!place?.landmarkId || !place.catalogRevision || !place.landmarkRevision)) return Promise.resolve({ kind: 'unavailable' })
    if (selection && sameSelection(selection, intent)) {
      surface(intent.action === 'MESSAGE' ? 'conversation' : 'invitation')
      if (flight) return flight
      if (state.phase === 'ready') return Promise.resolve({ kind: 'opened' })
      if (locked()) return Promise.resolve({ kind: 'unavailable' })
    } else if (locked()) return Promise.resolve({ kind: 'unavailable' })
    state = Object.freeze({ ...state, surface: intent.action === 'MESSAGE' ? 'conversation' : 'invitation', composerOpen: intent.action === 'MESSAGE' })
    return beginLoad(intent, null)
  }
  function openInvitation(id: string, next: LeafSurface): Promise<MeetingOpenResult> {
    if (!usable() || !canonicalId(id)) return Promise.resolve({ kind: 'unavailable' })
    if (state.context?.invitation?.id === id || state.requestedInvitationId === id) {
      surface(next, false)
      if (flight) return flight
      if (state.phase === 'ready') return Promise.resolve({ kind: 'opened' })
    }
    if (locked()) return Promise.resolve({ kind: 'unavailable' })
    state = Object.freeze({ ...state, surface: next, composerOpen: false })
    return beginLoad(null, id)
  }
  async function settle(command: LeafCommand, result: LeafCommandResult) {
    if (!usable() || state.command?.command !== command) return
    if (result.state === 'unknown') { publish({ command: { command, phase: 'unknown' }, notice: 'unknown' }); return }
    const context = state.context
    publish({ phase: 'loading', context: null, canViewOnMap: false, command: { command, phase: result.state }, notice: result.state === 'rejected' ? 'rejected' : null,
      ...(result.state === 'applied' && command.kind === 'send' ? { draft: '' } : {}) })
    if (!usable() || !context) return
    // Refresh only after a definitive outcome. Never synthesize messages/status/rights.
    const attempt = serial
    try {
      const updated = await release!.adapter.readSelection(context.selection, request!)
      if (!usable() || serial !== attempt || state.command?.command !== command) return
      if (!validContext(updated, context.selection, null) || updated!.contextId !== command.contextId) { withdraw(); return }
      publish({ phase: 'ready', context: updated, canViewOnMap: !!release!.mapFocus && !!mappedLandmark(updated!) }); armExpiry()
    } catch {
      if (usable() && serial === attempt && state.command?.command === command) publish({ phase: 'error', notice: 'unavailable' })
    }
  }
  async function execute(kind: LeafCommandKind) {
    if (!usable() || locked() || !state.context || state.phase !== 'ready' || !request || !release) return false
    const context = state.context
    if (kind === 'send') {
      let accepts = false
      try { accepts = context.canSend && context.messagePolicy.accepts(state.draft) } catch { /* Fail closed. */ }
      if (!accepts) { publish({ notice: 'invalid-message' }); return false }
    } else if (!context.invitationActions.includes(kind) || (kind === 'create' && !association(context.selection))
      || (context.invitation && now()! >= context.invitation.expiresAtMs)) return false
    let key: string
    try { key = newCommandKey() } catch { return false }
    if (!canonicalId(key)) return false
    const command: LeafCommand = Object.freeze({ key, kind, contextId: context.contextId, contextVersion: context.contextVersion,
      invitationId: context.invitation?.id ?? null, invitationVersion: context.invitation?.version ?? null,
      body: kind === 'send' ? state.draft : null, selection: context.selection })
    // Install original identity before notifying callbacks; re-entrant sends cannot duplicate it.
    publish({ command: { command, phase: 'pending' }, notice: null })
    if (!usable() || state.command?.command !== command) return false
    try { await settle(command, await release.adapter.execute(command, request)) }
    catch { if (usable() && state.command?.command === command) publish({ command: { command, phase: 'unknown' }, notice: 'unknown' }) }
    return true
  }
  async function resolve() {
    if (!usable() || state.command?.phase !== 'unknown' || !request || !release) return false
    const command = state.command.command
    publish({ command: { command, phase: 'checking' }, notice: null })
    if (!usable() || state.command?.command !== command) return false
    try { await settle(command, await release.adapter.status(command, request)) }
    catch { if (usable() && state.command?.command === command) publish({ command: { command, phase: 'unknown' }, notice: 'unknown' }) }
    return true
  }
  async function loadMore() {
    if (!usable() || locked() || state.phase !== 'ready' || !state.context?.conversation?.nextCursor
      || !release?.adapter.loadMore || !request) return false
    const context = state.context, attempt = ++serial
    publish({ phase: 'loading' })
    if (!usable() || serial !== attempt) return false
    try {
      const updated = await release.adapter.loadMore(context, request)
      if (!usable() || serial !== attempt) return false
      if (!validContext(updated, context.selection, null) || updated!.contextId !== context.contextId) { withdraw(); return false }
      publish({ phase: 'ready', context: updated, canViewOnMap: !!release.mapFocus && !!mappedLandmark(updated!) }); armExpiry()
      return true
    } catch { if (usable() && serial === attempt) withdraw(); return false }
  }
  function viewOnMap() {
    if (!usable() || locked() || !state.context || !release?.mapFocus) return false
    const artLandmarkId = mappedLandmark(state.context)
    if (!artLandmarkId) return false
    try {
      const focused = release.mapFocus.focus({ scope: scope!, venue: state.context.selection.venue, landmarkId: artLandmarkId })
      if (!focused || !usable() || locked()) return false
      publish({ active: false })
      if (!usable() || locked()) return false
      release.onMapFocused?.(); return true
    } catch { if (usable()) publish({ notice: 'unavailable' }); return false }
  }
  if (live() && now() !== null) {
    try {
      register(() => auth!.privacy.registerCleanup(withdraw))
      register(() => release!.invalidations.subscribe(withdraw))
      register(() => release!.clock.subscribe(() => { if (usable()) armExpiry() }))
      if (usable()) request!.signal.addEventListener('abort', withdraw, { once: true })
    } catch { withdraw() }
  }
  return {
    read: () => { if (state.phase !== 'idle' && !withdrawn) usable(); return state },
    subscribe(listener: () => void) { if (disposed) return () => {}; listeners.add(listener); return () => { listeners.delete(listener) } },
    open, openInvitation, loadMore, surface, execute, resolve, viewOnMap,
    setDraft(draft: string) { if (usable() && state.context && !locked()) publish({ draft, notice: null }) },
    close() { if (!usable() || locked()) return false; publish({ active: false }); return true },
    refresh() { if (!usable() || locked() || (!selection && !state.requestedInvitationId)) return Promise.resolve({ kind: 'unavailable' } as const)
      return beginLoad(selection, state.requestedInvitationId) },
    withdraw,
    dispose() { if (disposed) return; withdraw(); disposed = true; listeners.clear() },
  }
}
export type MeetingLeafOwner = ReturnType<typeof createMeetingLeafOwner>
