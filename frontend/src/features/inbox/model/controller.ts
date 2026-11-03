import type { SessionController } from '../../auth/model/session'
import { TransportError } from '../../../shared/api/errors'
import type { HttpRequest } from '../../../shared/api/http'
import type { PrivacyScope } from '../../../shared/query/privacy'
import { combine, decoder, freeze, notification, page, prefix, readAllResult, receipt, uuid,
  type CommandBinding, type InboxNotification, type Receipt } from './contract'
import { targetRequest, type InboxDestination } from './targets'
export type InboxFailure = 'offline' | 'denied' | 'unavailable' | 'invalid' | 'conflict' | 'rate-limited' | 'failed'
export interface InboxOptions {
  readonly auth: Pick<SessionController, 'privacy' | 'request'>
  /** Explicit producer availability from shell composition; no invented notifications capability. */
  readonly access: { read(): boolean; subscribe(listener: () => void): () => void }
  readonly navigate?: (destination: InboxDestination) => void
  readonly key?: () => string; readonly now?: () => number
}
export interface InboxState {
  readonly available: boolean; readonly scope: PrivacyScope | null; readonly unreadOnly: boolean
  readonly phase: 'idle' | 'loading' | 'ready' | 'error'; readonly rows: readonly InboxNotification[]
  readonly nextCursor: string | null; readonly unreadShown: number; readonly atLimit: boolean; readonly stale: boolean
  readonly failure: InboxFailure | null
  readonly command: { readonly phase: 'idle' | 'sending' | 'uncertain' | 'checking' | 'succeeded' | 'failed' | 'retired'; readonly failure: InboxFailure | null }
  readonly target: { readonly phase: 'idle' | 'checking' | 'unavailable'; readonly notificationId: string | null }
}
const blank = (): InboxState => ({ available: false, scope: null, unreadOnly: false, phase: 'idle', rows: [], nextCursor: null, unreadShown: 0, atLimit: false, stale: false,
  failure: null, command: { phase: 'idle', failure: null }, target: { phase: 'idle', notificationId: null } })
function failure(error: unknown): InboxFailure {
  if (!(error instanceof TransportError)) return 'invalid'
  if (error.kind === 'network' || error.kind === 'timeout') return 'offline'
  if (error.status === 401 || error.status === 403) return 'denied'
  if (error.status === 404 || error.status === 503) return 'unavailable'
  if (error.status === 409) return 'conflict'
  if (error.status === 429) return 'rate-limited'
  return error.kind === 'invalid-response' ? 'invalid' : 'failed'
}
export function createInboxController(options: InboxOptions) {
  const { auth, access } = options, now = options.now ?? Date.now, key = options.key ?? (() => crypto.randomUUID())
  let state = freeze(blank()), scope: PrivacyScope | null = null, generation = 0, started = false, disposed = false
  let listVersion = 0, targetVersion = 0
  let binding: CommandBinding | null = null, currentReceipt: Receipt | null = null
  const listeners = new Set<() => void>(), flights = new Map<string, AbortController>(), cursors = new Set<string>(), attempts = new Set<string>()
  let removers: (() => void)[] = []
  const publish = (patch: Partial<InboxState>) => { state = freeze({ ...state, ...patch }); for (const listener of listeners) listener() }
  const usable = () => started && !disposed && scope !== null && auth.privacy.isCurrent(scope) && access.read() && uuid(scope.eventId) && uuid(scope.userId)
  function cancel(lane?: string) {
    if (lane) { const c = flights.get(lane); flights.delete(lane); c?.abort() }
    else { const pending = [...flights.values()]; flights.clear(); generation++; pending.forEach(c => c.abort()) }
  }
  function clear() {
    cancel(); listVersion++; targetVersion++; scope = null; binding = null; currentReceipt = null; cursors.clear(); attempts.clear()
    state = freeze(blank()); for (const listener of listeners) listener()
  }
  function sync() {
    if (disposed || !started) return
    const next = auth.privacy.capture(), available = access.read() && auth.privacy.isCurrent(next) && uuid(next.eventId)
    if (scope && JSON.stringify(scope) === JSON.stringify(next) && state.available === available) return
    clear(); scope = next
    publish({ available, scope: available ? scope : null })
    if (available) void load()
  }
  async function run<T>(lane: string, captured: PrivacyScope, input: Omit<HttpRequest<T>, 'refresh'>): Promise<T> {
    if (!usable() || !auth.privacy.isCurrent(captured)) throw new TransportError('authentication-required')
    cancel(lane)
    const c = new AbortController(), g = generation; flights.set(lane, c)
    try {
      const result = await auth.privacy.run(captured, ({ signal }) => auth.request({ ...input, signal }), c.signal)
      if (!usable() || generation !== g || flights.get(lane) !== c) throw new TransportError('session-changed')
      return result.data
    } finally { if (flights.get(lane) === c) flights.delete(lane) }
  }
  const same = (g: number, captured: PrivacyScope) => generation === g && usable() && auth.privacy.isCurrent(captured)
  function rows(next: readonly InboxNotification[], cursor: string | null) {
    publish({ rows: next, nextCursor: cursor, unreadShown: next.filter(n => n.read_at === null).length, atLimit: next.length >= 200 || cursors.size >= 10 })
  }
  async function load(more = false) {
    if (!usable() || !scope || state.phase === 'loading' || (more && (!state.nextCursor || state.atLimit))) return
    const version = ++listVersion
    const captured = scope, g = generation, before = more ? state.rows : [], cursor = more ? state.nextCursor : null, unread = state.unreadOnly
    targetVersion++; cancel('target')
    if (!more) { cursors.clear(); rows([], null) }
    publish({ phase: 'loading', failure: null, stale: false, target: { phase: 'idle', notificationId: null } })
    const path = `${prefix(captured)}/me/notifications`
    try {
      const result = await run('list', captured, { path, method: 'GET', access: 'protected', errorFamily: 'domain', domainOperation: 'listConcertNotifications',
        query: { limit: 20, unread_only: unread, ...(cursor ? { cursor } : {}) },
        decode: decoder(captured, path, (v, status) => { if (status !== 200) throw new Error('Invalid list status'); return page(v, captured, unread) }),
      })
      if (!same(g, captured) || version !== listVersion) return
      if (result.next_cursor && (result.next_cursor === cursor || cursors.has(result.next_cursor))) throw new Error('Repeated cursor')
      if (cursor) cursors.add(cursor)
      const combined = combine(before, result.items)
      if (combined.length > 200) throw new Error('Inbox bound exceeded')
      rows(combined, result.next_cursor); publish({ phase: 'ready' })
    } catch (error) {
      if (same(g, captured) && version === listVersion) { rows([], null); publish({ phase: 'error', failure: failure(error) }) }
    }
  }
  const unresolved = () => ['sending', 'uncertain', 'checking', 'retired'].includes(state.command.phase)
  async function rereadAfterCommand() {
    // A manual/filter read may have begun before the write settled. Fence it before
    // rereading, so its older unread projection cannot replace confirmed state.
    listVersion++; cancel('list'); publish({ phase: 'idle' }); await load()
  }
  const fingerprint = (id: string | null, revision: number | null) => id ? `${id}:${revision}`
    : `all:${state.rows.filter(n => !n.read_at).map(n => `${n.notification_id}:${n.revision}`).sort().join(',')}`
  async function mark(id: string | null) {
    if (!usable() || !scope || state.phase !== 'ready' || unresolved() || state.stale) return
    const item = id === null ? null : state.rows.find(n => n.notification_id === id)
    if (id !== null && (!item || item.read_at !== null) || id === null && state.unreadShown === 0) return
    const signature = fingerprint(id, item?.revision ?? null)
    if (attempts.has(signature)) return
    if (attempts.size >= 512) { publish({ command: { phase: 'failed', failure: 'unavailable' } }); return }
    const k = key(); if (!uuid(k)) return
    const captured = scope, g = generation
    binding = freeze({ scope: captured, key: k, operation: id ? 'markConcertNotification' : 'markAllConcertNotifications', notificationId: id, revision: item?.revision ?? null })
    const b = binding; currentReceipt = null; attempts.add(signature)
    listVersion++; targetVersion++; cancel('list'); cancel('target')
    publish({ command: { phase: 'sending', failure: null }, target: { phase: 'idle', notificationId: null } })
    const path = id ? `${prefix(captured)}/me/notifications/${id}/read` : `${prefix(captured)}/me/notifications/read-all`
    try {
      const result = await run('command', captured, { path, method: 'PATCH', access: 'protected', errorFamily: 'domain', domainOperation: b.operation, idempotencyKey: b.key,
        body: { json: id ? { expected_revision: b.revision } : {} },
        decode: decoder(captured, path, (v, status): { receipt: Receipt } | { done: true; item: InboxNotification | null } => {
          if (status === 202) { const r = receipt(v, b); if (r.state !== 'PENDING') throw new Error('Invalid pending receipt'); return { receipt: r } }
          if (status !== 200) throw new Error('Invalid command status')
          if (id) { const n = notification(v, captured, id); if (!n.read_at || n.revision < b.revision!) throw new Error('Not marked read'); return { done: true, item: n } }
          readAllResult(v); return { done: true, item: null }
        }),
      })
      if (!same(g, captured) || binding !== b) return
      if ('receipt' in result) { currentReceipt = result.receipt; publish({ command: { phase: 'uncertain', failure: null } }); return }
      if (result.item) rows(state.rows.map(n => n.notification_id === id ? result.item! : n).filter(n => !state.unreadOnly || !n.read_at), state.nextCursor)
      binding = null; publish({ command: { phase: 'succeeded', failure: null } })
      // Read-all's server cutoff is authoritative; never blanket-mark cached or future rows locally.
      await rereadAfterCommand()
    } catch (error) {
      if (same(g, captured) && binding === b) {
        const definite = error instanceof TransportError && !error.uncertain
        publish({ command: { phase: definite ? 'failed' : 'uncertain', failure: failure(error) } })
      }
    }
  }
  async function recover() {
    if (!usable() || !scope || !binding || !['uncertain', 'failed'].includes(state.command.phase)) return
    const captured = scope, g = generation, b = binding
    if (currentReceipt && Date.parse(currentReceipt.expires_at) <= now()) { publish({ command: { phase: 'retired', failure: null } }); return }
    publish({ command: { phase: 'checking', failure: null } })
    const path = `${prefix(captured)}/commands`
    try {
      const r = await run('status', captured, { path, method: 'GET', access: 'protected', errorFamily: 'domain', domainOperation: 'getConcertCommandStatus',
        query: { operation_id: b.operation, idempotency_key: b.key }, decode: decoder(captured, path, (v, status) => { if (status !== 200) throw new Error('Invalid status'); return receipt(v, b) }),
      })
      if (!same(g, captured) || binding !== b) return
      currentReceipt = r
      if (Date.parse(r.expires_at) <= now() || r.state === 'RETIRED') { publish({ command: { phase: 'retired', failure: null } }); return }
      if (r.state === 'FAILED') { publish({ command: { phase: 'failed', failure: 'failed' } }); return }
      if (r.state !== 'SUCCEEDED') { publish({ command: { phase: 'uncertain', failure: null } }); return }
      // There is no notification-by-ID read contract. Reread the authorized list, without inferring
      // a missing paged row is unread/nonexistent; current rows come only from that fresh response.
      await rereadAfterCommand()
      if (same(g, captured) && state.phase === 'ready' && binding === b) { binding = null; publish({ command: { phase: 'succeeded', failure: null } }) }
      else if (same(g, captured)) publish({ command: { phase: 'uncertain', failure: 'unavailable' } })
    } catch (error) { if (same(g, captured) && binding === b) publish({ command: { phase: 'uncertain', failure: failure(error) } }) }
  }
  return {
    read: () => state, subscribe(listener: () => void) { listeners.add(listener); return () => listeners.delete(listener) },
    start() {
      if (started || disposed) return
      started = true; removers = [auth.privacy.registerCleanup(clear), auth.privacy.subscribe(sync), access.subscribe(sync)]; sync()
    },
    load, markRead: (id: string) => mark(id), markAllRead: () => mark(null), recover,
    setUnreadOnly(unreadOnly: boolean) {
      if (state.unreadOnly === unreadOnly || !usable()) return
      listVersion++; targetVersion++; cancel('list'); cancel('target'); publish({ unreadOnly, phase: 'idle', target: { phase: 'idle', notificationId: null } }); rows([], null); void load()
    },
    async open(id: string) {
      if (!usable() || !scope || state.stale) return
      const n = state.rows.find(n => n.notification_id === id), captured = scope, g = generation, version = ++targetVersion
      cancel('target')
      const input = n?.target && options.navigate ? targetRequest(captured, n.target) : null
      if (!input) { publish({ target: { phase: 'unavailable', notificationId: id } }); return }
      publish({ target: { phase: 'checking', notificationId: id } })
      try {
        const path = await run('target', captured, input)
        if (same(g, captured) && version === targetVersion && state.target.notificationId === id) {
          publish({ target: { phase: 'idle', notificationId: null } })
          if (same(g, captured)) options.navigate?.({ path, scope: captured })
        }
      } catch {
        if (same(g, captured) && version === targetVersion && state.target.notificationId === id) {
          // Remove the stale link synchronously. Target projections never enter the inbox state.
          rows(state.rows.map(row => row.notification_id === id ? { ...row, target: null } : row), state.nextCursor)
          publish({ target: { phase: 'unavailable', notificationId: id } })
        }
      }
    },
    /** Authenticated invalidation metadata only. Never ingest a pushed notification/payload as authority. */
    invalidate(expected: PrivacyScope) {
      if (!usable() || !auth.privacy.isCurrent(expected)) return
      listVersion++; targetVersion++; cancel('list'); cancel('target'); rows([], null)
      publish({ phase: 'idle', stale: true, failure: null, target: { phase: 'idle', notificationId: null } })
    },
    dispose() { if (disposed) return; disposed = true; started = false; removers.forEach(remove => remove()); removers = []; clear(); listeners.clear() },
  }
}
export type InboxController = ReturnType<typeof createInboxController>
