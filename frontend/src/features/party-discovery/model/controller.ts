import { validCapabilities, type ShellCapabilities } from '../../../app/capabilities'
import type { ShellBootstrap } from '../../../app/ports'
import type { SessionController } from '../../auth/model/session'
import type { PrivacyScope } from '../../../shared/query/privacy'
import { TransportError } from '../../../shared/api/errors'
import { decodeActivity, decodeCommand, decodeList, decodeParty, decodeReceipt, uuid,
  type Activity, type Application, type CommandBinding, type CommandOperation, type Page, type Party, type Receipt } from './contract'

export interface PartyAccess { readonly scope: PrivacyScope; readonly phase: ShellBootstrap['event']['phase']; readonly capabilities: ShellCapabilities }
export interface PartyAccessPort { read(): PartyAccess | null; subscribe(listener: () => void): () => void }
type Notice = 'unavailable' | 'denied' | 'missing' | 'rate' | 'invalid' | 'changed' | 'full' | 'duplicate' | 'conflict' | 'read-only'
export interface Resource<T> { readonly phase: 'idle' | 'loading' | 'ready' | 'error'; readonly data: T | null; readonly error: Notice | null; readonly retryAt: number }
export interface Filters { readonly ticket_section: string; readonly state: '' | Party['state']; readonly has_space: boolean; readonly starts_after: string; readonly starts_before: string }
export type CommandPhase = 'idle' | 'sending' | 'uncertain' | 'pending' | 'checking' | 'reconciling' | 'succeeded' | 'failed' | 'unavailable' | 'retired' | 'expired'
export interface CommandView { readonly phase: CommandPhase; readonly kind: 'apply' | 'cancel' | null; readonly error: Notice | null; readonly retryAt: number; readonly currentState: Application['state'] | null }
export interface PartyState {
  readonly epoch: number; readonly view: 'parties' | 'plan'; readonly filters: Filters; readonly filterError: boolean; readonly note: string
  readonly list: Resource<Page<Party>>; readonly detail: Resource<Party>; readonly activity: Resource<Activity>
  readonly command: CommandView; readonly access: boolean
}
const empty = <T>(): Resource<T> => ({ phase: 'idle', data: null, error: null, retryAt: 0 })
const initialCommand = (): CommandView => ({ phase: 'idle', kind: null, error: null, retryAt: 0, currentState: null })
const initialFilters: Filters = Object.freeze({ ticket_section: '', state: '', has_space: false, starts_after: '', starts_before: '' })
const same = (a: PrivacyScope, b: PrivacyScope) => a.generation === b.generation && a.userId === b.userId && a.eventId === b.eventId && a.policyGeneration === b.policyGeneration && a.gate === b.gate
const initial = (): PartyState => ({ epoch: 0, view: 'parties', filters: initialFilters, filterError: false, note: '', list: empty(), detail: empty(), activity: empty(), command: initialCommand(), access: false })
const unresolved = (p: CommandPhase) => !['idle', 'succeeded', 'failed'].includes(p)
function safeError(error: unknown): Notice {
  if (!(error instanceof TransportError)) return 'unavailable'
  if (error.kind === 'invalid-response') return 'invalid'
  if (error.status === 401 || error.status === 403) return 'denied'
  if (error.status === 404) return 'missing'
  if (error.status === 429) return 'rate'
  switch (error.server?.reason) {
    case 'PARTY_FULL': return 'full'
    case 'DUPLICATE_APPLICATION': return 'duplicate'
    case 'VERSION_CONFLICT': case 'INVALID_TRANSITION': return 'conflict'
    case 'EVENT_READ_ONLY': return 'read-only'
    default: return 'unavailable'
  }
}
function retryAt(error: unknown, now: number) { return error instanceof TransportError && error.status === 429 ? now + Math.min(Math.max(error.retryAfterMs ?? 1000, 1000), 300000) : 0 }
export const noticeText: Record<Notice, string> = {
  unavailable: 'Temporarily unavailable. Try reading again later.', denied: 'Your current access does not allow this view.', missing: 'This resource is unavailable or no longer accessible.',
  rate: 'Too many requests. Wait before checking again.', invalid: 'The response could not be verified.', changed: 'Your access changed. Read current information again.',
  full: 'This party is full. Read the latest party information.', duplicate: 'An application already exists. Check MY PLAN.', conflict: 'This information changed. Read the latest information.', 'read-only': 'New applications are paused for this event.',
}
export interface PartyOptions {
  readonly auth: Pick<SessionController, 'session' | 'privacy' | 'request'>; readonly access: PartyAccessPort
  /** Deterministic qualification seams; production uses the defaults. */
  readonly limit?: number; readonly now?: () => number; readonly newKey?: () => string
}
export function createPartyController(options: PartyOptions) {
  const { auth, access } = options, limit = options.limit ?? 20, now = options.now ?? Date.now, newKey = options.newKey ?? (() => crypto.randomUUID())
  if (!Number.isInteger(limit) || limit < 1 || limit > 100) throw new Error('Invalid party page limit')
  function immutable<T>(value: T): T {
    if (value && typeof value === 'object' && !Object.isFrozen(value)) { Object.values(value).forEach(immutable); Object.freeze(value) }; return value
  }
  let state = immutable(initial()), disposed = false, started = false, selected: string | null = null
  let captured: (CommandBinding & { scope: PrivacyScope; path: string; revision: number; note: string; fingerprint: string }) | null = null
  let receipt: Receipt | null = null, expiryTimer: ReturnType<typeof setTimeout> | undefined
  const listeners = new Set<() => void>(), attempts = new Map<string, AbortController>(), intentLocks = new Map<string, { operation: CommandOperation; partyId: string }>()
  const publish = (patch: Partial<PartyState>) => { if (disposed) return; state = immutable({ ...state, ...patch }); const observers = Array.from(listeners); for (const fn of observers) { try { fn() } catch { /* One observer must not prevent private-view eviction in others. */ } } }
  const command = (patch: Partial<CommandView>) => publish({ command: Object.freeze({ ...state.command, ...patch }) })
  function accessFor(scope = auth.privacy.capture()) {
    if (disposed) return null
    const a = access.read()
    if (!a || !same(a.scope, scope) || !auth.privacy.isCurrent(scope) || !uuid(scope.userId) || !uuid(scope.eventId)
      || !validCapabilities(a.capabilities) || !['DRAFT', 'OPEN', 'LIVE', 'READ_ONLY', 'ARCHIVED'].includes(a.phase)) return null
    if ((a.phase === 'READ_ONLY' || a.phase === 'ARCHIVED') && a.capabilities.mutations.available) return null
    return a
  }
  function canRead(scope = auth.privacy.capture()) { return !!accessFor(scope)?.capabilities.meet.available }
  function reset() {
    attempts.forEach(c => c.abort()); attempts.clear(); clearTimeout(expiryTimer); captured = null; receipt = null; intentLocks.clear()
    // Synchronous FE02 cleanup: no private rows, drafts, keys, roles or receipts survive policy/identity changes.
    publish({ ...initial(), epoch: state.epoch + 1 })
  }
  const stopCleanup = auth.privacy.registerCleanup(reset)
  const stopAccess = access.subscribe(() => {
    const wasAvailable = state.access
    publish({ access: canRead() })
    if (started && !wasAvailable && canRead()) { void loadList(); void loadActivity(); if (selected) void loadDetail() }
    if (!canRead()) {
      for (const key of ['list', 'detail', 'activity']) { attempts.get(key)?.abort(); attempts.delete(key) }
      publish({ list: empty(), detail: empty(), activity: empty(), note: '', filters: initialFilters, epoch: state.epoch + 1 })
    }
  })
  function begin(key: string) { attempts.get(key)?.abort(); const c = new AbortController(); attempts.set(key, c); return c }
  const live = (key: string, c: AbortController, s: PrivacyScope) => !disposed && attempts.get(key) === c && !c.signal.aborted && auth.privacy.isCurrent(s)
  function base(scope: PrivacyScope) { return `/api/concert/events/${scope.eventId}` }
  async function request<T>(scope: PrivacyScope, signal: AbortSignal, input: Parameters<typeof auth.request<T>>[0]) {
    return auth.privacy.run(scope, async ({ signal: privateSignal }) => {
      const r = await auth.request({ ...input, signal: privateSignal })
      if (!auth.session.isCurrent(r.scope) || r.scope.generation !== scope.generation || r.scope.userId !== scope.userId || r.scope.eventId !== scope.eventId) throw new Error('Changed scope')
      return r.data
    }, signal)
  }
  function validFilters(f: Filters) {
    const utc = (s: string) => s === '' || /^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:\.\d+)?Z$/.test(s) && Number.isFinite(Date.parse(s)) && new Date(s).toISOString().slice(0, 19) === s.slice(0, 19)
    return typeof f.ticket_section === 'string' && Array.from(f.ticket_section).length <= 120 && !Array.from(f.ticket_section).some(char => char.codePointAt(0)! < 32 || char.codePointAt(0) === 127)
      && ['', 'OPEN', 'CLOSED', 'CANCELLED', 'COMPLETED'].includes(f.state) && typeof f.has_space === 'boolean' && utc(f.starts_after) && utc(f.starts_before)
      && !(f.starts_after && f.starts_before && Date.parse(f.starts_after) >= Date.parse(f.starts_before))
  }
  async function loadList(more = false) {
    const s = auth.privacy.capture(), old = state.list.data
    if (!canRead(s) || state.list.retryAt > now() || more && (!old?.next_cursor || state.list.phase === 'loading')) return
    const c = begin('list'), path = `${base(s)}/parties`, filters = state.filters
    const query = { limit, ...(more ? { cursor: old!.next_cursor! } : {}), ...(filters.ticket_section ? { ticket_section: filters.ticket_section } : {}),
      ...(filters.state ? { state: filters.state } : {}), ...(filters.has_space ? { has_space: true } : {}),
      ...(filters.starts_after ? { starts_after: filters.starts_after } : {}), ...(filters.starts_before ? { starts_before: filters.starts_before } : {}) }
    publish({ list: { phase: 'loading', data: more ? old : null, error: null, retryAt: 0 }, access: true })
    try {
      const data = await request(s, c.signal, { path, method: 'GET', access: 'protected', errorFamily: 'domain', domainOperation: 'listConcertParties', query, decode: decodeList(limit, path) })
      const merged = more ? { ...data, items: [...old!.items, ...data.items] } : data
      if (new Set(merged.items.map(p => p.party_id)).size !== merged.items.length || more && data.next_cursor === old!.next_cursor) throw new Error('Invalid continuation')
      if (live('list', c, s) && canRead(s)) publish({ list: { phase: 'ready', data: merged, error: null, retryAt: 0 } })
    } catch (e) { if (live('list', c, s)) publish({ list: { phase: 'error', data: more ? old : null, error: safeError(e), retryAt: retryAt(e, now()) } }) }
  }
  async function loadDetail(id = selected) {
    if (!id || !uuid(id)) { publish({ detail: { ...empty(), phase: 'error', error: 'missing' } }); return }
    selected = id
    const s = auth.privacy.capture()
    if (!canRead(s) || state.detail.retryAt > now()) return
    const c = begin('detail'), path = `${base(s)}/parties/${id}`
    publish({ detail: { ...empty(), phase: 'loading' }, note: '' })
    try {
      const data = await request(s, c.signal, { path, method: 'GET', access: 'protected', errorFamily: 'domain', domainOperation: 'getConcertParty', decode: decodeParty(id, path) })
      if (live('detail', c, s) && selected === id && canRead(s)) publish({ detail: { phase: 'ready', data, error: null, retryAt: 0 } })
    } catch (e) { if (live('detail', c, s) && selected === id) publish({ detail: { ...empty(), phase: 'error', error: safeError(e), retryAt: retryAt(e, now()) } }) }
  }
  type Collection = keyof Activity
  async function loadActivity(more?: Collection) {
    const s = auth.privacy.capture(), old = state.activity.data
    if (!canRead(s) || state.activity.retryAt > now() || more && (!old?.[more].next_cursor || state.activity.phase === 'loading')) return
    const c = begin('activity'), path = `${base(s)}/me/parties`
    const cursorKey = more === 'owned' ? 'owned_cursor' : more === 'memberships' ? 'membership_cursor' : 'application_cursor'
    publish({ activity: { phase: 'loading', data: more ? old : null, error: null, retryAt: 0 } })
    try {
      const data = await request(s, c.signal, { path, method: 'GET', access: 'protected', errorFamily: 'domain', domainOperation: 'getConcertPartyActivity',
        query: { limit, ...(more ? { [cursorKey]: old![more].next_cursor! } : {}) }, decode: decodeActivity(limit, path) })
      let merged = data
      if (more) {
        const items = [...old![more].items, ...data[more].items], ids = items.map(p => 'application_id' in p ? p.application_id : p.party_id)
        if (new Set(ids).size !== ids.length || data[more].next_cursor === old![more].next_cursor) throw new Error('Invalid continuation')
        merged = { ...old!, [more]: { ...data[more], items } }
      }
      if (live('activity', c, s) && canRead(s)) {
        publish({ activity: { phase: 'ready', data: merged, error: null, retryAt: 0 } }); reconcile(merged)
      }
    } catch (e) { if (live('activity', c, s)) publish({ activity: { phase: 'error', data: more ? old : null, error: safeError(e), retryAt: retryAt(e, now()) } }) }
  }
  function normalPermission() {
    const a = accessFor()
    return !!a && a.capabilities.meet.available && a.capabilities.mutations.available && (a.phase === 'OPEN' || a.phase === 'LIVE')
  }
  function safetyPermission() {
    const a = accessFor()
    return !!a && a.capabilities.meet.available && ((a.phase === 'OPEN' || a.phase === 'LIVE') && a.capabilities.mutations.available
      || a.phase === 'READ_ONLY' && !a.capabilities.mutations.available && a.capabilities.mutations.reason === 'EVENT_READ_ONLY')
  }
  function applyReason(): string | null {
    if (!normalPermission()) return 'New applications are unavailable with your current event access.'
    if (state.command.phase !== 'idle' && state.command.phase !== 'succeeded') return 'Resolve the previous command before starting another.'
    const p = state.detail.data, a = state.activity.data
    if (state.detail.phase !== 'ready' || !p || state.activity.phase !== 'ready' || !a) return 'Read the party and your current MY PLAN first.'
    if (p.state !== 'OPEN') return `This party is ${p.state.toLowerCase()}.`
    if (p.member_count >= p.capacity) return 'This party is full.'
    if (p.owner.user_id === auth.session.read().userId || a.owned.items.some(o => o.party_id === p.party_id)) return 'You own this party.'
    if (a.memberships.items.some(m => m.party_id === p.party_id && m.state === 'ACTIVE')) return 'You are already a member.'
    if (a.applications.items.some(a => a.party_id === p.party_id && (a.state === 'PENDING' || a.state === 'APPROVED'))) return 'You already have an application or membership for this party.'
    if (a.owned.next_cursor || a.memberships.next_cursor || a.applications.next_cursor) return 'Load all remaining MY PLAN pages before applying. Partial results cannot confirm eligibility.'
    if (Array.from(state.note).length > 500) return 'Keep the application note within 500 characters.'
    const fingerprint = JSON.stringify(['applyConcertParty', p.party_id, p.revision, state.note])
    if (intentLocks.has(fingerprint)) return 'This application intent was already sent. Read its outcome in MY PLAN.'
    return null
  }
  function cancelReason(id: string): string | null {
    if (!safetyPermission()) return 'Cancellation is unavailable with your current event access.'
    if (state.command.phase !== 'idle' && state.command.phase !== 'succeeded') return 'Resolve the previous command before starting another.'
    const a = state.activity.phase === 'ready' && state.activity.data?.applications.items.find(a => a.application_id === id)
    if (!a || a.state !== 'PENDING' || a.applicant.user_id !== auth.session.read().userId) return 'Only your current pending application can be cancelled here.'
    if (intentLocks.has(JSON.stringify(['cancelConcertApplication', id, a.revision, '']))) return 'This cancellation intent was already sent.'
    return null
  }
  function armExpiry() {
    clearTimeout(expiryTimer)
    if (!receipt || !captured || !unresolved(state.command.phase) || state.command.phase === 'retired') return
    const remaining = Date.parse(receipt.expires_at) - now()
    if (remaining <= 0) { command({ phase: 'expired' }); return }
    expiryTimer = setTimeout(armExpiry, Math.min(remaining, 2147483647))
  }
  function reconcile(a: Activity) {
    if (!captured || !receipt || receipt.state !== 'SUCCEEDED' || !auth.privacy.isCurrent(captured.scope)) return
    const found = a.applications.items.find(a => a.application_id === receipt!.resource_id && a.party_id === captured!.partyId)
    if (!found || receipt.resource_revision !== null && found.revision < receipt.resource_revision
      || captured.operation === 'cancelConcertApplication' && found.state !== 'CANCELLED') return
    clearTimeout(expiryTimer); if (captured.operation === 'cancelConcertApplication') releaseApplicationIntent(captured.partyId)
    command({ phase: 'succeeded', currentState: found.state, error: null })
  }
  function releaseApplicationIntent(partyId: string) {
    for (const [fingerprint, intent] of intentLocks) if (intent.operation === 'applyConcertParty' && intent.partyId === partyId) intentLocks.delete(fingerprint)
  }
  async function acceptReceipt(r: Receipt) {
    receipt = r
    if (r.state === 'FAILED') { command({ phase: 'failed', error: safeReason(r.failure_reason), retryAt: 0 }); publish({ detail: empty(), activity: empty() }); clearTimeout(expiryTimer); return }
    if (r.state === 'RETIRED') { command({ phase: 'retired', retryAt: 0 }); clearTimeout(expiryTimer); return }
    if (r.state === 'SUCCEEDED') {
      clearTimeout(expiryTimer); command({ phase: 'reconciling', error: null, retryAt: 0 }); await loadActivity(); return
    }
    command({ phase: 'pending', error: null, retryAt: 0 }); armExpiry()
  }
  function safeReason(reason: string | null): Notice {
    if (reason === 'PARTY_FULL') return 'full'
    if (reason === 'DUPLICATE_APPLICATION') return 'duplicate'
    if (reason === 'EVENT_READ_ONLY') return 'read-only'
    if (reason === 'VERSION_CONFLICT' || reason === 'INVALID_TRANSITION') return 'conflict'
    return 'unavailable'
  }
  async function send(operation: CommandOperation, id?: string) {
    if (disposed || (operation === 'applyConcertParty' ? applyReason() : cancelReason(id ?? ''))) return
    const scope = auth.privacy.capture(), p = state.detail.data
    const a = operation === 'cancelConcertApplication' ? state.activity.data!.applications.items.find(a => a.application_id === id)! : null
    const key = newKey(); if (!uuid(key)) return
    const partyId = a?.party_id ?? p!.party_id, revision = a?.revision ?? p!.revision, note = a ? '' : state.note
    const path = a ? `${base(scope)}/party-applications/${a.application_id}` : `${base(scope)}/parties/${partyId}/applications`
    const fingerprint = JSON.stringify([operation, a?.application_id ?? partyId, revision, note])
    if (intentLocks.has(fingerprint)) return
    captured = Object.freeze({ scope, operation, key, partyId, ...(a ? { applicationId: a.application_id } : {}), path, revision, note, fingerprint })
    const b = captured; receipt = null; clearTimeout(expiryTimer); intentLocks.set(fingerprint, { operation, partyId })
    const c = begin('command'); command({ phase: 'sending', kind: a ? 'cancel' : 'apply', error: null, retryAt: 0, currentState: null })
    try {
      const result = await request(scope, c.signal, { path, method: a ? 'DELETE' : 'POST', access: 'protected', errorFamily: 'domain', domainOperation: operation,
        idempotencyKey: key, ...(a ? { query: { expected_revision: revision } } : { body: { json: { party_revision: revision, note } } }), decode: decodeCommand(b, path) })
      if (!live('command', c, scope) || captured !== b) return
      if ('receipt_id' in result) await acceptReceipt(result)
      else {
        if (operation === 'cancelConcertApplication') releaseApplicationIntent(partyId)
        command({ phase: 'succeeded', currentState: result.state }); publish({ note: '' })
        await Promise.all([loadActivity(), selected === partyId ? loadDetail() : Promise.resolve()])
      }
    } catch (e) {
      if (!live('command', c, scope) || captured !== b) return
      // Any unknown result remains recoverable with the ORIGINAL key; no replay method exists.
      const uncertain = !(e instanceof TransportError) || e.uncertain || !e.status || e.status >= 500 || e.status === 404 || e.server?.reason === 'IDEMPOTENCY_CONFLICT' || e.server?.reason === 'IDEMPOTENCY_EXPIRED'
      command({ phase: uncertain ? 'uncertain' : 'failed', error: safeError(e), retryAt: retryAt(e, now()) })
      if (!uncertain) publish({ detail: empty(), activity: empty() })
    }
  }
  async function checkStatus() {
    const b = captured
    if (!b || disposed || !auth.privacy.isCurrent(b.scope) || !unresolved(state.command.phase) || state.command.phase === 'sending'
      || state.command.phase === 'checking' || state.command.retryAt > now()) return
    const c = begin('status'), path = `${base(b.scope)}/commands`
    const knownTarget = b.applicationId ?? (receipt?.state === 'SUCCEEDED' ? receipt.resource_id : null)
    const binding = knownTarget ? { ...b, applicationId: knownTarget } : b
    clearTimeout(expiryTimer); command({ phase: 'checking', error: null })
    try {
      const r = await request(b.scope, c.signal, { path, method: 'GET', access: 'protected', errorFamily: 'domain', domainOperation: 'getConcertCommandStatus',
        query: { operation_id: b.operation, idempotency_key: b.key }, decode: decodeReceipt(binding, path) })
      if (live('status', c, b.scope) && captured === b) await acceptReceipt(r)
    } catch (e) {
      if (live('status', c, b.scope) && captured === b) { command({ phase: 'unavailable', error: safeError(e), retryAt: retryAt(e, now()) }); armExpiry() }
    }
  }
  return {
    read: () => state,
    subscribe(listener: () => void) { listeners.add(listener); return () => { listeners.delete(listener) } },
    start() { if (started || disposed) return; started = true; publish({ access: canRead() }); void loadList(); void loadActivity() },
    setView(view: PartyState['view']) { publish({ view }) },
    setNote(note: string) { if (auth.privacy.isCurrent(auth.privacy.capture()) && !unresolved(state.command.phase)) publish({ note }) },
    async setFilters(filters: Filters) { if (!validFilters(filters)) { publish({ filterError: true }); return }; publish({ filters: Object.freeze({ ...filters }), filterError: false }); await loadList() },
    loadList, loadDetail, loadActivity, applyReason, cancelReason, apply: () => send('applyConcertParty'), cancel: (id: string) => send('cancelConcertApplication', id), checkStatus,
    canReviewNewIntent: () => state.command.phase === 'failed' && state.activity.phase === 'ready' && (state.detail.phase === 'ready' || selected === null),
    clearFailure() { if (state.command.phase === 'failed' && state.activity.phase === 'ready' && (state.detail.phase === 'ready' || selected === null)) { captured = null; receipt = null; command(initialCommand()) } },
    dispose() { if (disposed) return; reset(); disposed = true; stopCleanup(); stopAccess(); listeners.clear() },
  }
}
export type PartyController = ReturnType<typeof createPartyController>
