import type { SessionController } from '../../features/auth/model/session'
import { createOperationsController, type OperationsController } from '../../features/admin/event'
import { equal, freeze, settingsSchema, type Settings } from '../../features/admin/event/model/contract'
import type { OperatorContext, OperatorContextPort } from '../../features/admin/mural/contract'
import { domainOperation } from '../../shared/api/domain'
import type { PrivacyScope } from '../../shared/query/privacy'
import type { ActiveScope } from '../ports'
import { createMuralClosureEntry, type MuralClosureEntry } from '../admin-entry/mural-closure'
import { createOperationsEntry, type OperationsAccessContext, type OperationsEntryOptions } from '../operations-entry'
import { sameOperationsScope, validOperationsScope } from '../operations-entry/contract'
import { createReportsEntry, type ReportsEntry } from './reports'

// Actual report/evidence/moderation producers remain owned by Operations205.
// A role response does not supply their data or a reviewed place write release.
export const unavailableProducers = Object.freeze({ reports: 'Operations205', evidence: 'Operations205', moderation: 'Operations205', placeWrite: 'BE08/Operations205' })
export interface AdminCompositionState {
  readonly phase: 'idle' | 'checking' | 'ready' | 'unavailable' | 'disposed'
  readonly decision: OperationsAccessContext | null
  readonly settings: Settings | null
  readonly operations: OperationsController | null
  readonly muralClosure: MuralClosureEntry | null
  readonly reports: ReportsEntry | null
}
interface Bundle {
  readonly decision: OperationsAccessContext
  readonly operations: OperationsController
  readonly muralClosure: MuralClosureEntry
  readonly context: OperatorContextPort
  readonly stop: () => void
  readonly withdraw: () => void
}
const empty = (phase: AdminCompositionState['phase']): AdminCompositionState => Object.freeze({ phase, decision: null, settings: null, operations: null, muralClosure: null, reports: null })

/** One full-scope lifetime above Admin panels; no independent Auth/cache/socket. */
export function createAdminComposition(auth: SessionController, scope: PrivacyScope,
  options: Pick<OperationsEntryOptions, 'timeline'> = {}) {
  const expected = Object.freeze({ ...scope }), lifetime = new AbortController(), listeners = new Set<() => void>()
  let state = empty('idle'), disposed = false, started = false, bundle: Bundle | null = null
  let reports: ReportsEntry | null = null
  let version = 0, join: AbortController | null = null, joining: Promise<void> = Promise.resolve(), stopAccess = () => {}, stopPrivacy = () => {}
  const access = createOperationsEntry({ auth, signal: lifetime.signal, ...options })
  const notify = () => { for (const fn of Array.from(listeners)) { try { fn() } catch { /* Independent borrowers. */ } } }
  const current = () => !disposed && validOperationsScope(expected) && auth.privacy.isCurrent(expected) && auth.session.isCurrent(expected)
  function clear(phase: AdminCompositionState['phase'], retainReports = false) {
    ++version
    const previous = bundle, pending = join
    const previousReports = reports
    reports = retainReports && previousReports?.isCurrent() ? previousReports : null
    bundle = null; join = null; state = Object.freeze({ ...empty(phase), reports, decision: reports?.decision ?? null })
    // Publish null before abort, child cleanup or any reentrant consumer callback.
    notify(); pending?.abort()
    if (previousReports !== reports) previousReports?.dispose()
    if (previous) { previous.withdraw(); previous.stop(); previous.muralClosure.dispose(); previous.operations.dispose() }
  }
  function dispose() {
    if (disposed) return
    disposed = true; clear('disposed'); stopAccess(); stopPrivacy(); lifetime.abort(); access.dispose(); listeners.clear()
  }
  const bound = (decision: OperationsAccessContext) => current() && access.read() === decision && access.clock.read() !== null
  async function qualify(decision: OperationsAccessContext) {
    const attempt = version, request = new AbortController(); join = request
    let partialOperations: OperationsController | null = null, partialMural: MuralClosureEntry | null = null
    try {
      const registered = domainOperation('getConcertSettings')
      if (!registered || registered.method !== 'GET' || registered.path !== '/api/concert/events/{eventId}/admin/settings') throw Error('Settings binding unavailable')
      const path = `/api/concert/events/${expected.eventId}/admin/settings`
      const response = await auth.privacy.run(expected, ({ signal }) => auth.request({ method: 'GET', path, access: 'protected',
        errorFamily: 'domain', domainOperation: 'getConcertSettings', signal,
        decode(value, context) {
          if (!bound(decision) || context.status !== 200 || context.path !== path || !auth.session.isCurrent(context.scope)
            || context.scope.userId !== expected.userId || context.scope.eventId !== expected.eventId) throw Error('Settings scope unavailable')
          const settings = settingsSchema.parse(value)
          if (settings.event.event_id !== expected.eventId) throw Error('Settings event unavailable')
          return freeze(settings)
        } }), request.signal)
      if (attempt !== version || request.signal.aborted || !bound(decision)) return
      const contextListeners = new Set<() => void>()
      let settings = response.data
      let contextValue: OperatorContext | null = Object.freeze({ scope: expected, role: 'OPERATOR', roleRevision: decision.roleRevision,
        eventRevision: settings.event.revision, phase: settings.event.phase })
      const context: OperatorContextPort = { read: () => bound(decision) ? contextValue : null,
        subscribe(fn) { contextListeners.add(fn); return () => { contextListeners.delete(fn) } } }
      const contextChanged = () => { for (const fn of Array.from(contextListeners)) { try { fn() } catch { /* Independent owners. */ } } }
      const operations = createOperationsController({ auth, authority: access.authority, clock: access.clock })
      partialOperations = operations
      const muralClosure = createMuralClosureEntry({ auth, scope: expected, context, now: access.moderatorNow })
      partialMural = muralClosure
      if (attempt !== version || !bound(decision)) return
      const stop = operations.subscribe(() => {
        if (!bundle || bundle.operations !== operations || !bound(decision)) return
        if (operations.read().access !== 'ready') { access.withdraw(); return }
        const next = operations.read().settings
        if (!next || next === settings) return
        if (next.event.event_id !== expected.eventId || next.revision < settings.revision
          || next.revision === settings.revision && !equal(next, settings)) { access.withdraw(); return }
        settings = next
        contextValue = Object.freeze({ scope: expected, role: 'OPERATOR', roleRevision: decision.roleRevision,
          eventRevision: next.event.revision, phase: next.event.phase })
        state = Object.freeze({ ...state, settings: next }); contextChanged(); notify()
      })
      bundle = { decision, context, operations, muralClosure, stop, withdraw() { contextValue = null; contextChanged(); contextListeners.clear() } }
      partialOperations = null; partialMural = null
      join = null
      state = Object.freeze({ phase: 'ready', decision, settings, operations, muralClosure, reports }); notify()
    } catch {
      if (attempt === version && current()) clear('unavailable', true)
    } finally {
      partialMural?.dispose(); partialOperations?.dispose()
    }
  }
  function changed() {
    if (disposed) return
    const decision = access.read()
    clear(decision ? 'checking' : 'unavailable')
    if (decision && sameOperationsScope(decision.scope, expected)) {
      reports = createReportsEntry({ auth, scope: expected, decision, access, signal: lifetime.signal })
      state = Object.freeze({ ...state, decision, reports: reports.isCurrent() ? reports : null }); notify()
    }
    if (decision?.role === 'OPERATOR' && sameOperationsScope(decision.scope, expected)) joining = qualify(decision)
    else if (decision) { state = Object.freeze({ ...empty('unavailable'), decision, reports }); notify() }
  }
  stopAccess = access.subscribe(changed)
  stopPrivacy = auth.privacy.registerCleanup(dispose)
  if (!current()) dispose()
  async function refresh() {
    if (!current()) { dispose(); return }
    clear('checking'); await access.refresh(); await joining
    if (!disposed && state.phase === 'checking') clear('unavailable', true)
  }
  return Object.freeze({ scope: expected,
    read() {
      if (!current()) dispose()
      else if (bundle && !bound(bundle.decision)) clear('unavailable')
      else if (reports && !reports.isCurrent()) clear('unavailable')
      return state
    },
    subscribe(fn: () => void) { if (disposed) return () => {}; listeners.add(fn); return () => { listeners.delete(fn) } },
    start() { if (started || disposed) return Promise.resolve(); started = true; return refresh() },
    refresh, dispose, isDisposed: () => disposed,
  })
}
export type AdminComposition = ReturnType<typeof createAdminComposition>

/** Production ports retain this owner even when sibling Admin route elements unmount. */
export function createDefaultAdmin(auth: SessionController, options: Pick<OperationsEntryOptions, 'timeline'> = {}) {
  let owner: AdminComposition | null = null, disposed = false
  function open(scope: ActiveScope) {
    if (disposed || !auth.session.isCurrent(scope)) return null
    const captured = auth.privacy.capture()
    if (!validOperationsScope(captured) || !auth.privacy.isCurrent(captured)) return null
    if (owner && !owner.isDisposed() && sameOperationsScope(owner.scope, captured)) return owner
    const previous = owner; owner = null; previous?.dispose()
    owner = createAdminComposition(auth, captured, options); return owner
  }
  return Object.freeze({ open,
    async authorize(scope: ActiveScope, signal: AbortSignal) {
      if (signal.aborted) return false
      const entry = open(scope); await entry?.start()
      const state = entry?.read()
      return !signal.aborted && auth.session.isCurrent(scope) && (state?.phase === 'ready' || state?.reports?.isCurrent() === true)
    },
    dispose() { if (disposed) return; disposed = true; const previous = owner; owner = null; previous?.dispose() },
  })
}
