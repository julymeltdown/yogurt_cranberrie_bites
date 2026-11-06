import type { MeetingClockPort } from '../meeting-entry/ports'
import type {
  CapabilitySnapshot, CommandContext, CommandResult, ReadPort, VenueAuthority,
  VenueCapabilities, VenueView, PlaceNote as ViewNote,
} from '../../features/paradise-city-map/ports'
import { sameSiteplanIdentity, type LandmarkId } from '../../features/paradise-city-map/spatial/contract'
import type { PrivacyScope } from '../../shared/query/privacy'
import {
  buildNotesRequest, NotesApiError, type createNotesClient, type NoteCommand, type NoteLandmarkSelection,
  type NotePage, type NoteThread, type PlaceNote,
} from '../../shared/api/paradise-city-v1/notes'
import { notesSchemas } from '../../shared/api/paradise-city-v1/notes.generated'
import { decodeWire, utcMilliseconds } from '../../shared/api/paradise-city-v1/schema'

/** A projection of the existing primary port, suitable for the aggregate owner's merge. */
export type VenueNotesSnapshot = Pick<CapabilitySnapshot,
  'scope' | 'venue' | 'classification' | 'serverNowMs' | 'accessUntilMs' | 'notes' | 'noteClusters' | 'notesVisibleCount'> & {
  readonly rights: Pick<CapabilitySnapshot['rights'], 'postNote' | 'noteAudiences' | 'noteExpiries'>
}
type Lease = Pick<CapabilitySnapshot, 'scope' | 'venue' | 'accessUntilMs'>
/** Supplied by the reviewed physical provider; ART IDs never stand in for physical IDs. */
export interface ReviewedNoteAssociations extends Lease {
  readonly bindings: readonly {
    readonly landmarkId: LandmarkId
    readonly selection: NoteLandmarkSelection
  }[]
}
export type NoteCreationRights = Lease & Pick<VenueNotesSnapshot, 'rights'>
export interface NoteStatusBinding {
  readonly operationId: NoteCommand['operation_id']
  readonly key: string
  readonly scope: PrivacyScope
  readonly venue: CapabilitySnapshot['venue']
  readonly selection: NoteLandmarkSelection
  readonly parent: Pick<PlaceNote, 'id' | 'revision' | 'audience' | 'expires_at'> | null
  readonly receipt: NoteCommand | null
}
/** Root owns authenticated same-actor/event routing; the N receipt has no identity echo. */
export interface NoteStatusProducer {
  read(binding: NoteStatusBinding, context: Pick<CommandContext, 'scope' | 'signal'>): Promise<NoteCommand>
}
export interface VenueNotesOptions {
  readonly authority: VenueAuthority
  readonly venue: VenueView
  readonly clock: MeetingClockPort
  readonly client: ReturnType<typeof createNotesClient> | null
  readonly associations: ReadPort<ReviewedNoteAssociations | null>
  readonly creationRights: ReadPort<NoteCreationRights | null> | null
  readonly status: NoteStatusProducer | null
  readonly classification: CapabilitySnapshot['classification']
}
type Commands = Pick<VenueCapabilities, 'postNote' | 'reply' | 'withdraw' | 'resolve'>
export interface VenueNotesAdapter extends Commands, ReadPort<VenueNotesSnapshot | null> {
  ownsCommand(key: string): boolean
  load(landmarkId: LandmarkId, signal: AbortSignal): Promise<boolean>
  loadThread(noteId: string, signal: AbortSignal): Promise<boolean>
  loadSummary(signal: AbortSignal): Promise<boolean>
  dispose(): void
}
interface ClusterRow { readonly landmarkId: LandmarkId; readonly selection: NoteLandmarkSelection
  readonly count: number; readonly readAtMs: number; readonly refreshAtMs: number | null }
const unknown: CommandResult = Object.freeze({ state: 'unknown' })
const rejected: CommandResult = Object.freeze({ state: 'rejected' })
const applied: CommandResult = Object.freeze({ state: 'applied' })
const sameScope = (a: PrivacyScope, b: PrivacyScope) => a.userId === b.userId && a.eventId === b.eventId
  && a.generation === b.generation && a.policyGeneration === b.policyGeneration && a.gate === b.gate
const frozen = <T>(value: T): T => {
  if (value && typeof value === 'object' && !Object.isFrozen(value)) {
    Object.values(value).forEach(frozen)
    Object.freeze(value)
  }
  return value
}
const detached = <T>(value: T): T => frozen(structuredClone(value))
const selectionEqual = (a: NoteLandmarkSelection, b: NoteLandmarkSelection) => a.landmark_id === b.landmark_id
  && a.catalog_revision === b.catalog_revision && a.landmark_revision === b.landmark_revision

export function createVenueNotesAdapter(options: VenueNotesOptions): VenueNotesAdapter {
  const { authority, clock, client } = options
  const scope = detached(authority.scope), venue = detached(options.venue.identity)
  const eventId = scope.eventId
  const listeners = new Set<() => void>()
  const ledger = new Map<string, { binding: NoteStatusBinding; result: CommandResult; signal: AbortSignal; landmarkId: LandmarkId }>()
  let disposed = false, withdrawn = false, epoch = 0, readEpoch = 0, summaryEpoch = 0
  let selected: { landmarkId: LandmarkId; selection: NoteLandmarkSelection } | null = null
  let leaseUntil: number | null = null, visibleCount: number | null = null
  let roots: readonly PlaceNote[] = []
  let clusters = new Map<LandmarkId, ClusterRow>()
  let summaryFlight: Promise<boolean> | null = null
  const threads = new Map<string, readonly PlaceNote[]>()
  const now = () => {
    const value = clock.read()
    return value && sameScope(value.scope, scope) && Number.isFinite(value.nowMs) ? value.nowMs : null
  }
  const current = () => !disposed && !withdrawn && !authority.signal.aborted && authority.isCurrent()
    && scope.gate === 'open' && scope.userId !== null && eventId !== null && sameScope(scope, authority.scope)
  const validLease = (value: Lease | null, time: number) => value !== null && sameScope(value.scope, scope)
    && sameSiteplanIdentity(value.venue, venue) && value.accessUntilMs !== null
    && Number.isFinite(value.accessUntilMs) && value.accessUntilMs > time
  function association(id: LandmarkId) {
    const time = now(), value = options.associations.read()
    if (!current() || time === null || !validLease(value, time) || !value) return null
    const matches = value.bindings.filter(binding => binding.landmarkId === id)
    if (matches.length !== 1 || !options.venue.landmarks.some(landmark => landmark.id === id)) return null
    const binding = matches[0]
    if (!binding || !binding.selection.landmark_id || !Number.isSafeInteger(binding.selection.catalog_revision)
      || binding.selection.catalog_revision < 1 || !Number.isSafeInteger(binding.selection.landmark_revision)
      || binding.selection.landmark_revision < 1) return null
    // Physical IDs are unique too: even different revisions cannot silently select one ART target.
    if (value.bindings.filter(row => row.selection.landmark_id === binding.selection.landmark_id).length !== 1) return null
    return detached(binding)
  }
  const notify = () => { for (const listener of Array.from(listeners)) { try { listener() } catch { /* isolate consumers */ } } }
  function scrub(permanent: boolean) {
    epoch++; readEpoch++; summaryEpoch++; roots = []; threads.clear(); selected = null; leaseUntil = null
    visibleCount = null; clusters = new Map(); summaryFlight = null
    if (permanent) { withdrawn = true; ledger.clear() }
    notify()
  }
  const eligible = (note: PlaceNote, time: number) => selected !== null
    && note.landmark_id === selected.selection.landmark_id && note.landmark_revision === selected.selection.landmark_revision
    && utcMilliseconds(note.created_at) <= time && utcMilliseconds(note.expires_at) > time
  function snapshot(): VenueNotesSnapshot | null {
    const time = now(), provider = options.associations.read()
    if (!current() || !client || time === null || leaseUntil === null || leaseUntil <= time
      || !provider || provider.accessUntilMs === null || provider.accessUntilMs <= time) return null
    const binding = selected ? association(selected.landmarkId) : null
    if (selected && (!binding || !selectionEqual(binding.selection, selected.selection))) return null
    const policy = options.creationRights?.read() ?? null
    const rights = validLease(policy, time) && policy ? {
      postNote: policy.rights.postNote.filter(id => association(id) !== null),
      noteAudiences: policy.rights.noteAudiences.filter(audience => audience === 'FRIENDS' || audience === 'EVERYONE'),
      noteExpiries: policy.rights.noteExpiries.filter(expiry => expiry === 'ONE_HOUR' || expiry === 'END_OF_DAY'),
    } : { postNote: [], noteAudiences: [], noteExpiries: [] }
    const projectReply = (note: PlaceNote) => ({ id: note.id, body: note.text, createdAtMs: utcMilliseconds(note.created_at),
      author: { userId: note.author.user_id, displayName: note.author.display_name, avatar: note.author.avatar } })
    const visibleRoots = selected ? roots.filter(note => eligible(note, time)) : []
    const notes: ViewNote[] = !selected || !binding ? [] : visibleRoots.map(note => ({ ...projectReply(note),
      landmarkId: binding.landmarkId, audience: note.audience, expiresAtMs: utcMilliseconds(note.expires_at),
      revision: note.revision, replyCount: note.reply_count, canReply: note.can_reply, canWithdraw: note.can_withdraw,
      canMeet: note.meet_here !== null && note.meet_here.landmark_id === note.landmark_id
        && note.meet_here.landmark_revision === note.landmark_revision
        && note.meet_here.recipient_user_id === note.author.user_id && utcMilliseconds(note.meet_here.access_until) > time,
      replies: (threads.get(note.id) ?? []).filter(reply => eligible(reply, time)).map(projectReply),
    }))
    // Only current server counts become markers; no page length stands in for a place total.
    const liveClusters = options.venue.landmarks.flatMap(place => {
      const row = clusters.get(place.id), actual = association(place.id)
      if (!row || !actual || row.count < 1 || !selectionEqual(row.selection, actual.selection)
        || (row.refreshAtMs !== null && row.refreshAtMs <= time)) return []
      return [{ landmarkId: place.id, count: row.count }]
    })
    return detached({ scope, venue, classification: options.classification, serverNowMs: time,
      accessUntilMs: Math.min(leaseUntil, provider.accessUntilMs,
        validLease(policy, time) && policy?.accessUntilMs !== null && policy?.accessUntilMs !== undefined ? policy.accessUntilMs : Infinity),
      notes, noteClusters: liveClusters,
      // A total that no longer matches the still-visible producer rows is unavailable, not guessed.
      notesVisibleCount: selected && visibleRoots.length === roots.length ? visibleCount : null, rights })
  }
  function contextValid(context: CommandContext) {
    return current() && !context.signal.aborted && sameScope(context.scope, scope)
      && sameSiteplanIdentity(context.venue, venue) && now() !== null
  }
  function acceptRead(value: NotePage | NoteThread, binding: NonNullable<typeof selected>, token: number, signal: AbortSignal) {
    const time = now(), actual = association(binding.landmarkId), provider = options.associations.read()
    if (!current() || signal.aborted || token !== readEpoch || time === null || !actual
      || !selectionEqual(binding.selection, actual.selection) || utcMilliseconds(value.server_now) > time) return false
    const notes = 'items' in value ? value.items : [value.note, ...value.replies]
    if (notes.some(note => note.landmark_revision !== binding.selection.landmark_revision || utcMilliseconds(note.created_at) > time
      || utcMilliseconds(note.expires_at) <= time)) return false
    const until = utcMilliseconds(value.access_until)
    // An empty authorized page legitimately reports access_until === server_now; the
    // place lease still comes from the reviewed provider, never from a fabricated clock.
    const lease = notes.length ? (until > time ? until : null) : provider && provider.accessUntilMs !== null && provider.accessUntilMs > time ? provider.accessUntilMs : null
    if (lease === null) return false
    selected = binding; leaseUntil = lease
    if ('items' in value) {
      roots = detached(value.items); threads.clear(); visibleCount = value.visible_count
      clusters = new Map(clusters).set(binding.landmarkId, { landmarkId: binding.landmarkId, selection: binding.selection,
        count: value.visible_count, readAtMs: time, refreshAtMs: utcMilliseconds(value.access_until) > time ? utcMilliseconds(value.access_until) : null })
    } else {
      roots = detached([...roots.filter(note => note.id !== value.note.id), value.note])
      threads.set(value.note.id, detached(value.replies))
    }
    notify(); return true
  }
  function acceptSummary(value: NotePage, landmarkId: LandmarkId, binding: NonNullable<ReturnType<typeof association>>, token: number, signal: AbortSignal, target: Map<LandmarkId, ClusterRow>) {
    const time = now(), actual = association(landmarkId)
    if (!current() || signal.aborted || token !== summaryEpoch || time === null || !actual
      || !selectionEqual(binding.selection, actual.selection) || utcMilliseconds(value.server_now) > time
      || value.items.some(note => note.landmark_id !== binding.selection.landmark_id || note.landmark_revision !== binding.selection.landmark_revision
        || utcMilliseconds(note.created_at) > time || utcMilliseconds(note.expires_at) <= time)
      || !Number.isSafeInteger(value.visible_count) || value.visible_count < value.items.length) return false
    const until = utcMilliseconds(value.access_until)
    if (value.items.length > 0 && until <= time) return false
    target.set(landmarkId, { landmarkId, selection: binding.selection, count: value.visible_count, readAtMs: time, refreshAtMs: until > time ? until : null })
    return true
  }
  function bindCommand(key: string, operationId: NoteCommand['operation_id'], binding: NonNullable<typeof selected>,
    context: CommandContext, parent: PlaceNote | null) {
    if (ledger.has(key) || ledger.size >= 64) return null
    const entry: { binding: NoteStatusBinding; result: CommandResult; signal: AbortSignal; landmarkId: LandmarkId } = { binding: detached({ operationId, key, scope, venue, selection: binding.selection,
      parent: parent ? { id: parent.id, revision: parent.revision, audience: parent.audience, expires_at: parent.expires_at } : null,
      receipt: null }), result: unknown, signal: context.signal, landmarkId: binding.landmarkId }
    ledger.set(key, entry); return entry
  }
  function receiptResult(entry: NonNullable<ReturnType<typeof bindCommand>>, value: NoteCommand): CommandResult {
    const receipt = decodeWire<NoteCommand>(value, notesSchemas.NoteCommand, notesSchemas, 'invalid-response')
    const old = entry.binding.receipt
    if (receipt.operation_id !== entry.binding.operationId || utcMilliseconds(receipt.expires_at) <= utcMilliseconds(receipt.created_at)
      || (old && (old.receipt_id !== receipt.receipt_id || old.created_at !== receipt.created_at || old.expires_at !== receipt.expires_at
        || (old.resource && (!receipt.resource || old.resource.id !== receipt.resource.id || old.resource.revision !== receipt.resource.revision))))
      || (old && (old.state === 'SUCCEEDED' || old.state === 'FAILED' || old.state === 'RETIRED') && receipt.state !== old.state)
      || (receipt.state === 'SUCCEEDED' && (!receipt.resource || receipt.failure_reason !== null))
      || (receipt.state === 'FAILED' && (receipt.resource !== null || !receipt.failure_reason))) return unknown
    entry.binding = detached({ ...entry.binding, receipt })
    entry.result = receipt.state === 'SUCCEEDED' ? applied : receipt.state === 'FAILED' ? rejected : unknown
    return entry.result
  }
  async function write(entry: NonNullable<ReturnType<typeof bindCommand>>, context: CommandContext, perform: () => Promise<NoteCommand>) {
    const token = epoch
    try {
      const value = await perform()
      if (token !== epoch || !contextValid(context) || entry.signal.aborted || ledger.get(context.key) !== entry) return unknown
      return receiptResult(entry, value)
    } catch (error) {
      // A decoded 4xx is a definitive server decision, never an uncertain outcome.
      return error instanceof NotesApiError && [400, 401, 403, 404, 409, 422].includes(error.status) ? rejected : unknown
    }
  }
  const unsubscribers = [authority.subscribe(() => scrub(true)), clock.subscribe(() => {
    if (!current() || now() === null) scrub(true)
    else if (leaseUntil !== null && leaseUntil <= (now() ?? Infinity)) scrub(false)
    else notify()
  }), options.associations.subscribe(() => scrub(false))]
  if (options.creationRights) unsubscribers.push(options.creationRights.subscribe(notify))
  const abort = () => scrub(true)
  authority.signal.addEventListener('abort', abort, { once: true })
  if (!current()) scrub(true)
  return Object.freeze({
    read: snapshot,
    subscribe(listener: () => void) { if (disposed) return () => {}; listeners.add(listener); return () => { listeners.delete(listener) } },
    ownsCommand(key: string) { return current() && ledger.has(key) },
    async load(landmarkId: LandmarkId, signal: AbortSignal) {
      const binding = association(landmarkId)
      if (!client || !eventId || !binding || signal.aborted) return false
      readEpoch++; roots = []; threads.clear(); selected = null; visibleCount = null; leaseUntil = null
      const token = readEpoch
      try {
        const value = await client.request('listVenuePlaceNotes', { eventId, landmarkId: binding.selection.landmark_id }, { scope, signal })
        return acceptRead(value, binding, token, signal)
      } catch { return false }
    },
    async loadSummary(signal: AbortSignal) {
      if (summaryFlight) return summaryFlight
      const time = now(), provider = options.associations.read()
      if (!client || !eventId || !current() || time === null || signal.aborted || !validLease(provider, time)) return false
      const entries = options.venue.landmarks.flatMap(place => {
        const binding = association(place.id)
        return binding ? [{ landmarkId: place.id, binding }] : []
      })
      if (!entries.length) return false
      const token = ++summaryEpoch, collected = new Map<LandmarkId, ClusterRow>(), providerUntil = provider!.accessUntilMs!
      const run = async () => {
        let accepted = 0
        let cursor = 0
        const worker = async () => {
          while (cursor < entries.length) {
            const entry = entries[cursor++]
            if (!entry || token !== summaryEpoch || signal.aborted) return
            try {
              const value = await client.request('listVenuePlaceNotes', { eventId, landmarkId: entry.binding.selection.landmark_id, limit: 1 }, { scope, signal })
              if (token === summaryEpoch && !signal.aborted && acceptSummary(value, entry.landmarkId, entry.binding, token, signal, collected)) accepted++
            } catch { /* Denied or unavailable places contribute no marker or count. */ }
          }
        }
        await Promise.all(Array.from({ length: Math.min(3, entries.length) }, worker))
        if (token !== summaryEpoch || signal.aborted || !current()) return false
        if (!accepted) {
          // A total denial is a withdrawn read lease, not an empty successful summary.
          clusters = new Map(); roots = []; threads.clear(); selected = null; visibleCount = null; leaseUntil = null; notify(); return false
        }
        clusters = collected; leaseUntil = providerUntil
        notify(); return true
      }
      const promise = run()
      const flight = promise.finally(() => { if (summaryFlight === flight) summaryFlight = null })
      summaryFlight = flight
      return promise
    },
    async loadThread(noteId: string, signal: AbortSignal) {
      const binding = selected && association(selected.landmarkId)
      if (!client || !eventId || !binding || !snapshot()?.notes.some(note => note.id === noteId) || signal.aborted) return false
      const token = ++readEpoch
      try {
        const value = await client.request('getVenuePlaceNote', { eventId, landmarkId: binding.selection.landmark_id, noteId }, { scope, signal })
        return acceptRead(value, binding, token, signal)
      } catch { return false }
    },
    async postNote(input: Parameters<Commands['postNote']>[0], context: CommandContext) {
      if (ledger.has(context.key)) return unknown
      const view = snapshot(), binding = association(input.landmarkId)
      if (!client || !eventId || !contextValid(context) || !view || !binding || !view.rights.postNote.includes(input.landmarkId)
        || !view.rights.noteAudiences.includes(input.audience) || !view.rights.noteExpiries.includes(input.expiry)) return rejected
      const request = { eventId, landmarkId: binding.selection.landmark_id, idempotencyKey: context.key,
        body: { text: input.body, audience: input.audience, expiry: input.expiry === 'END_OF_DAY' ? 'VENUE_END_DAY' as const : 'ONE_HOUR' as const,
          landmark_selection: binding.selection } }
      try { buildNotesRequest('createVenuePlaceNote', request) } catch { return rejected }
      const entry = bindCommand(context.key, 'createVenuePlaceNote', binding, context, null)
      return entry ? write(entry, context, () => client.request('createVenuePlaceNote', request, { scope: context.scope, signal: context.signal })) : unknown
    },
    async reply(input: Parameters<Commands['reply']>[0], context: CommandContext) {
      if (ledger.has(context.key)) return unknown
      const time = now(), binding = selected && association(selected.landmarkId)
      const parent = roots.find(note => note.id === input.noteId)
      if (!client || !eventId || !contextValid(context) || !snapshot() || !binding || !parent || !parent.can_reply || time === null || !eligible(parent, time)) return rejected
      const request = { eventId, landmarkId: binding.selection.landmark_id, noteId: parent.id,
        idempotencyKey: context.key, body: { text: input.body, expected_parent_revision: parent.revision } }
      try { buildNotesRequest('createVenuePlaceReply', request) } catch { return rejected }
      const entry = bindCommand(context.key, 'createVenuePlaceReply', binding, context, parent)
      return entry ? write(entry, context, () => client.request('createVenuePlaceReply', request, { scope: context.scope, signal: context.signal })) : unknown
    },
    async withdraw(input: Parameters<Commands['withdraw']>[0], context: CommandContext) {
      if (ledger.has(context.key)) return unknown
      const time = now(), binding = selected && association(selected.landmarkId)
      const note = roots.find(row => row.id === input.noteId)
      if (!client || !eventId || !contextValid(context) || !snapshot() || !binding || !note || !note.can_withdraw
        || note.revision !== input.expectedRevision || time === null || !eligible(note, time)) return rejected
      const request = { eventId, landmarkId: binding.selection.landmark_id, noteId: note.id,
        expected_revision: note.revision, idempotencyKey: context.key }
      try { buildNotesRequest('withdrawVenuePlaceNote', request) } catch { return rejected }
      const entry = bindCommand(context.key, 'withdrawVenuePlaceNote', binding, context, note)
      return entry ? write(entry, context, () => client.request('withdrawVenuePlaceNote', request, { scope: context.scope, signal: context.signal })) : unknown
    },
    async resolve(key: string, context: CommandContext) {
      const entry = ledger.get(key)
      if (!entry || key !== context.key || !contextValid(context) || entry.signal.aborted) return unknown
      const physical = association(entry.landmarkId)
      if (!physical || !selectionEqual(physical.selection, entry.binding.selection)) return unknown
      if (entry.result.state !== 'unknown') return entry.result
      const status = options.status
      if (!status) return unknown
      return write(entry, context, () => status.read(entry.binding, { scope: context.scope, signal: context.signal }))
    },
    dispose() {
      if (disposed) return
      disposed = true; scrub(true)
      authority.signal.removeEventListener('abort', abort)
      for (const unsubscribe of unsubscribers) { try { unsubscribe() } catch { /* already scrubbed */ } }
      listeners.clear()
    },
  })
}
