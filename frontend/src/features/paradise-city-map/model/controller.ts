import type { LandmarkId } from '../spatial/contract'
import type { CapabilitySnapshot, CommandContext, CommandResult, LandmarkView, MeetingConsumerPort, NoteAudience, NoteExpiry,
  PersonView, PhoneLocationPort, PlaceNote, VenueAuthority, VenueCapabilities, VenueMapFocusIntent, VenueView } from '../ports'

export type Tab = 'MAP' | 'FRIENDS' | 'NOTES'
export type Selection = { readonly kind: 'friend'; readonly id: string } | { readonly kind: 'landmark'; readonly id: LandmarkId }
  | { readonly kind: 'notes'; readonly id: LandmarkId } | { readonly kind: 'cluster'; readonly ids: readonly string[] }
export interface Draft { readonly landmarkId: LandmarkId; readonly replyTo: string | null; readonly body: string; readonly audience: NoteAudience; readonly expiry: NoteExpiry }
export interface MapState {
  readonly tab: Tab; readonly selection: Selection | null; readonly draft: Draft | null
  readonly snapshot: CapabilitySnapshot | null; readonly people: readonly PersonView[]; readonly notes: readonly PlaceNote[]
  readonly busy: boolean; readonly uncertain: boolean; readonly notice: string; readonly disposed: boolean
  readonly meeting: ReturnType<MeetingConsumerPort['read']>; readonly phone: ReturnType<PhoneLocationPort['read']>
  readonly focusRequest: { readonly id: LandmarkId; readonly serial: number } | null
}
const sameVenue = (a: VenueView['identity'], b: VenueView['identity']) => a.venueId === b.venueId && a.siteplanId === b.siteplanId && a.revision === b.revision && a.variantId === b.variantId
const sameScope = (a: VenueAuthority['scope'], b: VenueAuthority['scope']) => a.userId === b.userId && a.eventId === b.eventId && a.generation === b.generation && a.policyGeneration === b.policyGeneration && a.gate === b.gate
const noPosition = (p: PersonView): PersonView => ({ ...p, point: null, landmarkId: null, updatedAtMs: null, freshUntilMs: null, locationSource: 'unavailable' })
export function locationLabel(person: PersonView, serverNowMs: number | null) {
  if (person.locationSource === 'unavailable' || person.updatedAtMs === null || serverNowMs === null) return 'Location unavailable'
  const elapsed = Math.max(0, Math.floor((serverNowMs - person.updatedAtMs) / 1000))
  const time = elapsed < 60 ? `${elapsed}s ago` : `${Math.floor(elapsed / 60)}m ago`
  return `${person.locationSource === 'phone' ? 'Phone location · updated' : person.locationSource === 'check-in' ? 'Check-in ·' : 'Last update ·'} ${time}`
}

/** One bounded view controller. All backend authority, clock, Auth and meeting owners are borrowed. */
export function createVenueMapController({ venue, authority, capabilities, meeting, phone }: {
  readonly venue: VenueView; readonly authority: VenueAuthority; readonly capabilities: VenueCapabilities
  readonly meeting: MeetingConsumerPort; readonly phone: PhoneLocationPort
}) {
  const listeners = new Set<() => void>(), stops: (() => void)[] = []
  let active = true, tab: Tab = 'MAP', selection: Selection | null = null, draft: Draft | null = null
  let busy = false, notice = '', uncertainKey: string | null = null, operation = 0, uploadGeneration = 0
  let localPause = false, pauseKey: string | null = null, pauseCommand: AbortController | null = null
  let command: AbortController | null = null, uploading: AbortController | null = null
  let wake: ReturnType<typeof setTimeout> | undefined
  let state: MapState
  let focusRequest: MapState['focusRequest'] = null, focusSerial = 0
  const landmark = (id: string) => venue.landmarks.find(p => p.id === id)
  const current = () => active && !authority.signal.aborted && authority.isCurrent()
  function readSnapshot(): CapabilitySnapshot | null {
    if (!current()) return null
    try {
      const value = capabilities.read()
      return value && sameScope(value.scope, authority.scope) && sameVenue(value.venue, venue.identity)
        && value.scope.gate === 'open' && value.serverNowMs !== null && Number.isFinite(value.serverNowMs)
        && value.accessUntilMs !== null && value.serverNowMs < value.accessUntilMs ? value : null
    } catch { return null }
  }
  function refresh() {
    clearTimeout(wake)
    const snapshot = readSnapshot(), now = snapshot?.serverNowMs ?? null
    const people = (snapshot?.friends ?? []).filter(p => p.visible).map(p =>
      now === null || p.updatedAtMs === null || p.updatedAtMs > now || p.freshUntilMs === null || p.freshUntilMs <= now ? { ...p, point: null, locationSource: p.updatedAtMs === null ? 'unavailable' as const : 'last-known' as const } : p)
    const notes = (snapshot?.notes ?? []).filter(n => landmark(n.landmarkId) && now !== null && n.createdAtMs <= now && n.expiresAtMs > now)
    if (!snapshot) {
      selection = current() && selection?.kind === 'landmark' && landmark(selection.id) ? selection : null
      if (!current()) focusRequest = null
      draft = null; command?.abort(); pauseCommand?.abort(); uploading?.abort(); uploadGeneration++; phone.stop(); busy = false; uncertainKey = null; pauseKey = null
    } else {
      const selectedFriend = selection?.kind === 'friend' ? selection.id : null
      if (selectedFriend && !people.some(p => p.userId === selectedFriend)) selection = null
      if (selection?.kind === 'cluster') {
        const ids = selection.ids.filter(id => people.some(p => p.userId === id && p.point))
        selection = ids.length ? { kind: 'cluster', ids } : null
      }
      if (draft && (!landmark(draft.landmarkId) || draft.replyTo && !notes.some(n => n.id === draft!.replyTo && n.canReply)
        || !draft.replyTo && !snapshot.rights.postNote.includes(draft.landmarkId))) draft = null
      if (snapshot.sharing !== 'active') { uploadGeneration++; uploading?.abort(); phone.stop() }
    }
    let borrowed: ReturnType<MeetingConsumerPort['read']> = null
    try { if (snapshot) borrowed = meeting.read() } catch { /* Missing borrowed leaf is unavailable. */ }
    state = { tab, selection, draft, snapshot, people, notes, busy, uncertain: uncertainKey !== null || pauseKey !== null, notice, disposed: !active,
      meeting: borrowed, phone: phone.read(), focusRequest }
    if (snapshot && now !== null) {
      const deadlines = [snapshot.accessUntilMs, snapshot.self?.freshUntilMs, ...snapshot.friends.map(p => p.freshUntilMs), ...snapshot.notes.map(n => n.expiresAtMs)]
        .filter((value): value is number => value !== null && value !== undefined && Number.isFinite(value) && value > now)
      if (deadlines.length) wake = setTimeout(refresh, Math.min(2147483647, Math.max(1, Math.min(...deadlines) - now)))
    }
    for (const listener of listeners) listener()
  }
  function invalidate() { if (!current()) { notice = 'Current venue access is unavailable.'; refresh() } else refresh() }
  function context(key: string, signal: AbortSignal): CommandContext { return { key, scope: authority.scope, venue: venue.identity, signal } }
  async function run(work: (ctx: CommandContext) => Promise<CommandResult>, success: string, onApplied?: () => void) {
    if (busy || uncertainKey || pauseKey || !readSnapshot()) { if (!readSnapshot()) { notice = 'This action is unavailable until current access is verified.'; refresh() }; return }
    const version = ++operation, key = crypto.randomUUID(), controller = new AbortController(); command = controller; busy = true; notice = ''; refresh()
    try {
      const result = await work(context(key, controller.signal))
      if (!current() || controller.signal.aborted || version !== operation || !readSnapshot()) return
      if (result.state === 'unknown') { uncertainKey = key; notice = 'The result is unknown. Check its status before another action.' }
      else if (result.state === 'rejected') notice = 'The current server permission did not allow this action.'
      else { notice = success; onApplied?.() }
    } catch {
      if (current() && !controller.signal.aborted && version === operation) { uncertainKey = key; notice = 'The result is unknown. Check its status before another action.' }
    } finally { if (version === operation) { busy = false; command = null; refresh() } }
  }
  function stopPhone() { uploadGeneration++; uploading?.abort(); uploading = null; phone.stop() }
  async function pause(reason: 'user' | 'background' | 'withdrawal' = 'user') {
    localPause = true; stopPhone(); refresh()
    if (!readSnapshot()?.rights.pause) { notice = 'Phone location is stopped on this device.'; refresh(); return }
    if (pauseCommand) return
    const controller = new AbortController(), key = crypto.randomUUID(); pauseCommand = controller
    try {
      const result = await capabilities.pause(reason, context(key, controller.signal))
      if (!current() || controller.signal.aborted) return
      if (result.state === 'unknown') pauseKey = key
      notice = result.state === 'applied' ? 'Location sharing paused.' : 'Phone location stopped on this device. Server pause is unconfirmed.'
    } catch { if (current() && !controller.signal.aborted) { pauseKey = key; notice = 'Phone location stopped on this device. Check the server pause status.' } }
    finally { if (pauseCommand === controller) pauseCommand = null; refresh() }
  }
  function watch() {
    const generation = ++uploadGeneration
    phone.start(fix => {
      const snap = readSnapshot()
      if (!current() || localPause || generation !== uploadGeneration || snap?.sharing !== 'active' || !snap.rights.sharePhone || uploading) return
      const controller = new AbortController(); uploading = controller
      void capabilities.publishPhoneFix(fix, context(crypto.randomUUID(), controller.signal)).then(result => {
        if (!controller.signal.aborted && generation === uploadGeneration && result.state !== 'applied') { stopPhone(); notice = 'Location upload is unavailable. Sharing stopped on this device.'; refresh() }
      }).catch(() => { if (!controller.signal.aborted && generation === uploadGeneration) { stopPhone(); notice = 'Location upload is unavailable. Sharing stopped on this device.'; refresh() } })
        .finally(() => { if (uploading === controller) uploading = null })
    }, reason => { stopPhone(); notice = reason === 'denied' ? 'Phone location permission was denied.' : 'Phone location stopped.'; refresh(); if (reason === 'background') void pause('background') })
    refresh()
  }
  function dispose() {
    if (!active) return
    active = false; clearTimeout(wake); operation++; command?.abort(); pauseCommand?.abort(); stopPhone(); selection = null; draft = null; busy = false; uncertainKey = null; pauseKey = null
    for (const stop of stops.splice(0)) stop()
    authority.signal.removeEventListener('abort', invalidate); refresh(); listeners.clear()
  }
  refresh()
  stops.push(capabilities.subscribe(refresh), meeting.subscribe(refresh), phone.subscribe(refresh), authority.subscribe(invalidate))
  authority.signal.addEventListener('abort', invalidate, { once: true })
  if (!current()) invalidate()
  return {
    venue, read: () => state, subscribe(listener: () => void) { listeners.add(listener); return () => { listeners.delete(listener) } }, dispose,
    focus(intent: VenueMapFocusIntent) {
      const place = landmark(intent.landmarkId)
      if (!current() || !place || !sameScope(intent.scope, authority.scope) || !sameVenue(intent.venue, venue.identity) || state.meeting?.locked || busy || uncertainKey || pauseKey) return false
      tab = 'MAP'; selection = { kind: 'landmark', id: place.id }; draft = null; focusRequest = { id: place.id, serial: ++focusSerial }; refresh(); return true
    },
    setTab(value: Tab) { if (state.meeting?.locked || busy || uncertainKey || pauseKey) return; tab = value; selection = null; draft = null; focusRequest = null; refresh() },
    select(value: Selection | null) { if (busy || uncertainKey || pauseKey || state.meeting?.locked) return; selection = value; draft = null; focusRequest = null; refresh() },
    edit(body: string) { if (draft && !busy && !uncertainKey) { draft = { ...draft, body: body.slice(0, 1000) }; refresh() } },
    setAudience(audience: NoteAudience) { if (draft && state.snapshot?.rights.noteAudiences.includes(audience) && !busy && !uncertainKey) { draft = { ...draft, audience }; refresh() } },
    setExpiry(expiry: NoteExpiry) { if (draft && state.snapshot?.rights.noteExpiries.includes(expiry) && !busy && !uncertainKey) { draft = { ...draft, expiry }; refresh() } },
    compose(id: string, replyTo: string | null = null) {
      const snap = readSnapshot(), note = state.notes.find(n => n.id === replyTo), place = landmark(id)
      if (!snap || !place || busy || uncertainKey || pauseKey || (replyTo ? !note?.canReply || note.landmarkId !== place.id : !snap.rights.postNote.includes(place.id))) return
      tab = 'NOTES'; selection = null; draft = { landmarkId: place.id, replyTo, body: '', audience: snap.rights.noteAudiences[0] ?? 'FRIENDS', expiry: snap.rights.noteExpiries[0] ?? 'ONE_HOUR' }; refresh()
    },
    changeDraftPlace(id: string) { const place = landmark(id); if (place && draft && !draft.replyTo && !busy && !uncertainKey && !pauseKey && state.snapshot?.rights.postNote.includes(place.id)) { draft = { ...draft, landmarkId: place.id }; refresh() } },
    cancelDraft() { if (!busy && !uncertainKey) { draft = null; refresh() } },
    async submit() {
      const captured = draft, snap = readSnapshot()
      if (!captured || !captured.body.trim() || !snap) return
      if (captured.replyTo) {
        if (!state.notes.some(n => n.id === captured.replyTo && n.canReply)) return
        await run(ctx => capabilities.reply({ noteId: captured.replyTo!, body: captured.body.trim() }, ctx), 'Reply posted.', () => { draft = null })
      } else if (snap.rights.postNote.includes(captured.landmarkId) && snap.rights.noteAudiences.includes(captured.audience) && snap.rights.noteExpiries.includes(captured.expiry)) {
        await run(ctx => capabilities.postNote({ landmarkId: captured.landmarkId, body: captured.body.trim(), audience: captured.audience, expiry: captured.expiry }, ctx), 'Note posted.', () => { draft = null })
      }
    },
    pause,
    async startPhone(consented: boolean) {
      if (!consented || !readSnapshot()?.rights.sharePhone) return
      await run(ctx => capabilities.beginPhoneSharing(ctx), 'Sharing permission confirmed.', () => { if (readSnapshot()?.sharing === 'active') { localPause = false; watch() } })
    },
    async checkIn(id: string) { const place = landmark(id); if (place && readSnapshot()?.rights.checkIn.includes(place.id)) await run(ctx => capabilities.checkIn(place.id, ctx), 'Check-in confirmed.') },
    async requestLocation(id: string) { if (state.people.some(p => p.userId === id && p.canRequest)) await run(ctx => capabilities.requestLocation(id, ctx), 'Location request sent.') },
    async openMeeting(action: 'INVITE' | 'MESSAGE', person: Pick<PersonView, 'userId'>, place: LandmarkView | undefined, origin: 'map-friend' | 'map-landmark' | 'map-note') {
      const snap = readSnapshot(), fixed = place ? landmark(place.id) : undefined
      const association = fixed?.meetingContext ?? undefined
      const allowed = snap?.friends.some(p => p.visible && p.userId === person.userId && (action === 'INVITE' ? p.canMeet : p.canMessage))
        || origin === 'map-note' && action === 'INVITE' && state.notes.some(n => n.author.userId === person.userId && n.landmarkId === fixed?.id && n.canMeet)
      if (!snap || busy || uncertainKey || pauseKey || state.meeting?.locked || action === 'INVITE' && !association
        || !allowed) { notice = 'This meeting context is unavailable.'; refresh(); return }
      try {
        const common = { scope: authority.scope, venue: venue.identity, targetUserId: person.userId, origin }
        const result = await meeting.open(action === 'INVITE'
          ? { ...common, action, ...association! }
          : { ...common, action, ...(association ? { landmark: association } : {}) })
        if (current()) { notice = result.kind === 'opened' ? '' : result.kind === 'stale' ? 'The selected context changed. Select it again.' : 'Meeting and messages are unavailable for this context.'; refresh() }
      } catch { if (current()) { notice = 'Meeting and messages are unavailable for this context.'; refresh() } }
    },
    closeMeeting() { return meeting.close() },
    async resolve() {
      if ((!uncertainKey && !pauseKey) || busy || !readSnapshot()) return
      const recoveringPause = pauseKey !== null, key = pauseKey ?? uncertainKey!, controller = new AbortController(); command = controller; busy = true; refresh()
      try {
        const result = await capabilities.resolve(key, context(key, controller.signal))
        if (!current() || controller.signal.aborted || (recoveringPause ? pauseKey : uncertainKey) !== key) return
        if (result.state !== 'unknown') { if (recoveringPause) pauseKey = null; else { uncertainKey = null; if (result.state === 'applied') draft = null }; notice = result.state === 'applied' ? 'The server confirmed the action.' : 'The server rejected the action. Your draft is retained.' }
      } catch { /* Keep the original key and locked draft; never automatically resend. */ }
      finally { busy = false; command = null; refresh() }
    },
    self() { const snap = state.snapshot, person = snap?.self; return !localPause && snap?.sharing === 'active' && person && snap.serverNowMs !== null && person.updatedAtMs !== null && person.updatedAtMs <= snap.serverNowMs && person.freshUntilMs !== null && person.freshUntilMs > snap.serverNowMs ? person : person ? noPosition(person) : null },
  }
}
export type VenueMapController = ReturnType<typeof createVenueMapController>
