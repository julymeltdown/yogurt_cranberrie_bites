import { TransportError } from '../../../shared/api/errors'
import type { HttpRequest } from '../../../shared/api/http'
import type { PrivacyScope } from '../../../shared/query/privacy'
import { placeSchema, sourceLink } from '../../places/model/contract'
import type { Place } from '../../places/model/contract'
import { audienceKey, audienceSchema, decodeBeacon, decodeCommand, decodeReceipt, freeze, meetingSchema, revision, sameMeeting, utc, uuid,
  type CommandBinding, type MeetBeacon, type MeetingDescription, type Receipt } from './contract'
import type { MeetingAuthority, MeetingBeaconOptions, QualifiedMeetingPhoto } from './ports'

export interface BeaconDraft { readonly placeId: string; readonly audienceId: string; readonly instructions: string; readonly identification: string; readonly ttlSeconds: number; readonly acknowledged: boolean }
export type DetailPhase = 'idle' | 'loading' | 'current' | 'moved' | 'stale' | 'expired' | 'revoked' | 'unavailable'
export type CommandPhase = 'idle' | 'sending' | 'uncertain' | 'pending' | 'checking' | 'succeeded' | 'failed' | 'retired'
export type MeetingNotice = 'unavailable' | 'denied' | 'changed' | 'map-mismatch' | 'invalid' | 'rate' | 'read-only' | null
export interface MeetingBeaconState {
  readonly epoch: number; readonly phase: DetailPhase; readonly beacon: MeetBeacon | null; readonly place: Place | null
  readonly photo: QualifiedMeetingPhoto | null; readonly draft: BeaconDraft; readonly canRead: boolean; readonly canWrite: boolean
  readonly notice: MeetingNotice; readonly retryAt: number
  readonly command: { readonly phase: CommandPhase; readonly operation: CommandBinding['operation'] | null }
  readonly places: readonly { readonly id: string; readonly label: string }[]
  readonly audiences: readonly { readonly id: string; readonly label: string }[]
  readonly maxTtlSeconds: number
}
export const meetingNotice: Record<Exclude<MeetingNotice, null>, string> = {
  unavailable: 'Meeting information is unavailable. Check current access before trying again.', denied: 'Your current access does not allow this meeting.',
  changed: 'Meeting information changed. Read the latest revision before acting.', 'map-mismatch': 'Place or map information changed. Reload current places.',
  invalid: 'Check the place, audience, identification, expiry, and physical acknowledgement.', rate: 'Please wait before checking again.', 'read-only': 'Meeting changes are unavailable for this event.',
}
const draft = (): BeaconDraft => ({ placeId: '', audienceId: '', instructions: '', identification: '', ttlSeconds: 900, acknowledged: false })
const initial = (): MeetingBeaconState => ({ epoch: 0, phase: 'idle', beacon: null, place: null, photo: null, draft: draft(), canRead: false,
  canWrite: false, notice: null, retryAt: 0, command: { phase: 'idle', operation: null }, places: [], audiences: [], maxTtlSeconds: 0 })
export const sameMeetingScope = (a: PrivacyScope, b: PrivacyScope) => a.userId === b.userId && a.eventId === b.eventId
  && a.generation === b.generation && a.policyGeneration === b.policyGeneration && a.gate === b.gate
const unresolved = (phase: CommandPhase) => ['sending', 'uncertain', 'pending', 'checking', 'retired'].includes(phase)
function notice(error: unknown): MeetingNotice {
  if (!(error instanceof TransportError)) return 'unavailable'
  if (error.status === 401 || error.status === 403) return 'denied'
  if (error.status === 429) return 'rate'
  if (error.server?.reason === 'MAP_VERSION_MISMATCH') return 'map-mismatch'
  if (error.server?.reason === 'EVENT_READ_ONLY') return 'read-only'
  if (error.status === 409) return 'changed'
  return 'unavailable'
}

export function createMeetingBeaconController(options: MeetingBeaconOptions) {
  const { auth } = options
  let state = freeze(initial()), started = false, disposed = false, selected: string | null = null
  let attempt = 0, readAbort: AbortController | null = null, commandAbort: AbortController | null = null
  let binding: CommandBinding | null = null, receiptExpiry: number | null = null
  let watermark = 0, needsConfirmation = false, terminal: 'expired' | 'revoked' | null = null
  let timer: ReturnType<typeof setTimeout> | undefined
  const listeners = new Set<() => void>()
  const now = () => { const time = options.now(); return typeof time === 'number' && Number.isFinite(time) ? time : null }
  const emit = (patch: Partial<MeetingBeaconState>) => {
    if (disposed) return
    state = freeze({ ...state, ...patch })
    for (const listener of Array.from(listeners)) { try { listener() } catch { /* Eviction reaches all consumers. */ } }
  }
  const command = (phase: CommandPhase) => emit({ command: { phase, operation: binding?.operation ?? null } })
  function authority(): MeetingAuthority | null {
    const a = options.authority.read(), s = auth.privacy.capture(), time = now()
    if (!a || time === null || !auth.privacy.isCurrent(s) || !sameMeetingScope(a.scope, s)
      || !uuid.safeParse(s.eventId).success || !uuid.safeParse(s.userId).success
      || !utc.safeParse(a.expiresAt).success || Date.parse(a.expiresAt) <= time || !utc.safeParse(a.eventCutoff).success
      || typeof a.canRead !== 'boolean' || typeof a.canWrite !== 'boolean'
      || !Number.isInteger(a.maxTtlSeconds) || a.maxTtlSeconds < 1 || a.maxTtlSeconds > 900
      || a.audiences.length > 50 || new Set(a.audiences.map(v => v.id)).size !== a.audiences.length
      || a.audiences.some(v => !v.id || !v.label || !audienceSchema.safeParse(v.audience).success)
      || new Set(a.placeRevisions.map(v => v.placeId)).size !== a.placeRevisions.length
      || a.placeRevisions.some(v => !uuid.safeParse(v.placeId).success || !revision.safeParse(v.revision).success)) return null
    return a
  }
  function places(a: MeetingAuthority | null) {
    const p = options.places.read(), s = auth.privacy.capture()
    if (!a || !p.map || p.mapMismatch || p.list.phase !== 'ready' || p.list.data?.event_id !== s.eventId) return []
    return p.list.data.items.filter(place => placeSchema.safeParse(place).success && place.place_kind === 'PHYSICAL'
      && place.verification_state === 'VERIFIED' && place.closure_state === 'OPEN' && place.provenance.review_state === 'REVIEWED'
      && sourceLink(place.provenance.source_uri) !== null && !p.unavailablePlaceIds.includes(place.place_id)
      && a.placeRevisions.some(v => v.placeId === place.place_id)
      && p.map!.places.some(v => v.place_id === place.place_id && v.provenance.source_revision === place.provenance.source_revision
        && v.provenance.source_id === place.provenance.source_id))
  }
  function qualified(b: MeetBeacon, a: MeetingAuthority | null): Place | null {
    const time = now(), p = places(a).find(p => p.place_id === b.meeting.place_id)
    return a?.canRead && time !== null && b.event_id === a.scope.eventId && b.state === 'ACTIVE'
      && Date.parse(b.updated_at) <= time && Date.parse(b.meeting.expires_at) > time
      && b.meeting.map_revision === options.places.read().map?.map_revision
      && a.placeRevisions.some(v => v.placeId === b.meeting.place_id && v.revision === b.meeting.place_revision) ? p ?? null : null
  }
  function photo(b: MeetBeacon, p: Place): QualifiedMeetingPhoto | null {
    const value = options.photos?.read(), time = now()
    if (!value || time === null || p.provenance.photo_permission !== 'GRANTED' || value.permission !== 'GRANTED' || value.review !== 'REVIEWED'
      || !sameMeetingScope(value.scope, auth.privacy.capture()) || value.beaconId !== b.beacon_id || value.beaconRevision !== b.revision
      || value.placeId !== p.place_id || value.audienceKey !== audienceKey(b.meeting.audience)
      || !utc.safeParse(value.expiresAt).success || Date.parse(value.expiresAt) <= time || !sourceLink(value.src)
      || !value.alt.trim() || value.alt.length > 512) return null
    return freeze(structuredClone(value))
  }
  function schedule() {
    clearTimeout(timer)
    if (!started || disposed) return
    const time = now(), deadlines = [options.authority.read()?.expiresAt, state.beacon?.meeting.expires_at, state.photo?.expiresAt]
      .filter((s): s is string => !!s).map(Date.parse).filter(t => time !== null && t > time)
    const delay = time === null ? 1000 : Math.max(1, Math.min(1000, ...deadlines.map(t => t - time)))
    timer = setTimeout(() => { refreshTime(); schedule() }, delay)
  }
  function refreshTime() {
    if (disposed) return
    const a = authority(), time = now(), valid = places(a), b = state.beacon
    const write = !!a?.canRead && a.canWrite && time !== null && Date.parse(a.eventCutoff) > time
    emit({ canRead: !!a?.canRead, canWrite: write, places: valid.map(p => ({ id: p.place_id, label: p.label })),
      audiences: a?.canRead ? a.audiences.map(v => ({ id: v.id, label: v.label })) : [], maxTtlSeconds: a?.maxTtlSeconds ?? 0 })
    if (a && state.draft.ttlSeconds === 900 && !state.draft.placeId && !state.draft.audienceId && !state.draft.instructions && !state.draft.identification) {
      emit({ draft: { ...state.draft, ttlSeconds: a.maxTtlSeconds } })
    }
    if (!a?.canRead) {
      if (state.beacon || state.draft.instructions || state.draft.identification || state.draft.audienceId || state.draft.placeId) scrub(false)
      emit({ phase: 'unavailable', beacon: null, place: null, photo: null, canRead: false, canWrite: false })
    } else if (b) {
      if (time !== null && Date.parse(b.meeting.expires_at) <= time) { terminal = 'expired'; emit({ phase: 'expired', beacon: null, place: null, photo: null, draft: draft(), epoch: state.epoch + 1 }) }
      else { const p = qualified(b, a); if (!p) emit({ phase: 'stale', beacon: null, place: null, photo: null, draft: draft(), epoch: state.epoch + 1 }); else emit({ place: p, photo: photo(b, p) }) }
    }
    if (state.draft.placeId && !valid.some(p => p.place_id === state.draft.placeId)
      || state.draft.audienceId && !a?.audiences.some(v => v.id === state.draft.audienceId)) emit({ draft: draft(), epoch: state.epoch + 1 })
  }
  function scrub(clearCommand = true) {
    ++attempt; readAbort?.abort(); readAbort = null
    if (clearCommand) { commandAbort?.abort(); commandAbort = null; binding = null; receiptExpiry = null }
    emit({ ...initial(), epoch: state.epoch + 1, ...(clearCommand ? {} : { command: state.command }) })
  }
  const stopPrivacy = auth.privacy.registerCleanup(() => { scrub(); selected = null; watermark = 0; needsConfirmation = false; terminal = null })
  const changed = () => { refreshTime(); schedule() }
  const stopAuthority = options.authority.subscribe(changed), stopPlaces = options.places.subscribe(changed), stopPhotos = options.photos?.subscribe(changed)
  async function request<T>(scope: PrivacyScope, signal: AbortSignal, input: Omit<HttpRequest<T>, 'refresh'>) {
    return auth.privacy.run(scope, async ({ signal: privateSignal }) => {
      const result = await auth.request({ ...input, signal: privateSignal })
      if (!auth.session.isCurrent(result.scope) || result.scope.generation !== scope.generation || result.scope.eventId !== scope.eventId || result.scope.userId !== scope.userId) throw new Error('Invalid response scope')
      return result.data
    }, signal)
  }
  async function load() {
    if (disposed || !started || !selected || !uuid.safeParse(selected).success || !authority()?.canRead || (now() ?? 0) < state.retryAt || terminal) return
    const id = selected, ticket = ++attempt, scope = auth.privacy.capture(), abort = new AbortController()
    readAbort?.abort(); readAbort = abort
    const current = () => !disposed && ticket === attempt && selected === id && !abort.signal.aborted && auth.privacy.isCurrent(scope) && !!authority()?.canRead
    const previous = state.beacon?.revision ?? watermark
    emit({ phase: 'loading', beacon: null, place: null, photo: null, notice: null })
    try {
      const b = await request(scope, abort.signal, { path: `/api/concert/events/${scope.eventId}/beacons/${id}`,
        method: 'GET', access: 'protected', errorFamily: 'domain', domainOperation: 'getConcertBeacon', decode: decodeBeacon })
      if (!current()) return
      if (b.revision < watermark) { emit({ phase: 'stale', notice: 'changed' }); return }
      if (b.state !== 'ACTIVE' || Date.parse(b.meeting.expires_at) <= (now() ?? Infinity)) { terminal = b.state === 'REVOKED' ? 'revoked' : 'expired'; emit({ phase: terminal }); return }
      const p = qualified(b, authority())
      if (!p) { emit({ phase: 'stale', notice: 'map-mismatch' }); return }
      needsConfirmation ||= previous > 0 && b.revision > previous
      watermark = b.revision
      emit({ phase: needsConfirmation ? 'moved' : 'current', beacon: b, place: p, photo: photo(b, p) }); schedule()
    } catch (error) {
      if (!current()) return
      const time = now() ?? 0
      emit({ phase: 'unavailable', beacon: null, place: null, photo: null, draft: draft(), epoch: state.epoch + 1, notice: notice(error),
        retryAt: error instanceof TransportError && error.status === 429 ? time + (error.retryAfterMs ?? 1000) : 0 })
    }
  }
  function buildMeeting(): MeetingDescription | null {
    const a = authority(), time = now(), d = state.draft, place = places(a).find(p => p.place_id === d.placeId)
    const audience = a?.audiences.find(v => v.id === d.audienceId)
    if (!a?.canRead || !a.canWrite || time === null || !place || !audience || !d.acknowledged
      || !Number.isInteger(d.ttlSeconds) || d.ttlSeconds < 1 || d.ttlSeconds > a.maxTtlSeconds) return null
    const expires = Math.min(time + d.ttlSeconds * 1000, Date.parse(a.eventCutoff))
    if (expires <= time || !d.instructions.trim() || !d.identification.trim()) return null
    const value = meetingSchema.safeParse({ place_id: place.place_id, map_revision: options.places.read().map?.map_revision,
      place_revision: a.placeRevisions.find(v => v.placeId === place.place_id)?.revision,
      instructions: d.instructions.trim(), identification: d.identification.trim(), audience: audience.audience, expires_at: new Date(expires).toISOString() })
    return value.success ? value.data : null
  }
  async function send(operation: CommandBinding['operation']) {
    refreshTime()
    const a = authority(), time = now(), old = state.beacon
    if (disposed || !started || !a?.canRead || !a.canWrite || time === null || Date.parse(a.eventCutoff) <= time
      || time < state.retryAt || unresolved(state.command.phase)) return false
    if (operation !== 'createConcertBeacon' && (state.phase !== 'current' || !old || old.owner_id !== a.scope.userId)) return false
    const meeting = operation === 'revokeConcertBeacon' ? null : buildMeeting()
    if (operation !== 'revokeConcertBeacon' && !meeting) { emit({ notice: 'invalid' }); return false }
    const key = (options.newKey ?? (() => crypto.randomUUID()))()
    if (!uuid.safeParse(key).success) { emit({ notice: 'invalid' }); return false }
    binding = { scope: a.scope, operation, key, beaconId: operation === 'createConcertBeacon' ? null : old!.beacon_id,
      expectedRevision: operation === 'createConcertBeacon' ? 0 : old!.revision }
    const captured = binding, abort = new AbortController(); commandAbort = abort
    const path = `/api/concert/events/${a.scope.eventId}/beacons${captured.beaconId ? `/${captured.beaconId}` : ''}`
    command('sending'); emit({ notice: null })
    // Reentrant UI/authority changes cannot dispatch a command under a revoked lease.
    if (binding !== captured || !authority()?.canWrite || !auth.privacy.isCurrent(a.scope)) return false
    try {
      const result = await request(a.scope, abort.signal, { path, method: operation === 'createConcertBeacon' ? 'POST' : operation === 'updateConcertBeacon' ? 'PATCH' : 'DELETE',
        access: 'protected', errorFamily: 'domain', domainOperation: operation, idempotencyKey: key,
        ...(operation === 'revokeConcertBeacon' ? { query: { expected_revision: captured.expectedRevision } }
          : { body: { json: { meeting: meeting!, physical_presence_acknowledged: true, ...(operation === 'updateConcertBeacon' ? { expected_revision: captured.expectedRevision } : {}) } } }),
        decode: decodeCommand(captured) })
      if (disposed || binding !== captured || !auth.privacy.isCurrent(a.scope)) return false
      if ('receipt_id' in result) {
        if (Date.parse(result.created_at) > (now() ?? -Infinity)) throw new TransportError('invalid-response', { uncertain: true })
        receiptExpiry = Date.parse(result.expires_at); command(receiptExpiry <= (now() ?? Infinity) ? 'retired' : 'pending'); return false
      }
      if ('meeting' in result && !sameMeeting(result.meeting, meeting!)) throw new TransportError('invalid-response', { uncertain: true })
      command('succeeded'); emit({ draft: draft(), epoch: state.epoch + 1 })
      if (selected === result.beacon_id && (result.revision < watermark || terminal === 'revoked' && result.revision <= watermark)) return true
      if (selected && result.beacon_id !== selected && operation !== 'createConcertBeacon') return true
      selected = result.beacon_id; watermark = result.revision; needsConfirmation = operation === 'updateConcertBeacon'; terminal = operation === 'revokeConcertBeacon' ? 'revoked' : null
      emit({ beacon: null, place: null, photo: null, phase: terminal ?? 'stale' })
      if (!terminal) await load()
      return true
    } catch (error) {
      if (disposed || binding !== captured || !auth.privacy.isCurrent(a.scope)) return false
      const uncertain = !(error instanceof TransportError) || error.uncertain
      command(uncertain ? 'uncertain' : 'failed')
      const n = notice(error)
      emit({ notice: n, ...(n === 'changed' || n === 'map-mismatch' || n === 'denied' || n === 'unavailable' ? { beacon: null, place: null, photo: null, phase: 'stale' } : {}),
        ...(n === 'denied' ? { draft: draft(), epoch: state.epoch + 1 } : {}),
        retryAt: error instanceof TransportError && error.status === 429 ? (now() ?? time) + (error.retryAfterMs ?? 1000) : 0 })
      return false
    }
  }
  async function checkCommand() {
    const b = binding, time = now()
    if (!b || disposed || !authority()?.canRead || !auth.privacy.isCurrent(b.scope) || time === null
      || !['uncertain', 'pending'].includes(state.command.phase) || time < state.retryAt) return
    if (receiptExpiry !== null && receiptExpiry <= time) { command('retired'); return }
    const abort = new AbortController(); commandAbort = abort; command('checking')
    try {
      const r: Receipt = await request(b.scope, abort.signal, { path: `/api/concert/events/${b.scope.eventId}/commands`, method: 'GET', access: 'protected',
        errorFamily: 'domain', domainOperation: 'getConcertCommandStatus', query: { operation_id: b.operation, idempotency_key: b.key }, decode: decodeReceipt(b) })
      if (disposed || binding !== b || !auth.privacy.isCurrent(b.scope)) return
      receiptExpiry = Date.parse(r.expires_at)
      if (Date.parse(r.created_at) > (now() ?? -Infinity)) throw new Error('Invalid receipt time')
      if (receiptExpiry <= (now() ?? Infinity) || r.state === 'RETIRED') { command('retired'); return }
      if (r.state === 'PENDING') { command('pending'); return }
      if (r.state === 'FAILED') { command('failed'); emit({ notice: 'changed', beacon: null, place: null, photo: null, phase: 'stale' }); return }
      command('succeeded')
      if (selected === r.resource_id && (r.resource_revision! < watermark || terminal === 'revoked' && r.resource_revision! <= watermark)) return
      selected = r.resource_id; watermark = r.resource_revision!; needsConfirmation = true
      terminal = b.operation === 'revokeConcertBeacon' ? 'revoked' : null
      emit({ beacon: null, place: null, photo: null, phase: terminal ?? 'stale', draft: draft(), epoch: state.epoch + 1 })
      if (!terminal) await load()
    } catch (error) {
      if (disposed || binding !== b || !auth.privacy.isCurrent(b.scope)) return
      command('uncertain'); emit({ notice: notice(error),
        retryAt: error instanceof TransportError && error.status === 429 ? (now() ?? time) + (error.retryAfterMs ?? 1000) : 0 })
    }
  }
  return {
    read: () => state,
    subscribe(listener: () => void) { listeners.add(listener); return () => { listeners.delete(listener) } },
    start() { if (!started && !disposed) { started = true; refreshTime(); schedule(); if (selected) void load() } },
    select(id: string | null) {
      if (disposed) return
      ++attempt; readAbort?.abort()
      if (selected !== id) { watermark = 0; needsConfirmation = false; terminal = null }
      selected = id
      emit({ phase: id ? terminal ?? 'stale' : 'idle', beacon: null, place: null, photo: null, draft: draft(), epoch: state.epoch + 1, notice: null })
      if (id) return load()
    },
    refresh: load, refreshTime,
    setDraft(patch: Partial<BeaconDraft>) {
      if (disposed || unresolved(state.command.phase) || !authority()?.canWrite) return
      emit({ draft: { ...state.draft, ...patch }, notice: null })
    },
    editCurrent() {
      const b = state.beacon, a = authority()
      if (state.phase !== 'current' || !b || b.owner_id !== a?.scope.userId || !a.canWrite || unresolved(state.command.phase)) return false
      const choice = a.audiences.find(v => audienceKey(v.audience) === audienceKey(b.meeting.audience))
      if (!choice) return false
      emit({ draft: { placeId: b.meeting.place_id, audienceId: choice.id, instructions: b.meeting.instructions,
        identification: b.meeting.identification, ttlSeconds: Math.min(900, a.maxTtlSeconds), acknowledged: false } }); return true
    },
    confirmRevision() {
      refreshTime()
      if (state.phase !== 'moved' || !state.beacon || !qualified(state.beacon, authority())) return false
      needsConfirmation = false; emit({ phase: 'current' }); return true
    },
    invalidate(event: { readonly beaconId: string; readonly revision: number; readonly state?: 'EXPIRED' | 'REVOKED' }) {
      if (disposed || event.beaconId !== selected || !revision.safeParse(event.revision).success || event.revision < watermark) return
      ++attempt; readAbort?.abort(); watermark = event.revision; needsConfirmation = true
      terminal = event.state === 'EXPIRED' ? 'expired' : event.state === 'REVOKED' ? 'revoked' : null
      emit({ phase: terminal ?? 'moved', beacon: null, place: null, photo: null, draft: draft(), epoch: state.epoch + 1 })
    },
    create: () => send('createConcertBeacon'), update: () => send('updateConcertBeacon'), revoke: () => send('revokeConcertBeacon'), checkCommand,
    dispose() {
      if (disposed) return
      clearTimeout(timer); scrub(); stopPrivacy(); stopAuthority(); stopPlaces(); stopPhotos?.(); selected = null
      disposed = true; listeners.clear()
    },
  }
}
export type MeetingBeaconController = ReturnType<typeof createMeetingBeaconController>
