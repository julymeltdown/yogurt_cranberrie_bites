import type { SessionController } from '../../features/auth/model/session'
import type { ModeratorAccess, ModeratorLease } from '../../features/admin/reports'
import type { OperatorAuthorityPort, OperatorLease, OperationsClockPort } from '../../features/admin/event/model/ports'
import type { PrivacyScope } from '../../shared/query/privacy'
import { DOMAIN_OPERATIONS, type DomainOperationId } from '../../shared/api/domain-operations.generated'
import { domainOperation } from '../../shared/api/domain'
import { createBrowserAuthorityTimeline, type AuthorityTimeline, type PerformanceObservation } from '../community-authority/timeline'
import { decodeOperationsAccess, OPERATIONS_ACCESS_ID, operationsAccessPath, validOperationsScope } from './contract'

export interface OperationsAccessContext {
  readonly scope: PrivacyScope
  readonly decisionId: string
  readonly role: 'MODERATOR' | 'OPERATOR'
  readonly roleRevision: number
  readonly checkedAt: string
  readonly expiresAt: string
}
export interface OperationsEntryOptions {
  readonly auth: Pick<SessionController, 'session' | 'privacy' | 'request'>
  readonly signal: AbortSignal
  /** Same existing monotonic timeline; injection is for focused environment qualification. */
  readonly timeline?: (signal: AbortSignal) => AuthorityTimeline
}
interface Anchor {
  readonly context: OperationsAccessContext
  readonly timeline: AuthorityTimeline
  readonly start: PerformanceObservation
  readonly receipt: PerformanceObservation
  readonly serverMs: number
  readonly ttlMs: number
  readonly moderator: ModeratorLease
  readonly operator: OperatorLease
}

/** One fixed canonical scope, one explicit read at a time, no persistence or automatic renewal.
 * Informational role access never grants command, evidence, place or event-state authority.
 */
export function createOperationsEntry(options: OperationsEntryOptions) {
  const { auth, signal } = options, scope = Object.freeze({ ...auth.privacy.capture() })
  const listeners = new Set<() => void>(), stops: (() => void)[] = []
  let disposed = false, version = 0, anchor: Anchor | null = null, flight: AbortController | null = null
  let timeline: AuthorityTimeline | null = null, stopTimeline = () => {}, timer: ReturnType<typeof setTimeout> | undefined
  let lastServerMs = Number.NEGATIVE_INFINITY, lastDecision: string | null = null
  const notify = () => { for (const listener of Array.from(listeners)) { try { listener() } catch { /* Notify every borrower. */ } } }
  const current = () => {
    try { return !disposed && !signal.aborted && validOperationsScope(scope) && auth.privacy.isCurrent(scope) && auth.session.isCurrent(scope) }
    catch { return false }
  }
  function retire() {
    const hadAnchor = anchor !== null, oldFlight = flight, oldTimeline = timeline, oldStop = stopTimeline
    anchor = null; flight = null; timeline = null; stopTimeline = () => {}; clearTimeout(timer)
    // Null is visible before any abort/unsubscribe/consumer callback can re-enter.
    try { oldStop() } catch { /* Complete withdrawal. */ }
    // Detach our old clock observer before abort triggers its interruption callback.
    oldFlight?.abort()
    try { oldTimeline?.dispose() } catch { /* Complete withdrawal. */ }
    if (hadAnchor) notify()
  }
  function withdraw() { ++version; retire() }
  function dispose() {
    if (disposed) return
    disposed = true; withdraw()
    for (const stop of stops.splice(0)) { try { stop() } catch { /* Release all owned registrations. */ } }
    listeners.clear()
  }
  function subscribe(listener: () => void) {
    if (disposed) return () => {}
    const entry = () => listener(); listeners.add(entry); return () => { listeners.delete(entry) }
  }
  function sample() {
    const captured = anchor
    if (!current()) { dispose(); return null }
    if (!captured) return null
    let time: ReturnType<AuthorityTimeline['measure']> = null
    try { time = captured.timeline.measure(captured.start, captured.receipt) } catch { /* Unavailable clock. */ }
    // Account conservatively for Date's millisecond truncation of PostgreSQL UTC fractions.
    if (anchor !== captured || !time || time.roundTripMs < 0 || time.sinceStartMs < time.roundTripMs
      || time.sinceStartMs + 1 >= captured.ttlMs) { withdraw(); return null }
    const nowMs = captured.serverMs + time.sinceStartMs + 1
    if (!Number.isFinite(nowMs) || nowMs >= Date.parse(captured.context.expiresAt)) { withdraw(); return null }
    return { captured, nowMs }
  }
  const moderator: ModeratorAccess = Object.freeze({ read: () => sample()?.captured.moderator ?? null, subscribe })
  const authority: OperatorAuthorityPort = Object.freeze({ read: () => sample()?.captured.operator ?? null, subscribe })
  const clock: OperationsClockPort = Object.freeze({ read: () => {
    const value = sample(); return value ? Object.freeze({ scope, nowMs: value.nowMs }) : null
  }, subscribe })
  function schedule() {
    clearTimeout(timer)
    const value = sample()
    if (value) timer = setTimeout(() => { if (sample()) schedule() }, Math.max(1, Date.parse(value.captured.context.expiresAt) - value.nowMs))
  }
  async function refresh() {
    const attempt = ++version
    retire() // Existing guards must permanently withdraw, including equal-field replacement.
    if (attempt !== version || !current()) return
    // Actual C04 registration is required. Older generated read sets fail closed, without a cast or invented transport.
    const operation = Object.keys(DOMAIN_OPERATIONS).find((id): id is DomainOperationId => id === OPERATIONS_ACCESS_ID)
    const registered = domainOperation(operation)
    if (!operation || !registered || registered.method !== 'GET' || registered.path !== operationsAccessPath('{eventId}')
      || registered.idempotency || registered.query !== 'common.EmptyRequest' || registered.success.length !== 1 || registered.success[0] !== 200) return
    const request = new AbortController(); flight = request
    try {
      const observed = (options.timeline ?? (abort => createBrowserAuthorityTimeline(2000, abort)))(request.signal)
      if (attempt !== version || !current()) { observed.dispose(); return }
      timeline = observed
      const stop = observed.subscribe(withdraw)
      if (attempt !== version || !current()) { stop(); return }
      stopTimeline = stop
      const start = observed.capture()
      if (!start) throw Error('Operations clock unavailable')
      const result = await auth.privacy.run(scope, async ({ signal: requestSignal }) => auth.request({
        path: operationsAccessPath(scope.eventId!), method: 'GET', access: 'protected', errorFamily: 'domain',
        domainOperation: operation, signal: requestSignal,
        decode: (value, context) => decodeOperationsAccess(value, context, scope),
      }), request.signal)
      if (attempt !== version || !current() || request.signal.aborted) return
      const receipt = observed.capture(), value = result.data, serverMs = Date.parse(value.server_time)
      if (!receipt || !Number.isFinite(serverMs) || serverMs < lastServerMs) throw Error('Operations clock unavailable')
      const timing = observed.measure(start, receipt)
      if (!timing || timing.roundTripMs < 0 || timing.sinceStartMs < timing.roundTripMs) throw Error('Operations clock unavailable')
      if (!value.access) { lastServerMs = serverMs; withdraw(); return }
      const access = value.access, ttlMs = Date.parse(access.expires_at) - serverMs
      if (access.decision_id === lastDecision || timing.sinceStartMs + 1 >= ttlMs || ttlMs > 60_000) throw Error('Operations lease unavailable')
      const context: OperationsAccessContext = Object.freeze({ scope, decisionId: access.decision_id, role: access.role,
        roleRevision: access.role_revision, checkedAt: access.checked_at, expiresAt: access.expires_at })
      const next: Anchor = { context, timeline: observed, start, receipt, serverMs, ttlMs,
        moderator: Object.freeze({ scope, role: access.role, decisionId: access.decision_id, expiresAt: Date.parse(access.expires_at) }),
        operator: Object.freeze({ scope, actorId: access.actor_id, eventId: access.event_id, role: access.role,
          revision: access.role_revision, checkedAt: access.checked_at, expiresAt: access.expires_at }) }
      if (attempt !== version || !current() || request.signal.aborted) return
      lastServerMs = serverMs; lastDecision = access.decision_id; anchor = next; flight = null
      if (sample()) { notify(); if (anchor === next) schedule() }
    } catch { if (attempt === version) withdraw() }
  }
  try {
    if (!current()) dispose()
    else {
      const cleanup = auth.privacy.registerCleanup(dispose)
      if (disposed) cleanup(); else stops.push(cleanup)
      signal.addEventListener('abort', dispose, { once: true }); stops.push(() => signal.removeEventListener('abort', dispose))
      const sessionSignal = auth.session.read().signal
      sessionSignal.addEventListener('abort', dispose, { once: true }); stops.push(() => sessionSignal.removeEventListener('abort', dispose))
      if (!current()) dispose()
    }
  } catch { dispose() }
  return Object.freeze({ moderator, authority, clock, subscribe,
    read: () => sample()?.captured.context ?? null,
    moderatorNow: () => sample()?.nowMs ?? Number.NaN,
    refresh, withdraw, dispose })
}
export type OperationsEntry = ReturnType<typeof createOperationsEntry>
