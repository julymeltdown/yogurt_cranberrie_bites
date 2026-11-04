import type { z } from 'zod'
import type { Decoder } from '../../../../shared/api/http'
import { TransportError, type JsonValue } from '../../../../shared/api/errors'
import { DOMAIN_OPERATIONS, type DomainOperationId } from '../../../../shared/api/domain-operations.generated'
import { adminPlacesSchema, placeAdminSchema, setPlaceSchema, type PlaceAdmin, type SetPlace } from '../../places/contract'
import { auditListSchema, auditQuerySchema, type AuditEntry, type AuditQuery } from '../../audit/contract'
import { boundReceipt, createNoticeSchema, equal, freeze, noticesSchema, overviewSchema, setPhaseSchema, setSettingsSchema,
  settingsSchema, noticeSchema, updateNoticeSchema, utc, uuid, type CreateNotice, type Notice, type Operation, type Overview,
  type Receipt, type SetPhase, type SetSettings, type Settings, type UpdateNotice } from './contract'
import { createOperatorGuard, sameScope } from './lease'
import type { OperationsOptions } from './ports'

type Stage = 'review' | 'sending' | 'pending' | 'unknown' | 'applied' | 'failed' | 'conflict' | 'retired' | 'expired'
interface Action { operation: Operation; target: string | null; expectedRevision: number; stage: Stage; message: string; changes: readonly (readonly [string, string])[] }
interface Command { action: Action; key: string; body: SetSettings | SetPhase | SetPlace | CreateNotice | UpdateNotice; retainedFailure?: boolean }
export interface OperationsState {
  readonly access: 'ready' | 'unavailable' | 'withdrawn'
  readonly loading: boolean
  readonly message: string | null
  readonly settings: Settings | null
  readonly overview: Overview | null
  readonly notices: readonly Notice[]
  readonly noticeCursor: string | null
  readonly places: readonly PlaceAdmin[]
  readonly placeCursor: string | null
  readonly audit: readonly AuditEntry[]
  readonly auditCursor: string | null
  readonly auditDetail: AuditEntry | null
  readonly command: Action | null
}
const empty = (): OperationsState => ({ access: 'unavailable', loading: false, message: null, settings: null, overview: null,
  notices: [], noticeCursor: null, places: [], placeCursor: null, audit: [], auditCursor: null, auditDetail: null, command: null })
const uncertain = (stage: Stage) => ['sending', 'pending', 'unknown', 'retired', 'expired'].includes(stage)
export function createOperationsController(options: OperationsOptions) {
  const { auth } = options, listeners = new Set<() => void>()
  let state = freeze(empty()), command: Command | null = null, sequence = 0, disposed = false, auditQuery: AuditQuery = {}
  let noticePage: string | undefined, placePage: string | undefined
  let placeFlight: { release: unknown; abort: AbortController } | null = null
  function emit(patch: Partial<OperationsState>) { state = freeze({ ...state, ...patch }); listeners.forEach(listener => listener()) }
  const guard = createOperatorGuard(options, () => { sequence++; placeFlight?.abort.abort(); placeFlight = null; command = null; auditQuery = {}; noticePage = undefined; placePage = undefined; emit({ ...empty(), access: 'withdrawn' }) })
  if (guard.valid()) emit({ access: 'ready' })
  const prefix = `/api/concert/events/${guard.scope.eventId}`
  const alive = () => !disposed && guard.valid()
  const binding = (c: Command) => ({ actor: guard.scope.userId!, event: guard.scope.eventId!, operation: c.action.operation,
    key: c.key, target: c.action.target, revision: c.action.expectedRevision })
  function feedback(error: unknown): string {
    if (error instanceof TransportError) {
      if (error.status === 401 || error.status === 403) { guard.withdraw(); return 'Operator permission is no longer available.' }
      const reason = error.server?.reason
      if (reason === 'VERSION_CONFLICT') return 'The revision changed. Refresh and review your input before another action.'
      if (reason === 'EVENT_READ_ONLY') return 'The server rejected this action because the event is read only.'
      if (reason === 'RATE_LIMITED' || error.status === 429) return 'Too many requests. Wait before an explicit refresh; no action was replayed.'
      if (reason === 'FEATURE_UNAVAILABLE' || reason === 'DEPENDENCY_UNAVAILABLE' || error.status === 503) return 'The owning service is unavailable. The outcome has not been assumed.'
    }
    return 'The response could not be qualified. Refresh current information explicitly.'
  }
  function decoder<T>(schema: z.ZodType<T>, check?: (value: T) => boolean): Decoder<T> {
    return (value, context) => {
      guard.assert()
      if (context.status !== 200 || !auth.session.isCurrent(context.scope) || context.scope.userId !== guard.scope.userId
        || context.scope.eventId !== guard.scope.eventId) throw new Error('Invalid response scope')
      const result = schema.parse(value)
      if (check && !check(result)) throw new Error('Invalid response binding')
      return freeze(result)
    }
  }
  async function read<T>(operation: DomainOperationId, path: string, decode: Decoder<T>, query?: Readonly<Record<string, string | number | boolean>>) {
    guard.assert()
    return auth.privacy.run(guard.scope, async ({ signal }) => {
      const response = await auth.request({ method: 'GET', path: prefix + path, access: 'protected', errorFamily: 'domain', domainOperation: operation,
        decode, signal, ...(query ? { query } : {}) })
      guard.assert()
      if (!auth.session.isCurrent(response.scope)) throw new Error('Late response')
      return response.data
    }, guard.signal)
  }
  const fetchSettings = () => read('getConcertSettings', '/admin/settings', decoder(settingsSchema, s => s.event.event_id === guard.scope.eventId))
  const fetchNotices = (cursor?: string) => read('listConcertAdminNotices', '/admin/notices', decoder(noticesSchema,
    page => page.items.every(n => n.event_id === guard.scope.eventId && Date.parse(n.confirmed_at) <= guard.now()!)), { limit: 20, ...(cursor ? { cursor } : {}) })
  const fetchPlaces = (cursor?: string) => read('listConcertAdminPlaces', '/admin/places', decoder(adminPlacesSchema), { limit: 20, ...(cursor ? { cursor } : {}) })
  function startLoad() {
    if (!alive() || state.loading || command?.action.stage === 'sending' || command?.action.stage === 'review') return null
    emit({ loading: true, message: null }); return ++sequence
  }
  function finishLoad(id: number, collection: 'event' | 'places' | 'notices' | 'audit') {
    if (!alive() || id !== sequence) return false
    // A successful read only clears definite rejection/success. Uncertainty survives all refreshes.
    const operation = command?.action.operation
    const relevant = operation === 'setConcertPlace' ? collection === 'places'
      : operation === 'createConcertNotice' || operation === 'updateConcertNotice' ? collection === 'event' || collection === 'notices' : collection === 'event'
    if (command && relevant && !uncertain(command.action.stage) && !command.retainedFailure) { command = null; emit({ command: null }) }
    emit({ loading: false }); return true
  }
  async function loadEvent() {
    const id = startLoad(); if (id === null) return
    try {
      const [settings, overview, notices] = await Promise.all([fetchSettings(),
        read('getConcertAdminOverview', '/admin/overview', decoder(overviewSchema, o => o.event_id === guard.scope.eventId)), fetchNotices()])
      if (finishLoad(id, 'event')) { noticePage = undefined; emit({ settings, overview, notices: notices.items, noticeCursor: notices.next_cursor }) }
    } catch (error) { const message = feedback(error); if (alive() && id === sequence) emit({ loading: false, message }) }
  }
  async function loadPlaces(next = false) {
    const cursor = next ? state.placeCursor : null; if (next && !cursor) return
    const id = startLoad(); if (id === null) return
    try { const page = await fetchPlaces(cursor ?? undefined); if (finishLoad(id, 'places')) { placePage = cursor ?? undefined; emit({ places: page.items, placeCursor: page.next_cursor }) } }
    catch (error) { const message = feedback(error); if (alive() && id === sequence) emit({ loading: false, message }) }
  }
  async function loadNotices(next = false) {
    const cursor = next ? state.noticeCursor : null; if (next && !cursor) return
    const id = startLoad(); if (id === null) return
    try { const page = await fetchNotices(cursor ?? undefined); if (finishLoad(id, 'notices')) { noticePage = cursor ?? undefined; emit({ notices: page.items, noticeCursor: page.next_cursor }) } }
    catch (error) { const message = feedback(error); if (alive() && id === sequence) emit({ loading: false, message }) }
  }
  async function loadAudit(query: AuditQuery = {}, next = false) {
    const parsed = auditQuerySchema.safeParse(query)
    if (!parsed.success || next && !equal(parsed.data, auditQuery)) { emit({ message: 'Review the audit filters and time interval.' }); return }
    const cursor = next ? state.auditCursor : null; if (next && !cursor) return
    const id = startLoad(); if (id === null) return
    emit({ audit: [], auditDetail: null, auditCursor: null }); auditQuery = freeze(parsed.data)
    try {
      const filters: Record<string, string | number | boolean> = { limit: 20, ...(cursor ? { cursor } : {}) }
      for (const [key, value] of Object.entries(parsed.data)) if (value !== undefined) filters[key] = value
      const page = await read('listConcertAudit', '/admin/audit', decoder(auditListSchema, page => page.items.every((row, index) => {
        const before = page.items[index - 1]
        return row.event_id === guard.scope.eventId && (!parsed.data.actor_id || row.actor_id === parsed.data.actor_id)
          && (!parsed.data.operation_id || row.operation_id === parsed.data.operation_id) && (!parsed.data.outcome || row.outcome === parsed.data.outcome)
          && (!parsed.data.from || Date.parse(row.created_at) >= Date.parse(parsed.data.from)) && (!parsed.data.until || Date.parse(row.created_at) < Date.parse(parsed.data.until))
          && (!before || Date.parse(before.created_at) > Date.parse(row.created_at) || Date.parse(before.created_at) === Date.parse(row.created_at) && before.audit_id > row.audit_id)
      })), filters)
      if (finishLoad(id, 'audit')) emit({ audit: page.items, auditCursor: page.next_cursor })
    } catch (error) { const message = feedback(error); if (alive() && id === sequence) emit({ loading: false, message }) }
  }
  function readPlaceRelease() {
    try { return options.places?.read() ?? null } catch { return null }
  }
  function placeGrant(body: SetPlace) {
    const release = readPlaceRelease(), current = state.places.find(row => row.place.place_id === body.place.place_id)
    if (!release || !sameScope(release.scope, guard.scope) || !Number.isSafeInteger(release.mapRevision) || release.mapRevision < 1
      || !utc.safeParse(release.expiresAt).success || Date.parse(release.expiresAt) <= guard.now()! || !current || current.revision !== body.expected_revision) return null
    const target = release.targets.find(t => t.placeId === body.place.place_id && t.expectedRevision === current.revision)
    if (!target) return null
    const closureOnly = equal({ ...body.place, closure_state: current.place.closure_state }, current.place)
    if (!closureOnly && (!target.reviewedPlace || !equal(body.place, target.reviewedPlace))) return null
    return freeze(structuredClone(release))
  }
  function prepare(operation: Operation, target: string | null, expectedRevision: number, body: Command['body']) {
    if (!alive() || state.loading || command) return false
    const key = (options.newKey ?? (() => crypto.randomUUID()))()
    if (!uuid.safeParse(key).success) { emit({ message: 'Command correlation is unavailable.' }); return false }
    const changes: [string, string][] = []
    if ('features' in body) { for (const [key, value] of Object.entries(body.features)) changes.push([key, value ? 'Enabled' : 'Disabled']);
      for (const key of ['beacon_ttl_seconds', 'pickup_grace_seconds', 'social_grant_ttl_seconds', 'mural_cooldown_ms'] as const) changes.push([key.replaceAll('_', ' '), String(body[key])]) }
    if ('phase' in body) changes.push(['New phase', body.phase])
    if ('title' in body) changes.push(['Notice title', body.title], ['Notice text', body.text], ['Publication state', body.state], ['Source', body.source_uri], ['Confirmed at', body.confirmed_at])
    if ('place' in body) changes.push(['Place', body.place.label], ['Closure', body.place.closure_state], ['Verification', body.place.verification_state],
      ['Source revision', body.place.provenance.source_revision], ['Review state', body.place.provenance.review_state], ['Accessibility', body.place.accessibility_notes])
    if ('reason' in body) changes.push(['Reason', body.reason])
    const action: Action = freeze({ operation, target, expectedRevision, changes, stage: 'review', message: 'Review this exact target and revision before confirming.' })
    command = { action, key, body: freeze(structuredClone(body)) }; emit({ command: action, message: null }); return true
  }
  function prepareSettings(value: unknown) {
    const body = setSettingsSchema.safeParse(value)
    if (!body.success || !state.settings || body.data.expected_revision !== state.settings.revision) { emit({ message: 'Review all settings, limits, revision and reason.' }); return false }
    return prepare('setConcertSettings', guard.scope.eventId, body.data.expected_revision, body.data)
  }
  function preparePhase(value: unknown) {
    const body = setPhaseSchema.safeParse(value), current = state.settings
    if (!body.success || !current || body.data.expected_revision !== current.revision
      || !((current.event.phase === 'DRAFT' && body.data.phase === 'OPEN') || (current.event.phase === 'OPEN' && body.data.phase === 'LIVE'))) {
      emit({ message: 'Only DRAFT to OPEN or OPEN to LIVE can be reviewed here. Closure uses its separate job.' }); return false
    }
    return prepare('setConcertEventPhase', guard.scope.eventId, body.data.expected_revision, body.data)
  }
  function prepareNotice(value: unknown, id: string | null = null) {
    const body = (id ? updateNoticeSchema : createNoticeSchema).safeParse(value)
    if (!body.success || guard.now() === null || Date.parse(body.data.confirmed_at) > guard.now()!
      || id && (!uuid.safeParse(id).success || !('expected_revision' in body.data)
        || state.notices.find(n => n.notice_id === id)?.revision !== body.data.expected_revision)) {
      emit({ message: 'Review the notice, HTTPS source, nonfuture UTC confirmation time and current revision.' }); return false
    }
    return prepare(id ? 'updateConcertNotice' : 'createConcertNotice', id,
      id ? (body.data as UpdateNotice).expected_revision : 0, body.data)
  }
  function preparePlace(value: unknown) {
    const body = setPlaceSchema.safeParse(value)
    if (!body.success || !alive() || !placeGrant(body.data)) { emit({ message: 'A current map/write release is required. Closure must preserve provenance; other edits must match its reviewed new release.' }); return false }
    if (body.data.place.provenance.reviewed_at && Date.parse(body.data.place.provenance.reviewed_at) > guard.now()!) {
      emit({ message: 'Place review time must not be in the future.' }); return false
    }
    return prepare('setConcertPlace', body.data.place.place_id, body.data.expected_revision, body.data)
  }
  function cancel() { if (command?.action.stage === 'review') { command = null; emit({ command: null, message: 'Confirmation cancelled. Nothing was sent.' }) } }
  function action(stage: Stage, message: string) {
    if (!command) return
    command = { ...command, action: freeze({ ...command.action, stage, message }) }; emit({ command: command.action })
  }
  function matchesResult(c: Command, value: unknown): Settings | Notice | PlaceAdmin {
    const { operation, target, expectedRevision } = c.action
    if (operation === 'setConcertSettings' || operation === 'setConcertEventPhase') {
      const data = settingsSchema.parse(value)
      if (data.event.event_id !== target || data.revision !== expectedRevision + 1) throw new Error('Unbound settings result')
      if (operation === 'setConcertEventPhase') { if (data.event.phase !== (c.body as SetPhase).phase) throw new Error('Unapplied phase') }
      else for (const key of ['features', 'beacon_ttl_seconds', 'pickup_grace_seconds', 'social_grant_ttl_seconds', 'mural_cooldown_ms'] as const) {
        if (!equal(data[key], (c.body as SetSettings)[key])) throw new Error('Unapplied setting')
      }
      return freeze(data)
    }
    if (operation === 'setConcertPlace') {
      const data = placeAdminSchema.parse(value)
      if (data.place.place_id !== target || data.revision !== expectedRevision + 1 || !equal(data.place, (c.body as SetPlace).place)) throw new Error('Unapplied place')
      return freeze(data)
    }
    const data = noticeSchema.parse(value), body = c.body as CreateNotice
    if (data.event_id !== guard.scope.eventId || target !== null && data.notice_id !== target || data.revision !== expectedRevision + 1
      || Date.parse(data.confirmed_at) > guard.now()! || !equal(dataFromNotice(data), dataFromNotice(body as Notice))) throw new Error('Unapplied notice')
    return freeze(data)
  }
  function apply(c: Command, value: Settings | Notice | PlaceAdmin) {
    if (command?.key !== c.key || !alive()) return
    if ('event' in value) emit({ settings: value })
    else if ('notice_id' in value) emit({ notices: [value, ...state.notices.filter(n => n.notice_id !== value.notice_id)].slice(0, 100) })
    else emit({ places: state.places.map(p => p.place.place_id === value.place.place_id ? value : p) })
    action('applied', 'The authoritative result confirms this change. Review the current revision before another action.')
  }
  async function confirm() {
    const c = command
    if (!c || c.action.stage !== 'review' || !alive()) return
    const placeRelease = c.action.operation === 'setConcertPlace' ? placeGrant(c.body as SetPlace) : null
    if (c.action.operation === 'setConcertPlace' && !placeRelease) { cancel(); emit({ message: 'The map/write release changed. Refresh and review again.' }); return }
    if (placeRelease) placeFlight = { release: placeRelease, abort: new AbortController() }
    action('sending', 'Sending once. Do not repeat this action while its outcome is unresolved.')
    const id = ++sequence
    const decode: Decoder<unknown> = (value, context) => {
      guard.assert()
      if (!auth.session.isCurrent(context.scope)) throw new Error('Unbound response')
      if (context.status === 202) {
        const r = boundReceipt(value, binding(c), guard.now()!)
        if (r.state !== 'PENDING') throw new Error('Invalid pending response')
        return r
      }
      if (context.status !== 200) throw new Error('Invalid status')
      return matchesResult(c, value)
    }
    try {
      const data = await auth.privacy.run(guard.scope, async ({ signal }) => {
        if (placeRelease) {
          const result = await options.places!.write({ scope: guard.scope, mapRevision: placeRelease.mapRevision, placeId: c.action.target!, body: c.body as SetPlace, key: c.key, signal })
          if (!sameScope(result.scope, guard.scope)) throw new Error('Unbound place result')
          return decode(result.data, { status: result.status, scope: result.scope, path: `${prefix}/admin/places/${c.action.target}` })
        }
        const metadata = DOMAIN_OPERATIONS[c.action.operation]
        const path = metadata.path.replace('{eventId}', guard.scope.eventId!).replace('{noticeId}', c.action.target ?? '')
        const response = await auth.request({ path, method: metadata.method as 'PUT' | 'POST', access: 'protected', errorFamily: 'domain', domainOperation: c.action.operation,
          idempotencyKey: c.key, body: { json: c.body as JsonValue }, decode, signal })
        guard.assert(); return response.data
      }, placeFlight?.abort.signal ?? guard.signal)
      if (!alive() || id !== sequence || command?.key !== c.key) return
      if (placeRelease && !equal(placeGrant(c.body as SetPlace), placeRelease)) { action('unknown', 'The place write release changed during this action. Its outcome remains unresolved.'); return }
      if (data && typeof data === 'object' && 'receipt_id' in data) action('pending', 'The command is pending. Check its status explicitly; it has not been marked applied.')
      else apply(c, matchesResult(c, data))
    } catch (error) {
      const message = feedback(error)
      if (!alive() || id !== sequence || command?.key !== c.key) return
      const reason = error instanceof TransportError ? error.server?.reason : null
      if (reason === 'IDEMPOTENCY_EXPIRED') action('expired', 'Command retention expired. No replay or replacement key is allowed.')
      else if (reason === 'IDEMPOTENCY_CONFLICT' || reason === 'RESOURCE_UNAVAILABLE') action('unknown', 'This command cannot be qualified. A new key or replay is not authorized; inspect current state.')
      else if (reason === 'VERSION_CONFLICT') action('conflict', message)
      else if (error instanceof TransportError && error.status !== null && error.status < 500 && !error.uncertain) action('failed', message)
      else action('unknown', 'The outcome is unknown. Check status and current state; do not submit another command.')
    } finally { placeFlight = null }
  }
  async function checkStatus() {
    const c = command
    if (!c || !alive() || !['pending', 'unknown'].includes(c.action.stage) || state.loading) return
    const id = ++sequence; emit({ loading: true, message: null })
    try {
      const receipt = await read('getConcertCommandStatus', '/commands', (value, context) => {
        guard.assert(); if (context.status !== 200 || !auth.session.isCurrent(context.scope)) throw new Error('Invalid status context')
        return boundReceipt(value, binding(c), guard.now()!)
      }, { operation_id: c.action.operation, idempotency_key: c.key })
      if (!alive() || id !== sequence || command?.key !== c.key) return
      if (Date.parse(receipt.expires_at) <= guard.now()!) action('expired', 'Command retention expired. No replay or replacement key is allowed.')
      else if (receipt.state === 'RETIRED') action('retired', 'This receipt is retired. No replay or replacement key is allowed.')
      else if (receipt.state === 'PENDING') action('pending', 'The command remains pending. Nothing was replayed.')
      else if (receipt.state === 'FAILED') {
        command = { ...command!, retainedFailure: true }
        action('failed', 'The server recorded failure. Inspect current information; this receipt does not authorize replay or a replacement key.')
      }
      else await reconcile(c, receipt)
    } catch (error) { const message = feedback(error); if (alive() && id === sequence) emit({ message: `Status remains unresolved. ${message}` }) }
    finally { if (alive() && id === sequence) emit({ loading: false }) }
  }
  async function reconcile(c: Command, receipt: Receipt) {
    let current: unknown
    if (c.action.operation === 'setConcertPlace') {
      const page = await fetchPlaces(placePage); current = page.items.find(p => p.place.place_id === receipt.resource_id)
      if (alive()) emit({ places: page.items, placeCursor: page.next_cursor })
    } else if (c.action.operation === 'createConcertNotice' || c.action.operation === 'updateConcertNotice') {
      const page = await fetchNotices(noticePage); current = page.items.find(n => n.notice_id === receipt.resource_id)
      if (alive()) emit({ notices: page.items, noticeCursor: page.next_cursor })
    } else { current = await fetchSettings(); if (alive()) emit({ settings: current as Settings }) }
    if (!alive()) return
    const result = matchesResult(c, current)
    const target = 'event' in result ? result.event.event_id : 'notice_id' in result ? result.notice_id : result.place.place_id
    if (target !== receipt.resource_id || result.revision !== receipt.resource_revision) throw new Error('Receipt/current result mismatch')
    apply(c, result)
  }
  guard.own(() => options.places?.subscribe(() => {
    if (command?.action.operation === 'setConcertPlace' && command.action.stage === 'review' && !placeGrant(command.body as SetPlace)) cancel()
    if (placeFlight && !equal(readPlaceRelease(), placeFlight.release)) { placeFlight.abort.abort(); action('unknown', 'The map/write release was withdrawn. Check current state; do not repeat the command.') }
    // Refresh the displayed write-map revision even when no review or write is active.
    if (alive()) emit({})
  }) ?? (() => {}))
  return {
    read: () => state,
    subscribe(listener: () => void) { listeners.add(listener); return () => { listeners.delete(listener) } },
    loadEvent, loadPlaces, loadNotices, loadAudit, prepareSettings, preparePhase, prepareNotice, preparePlace, cancel, confirm, checkStatus,
    selectAudit(id: string | null) { if (alive()) emit({ auditDetail: state.audit.find(row => row.audit_id === id) ?? null }) },
    mapRevision() { const release = readPlaceRelease(); return alive() && release && sameScope(release.scope, guard.scope)
      && utc.safeParse(release.expiresAt).success && Date.parse(release.expiresAt) > guard.now()! && Number.isSafeInteger(release.mapRevision) && release.mapRevision > 0 ? release.mapRevision : null },
    dispose() { if (disposed) return; disposed = true; guard.withdraw(); listeners.clear() },
  }
}
export function dataFromNotice(notice: Notice): CreateNotice {
  return { title: notice.title, text: notice.text, source_uri: notice.source_uri, confirmed_at: notice.confirmed_at, state: notice.state }
}
export type OperationsController = ReturnType<typeof createOperationsController>
