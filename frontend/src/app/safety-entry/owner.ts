import type { SessionController } from '../../features/auth/model/session'
import { createSafetyController, type SafetyController } from '../../features/safety/controller'
import { blockListSchema, reportListSchema, sameScope, uuid } from '../../features/safety/contract'
import type { PrivacyScope } from '../../shared/query/privacy'
import { authorizeUser, reportAuthorization } from './authorization'

interface BlockTarget { readonly userId: string; readonly scope: PrivacyScope; readonly origin: string }
interface Snapshot {
  readonly controller: SafetyController | null; readonly scope: PrivacyScope | null
  readonly preparing: boolean; readonly reports: boolean; readonly blocks: boolean
  readonly target: BlockTarget | null; readonly message: string; readonly version: number
}
const unresolved = (controller: SafetyController | null) => !!controller && ['submitting', 'pending', 'unknown', 'failed'].includes(controller.read().phase)

/** One retained safety command owner per canonical full privacy lease. No Auth,
 * cache, World or socket is constructed here. Ordinary route changes retain it. */
export function createSafetyEntry(auth: SessionController, path: () => string, restricted: (scope: PrivacyScope) => void) {
  let disposed = false, sequence = 0, selecting = 0, flight: AbortController | null = null, selectionFlight: AbortController | null = null
  let preparing: Promise<boolean> | null = null, stopController = () => {}
  let state: Snapshot = Object.freeze({ controller: null, scope: null, preparing: false, reports: false, blocks: false, target: null, message: '', version: 0 })
  const listeners = new Set<() => void>()
  const publish = (patch: Partial<Snapshot> = {}) => {
    state = Object.freeze({ ...state, ...patch, version: state.version + 1 })
    for (const listener of Array.from(listeners)) { try { listener() } catch { /* Complete independent privacy teardown. */ } }
  }
  const current = (scope: PrivacyScope) => !disposed && auth.privacy.isCurrent(scope) && sameScope(scope, auth.privacy.capture())
  function clear() {
    sequence++; selecting++; flight?.abort(); flight = null; preparing = null; selectionFlight?.abort(); selectionFlight = null
    stopController(); stopController = () => {}; const controller = state.controller
    publish({ controller: null, scope: null, target: null, preparing: false, reports: false, blocks: false, message: '' })
    controller?.dispose()
  }
  const stopPrivacy = auth.privacy.registerCleanup(clear)
  async function prepare() {
    const scope = auth.privacy.capture()
    if (!current(scope) || !uuid.safeParse(scope.userId).success || !uuid.safeParse(scope.eventId).success) return false
    if (state.controller && state.scope && sameScope(state.scope, scope)) return true
    if (preparing) return preparing
    const version = ++sequence, controller = new AbortController(); flight = controller
    publish({ preparing: true, message: 'Checking safety controls…' })
    const task = (async () => {
      const results = await Promise.allSettled([
        auth.privacy.run(scope, async context => {
          if (!current(scope) || version !== sequence) throw new Error('Withdrawn safety lease')
          return auth.request({ path: `/api/concert/events/${scope.eventId}/me/blocks`, method: 'GET', access: 'protected', errorFamily: 'domain', domainOperation: 'listConcertBlocks', query: { limit: 20 }, signal: context.signal,
            decode(value, response) { const page = blockListSchema.parse(value); if (response.status !== 200 || page.items.length > 20 || new Set(page.items.map(item => item.target_user_id)).size !== page.items.length || page.items.some(item => !item.blocked_by_me || item.target_user_id === scope.userId)) throw new Error('Invalid block authority'); return true },
          })
        }, controller.signal),
        auth.privacy.run(scope, async context => {
          if (!current(scope) || version !== sequence) throw new Error('Withdrawn safety lease')
          return auth.request({ path: `/api/concert/events/${scope.eventId}/me/reports`, method: 'GET', access: 'protected', errorFamily: 'domain', domainOperation: 'listOwnConcertReports', query: { limit: 20 }, signal: context.signal,
            decode(value, response) { const page = reportListSchema.parse(value); if (response.status !== 200 || page.items.length > 20 || new Set(page.items.map(item => item.report_id)).size !== page.items.length || page.items.some(item => item.event_id !== scope.eventId)) throw new Error('Invalid report authority'); return true },
          })
        }, controller.signal),
      ])
      if (!current(scope) || version !== sequence) return false
      const blocks = results[0].status === 'fulfilled', reports = results[1].status === 'fulfilled'
      const safety = createSafetyController({ auth, blocksAvailable: blocks, reportsAvailable: reports, authorization: reportAuthorization(auth),
        onRestricted() { const next = auth.privacy.capture(); if (auth.session.isCurrent(scope) && next.userId === scope.userId && next.eventId === scope.eventId && next.gate === 'open') restricted(next) },
      })
      stopController = safety.subscribe(() => publish())
      publish({ scope, controller: safety, blocks, reports, preparing: false, message: !blocks && !reports ? 'Safety services are unavailable. No action has been sent.' : '' })
      return true
    })().catch(() => { if (current(scope) && version === sequence) publish({ preparing: false, message: 'Safety services could not be checked.' }); return false })
      .finally(() => { if (preparing === task) preparing = null; if (flight === controller) flight = null })
    preparing = task; return task
  }
  return {
    auth, read: () => state,
    subscribe(listener: () => void) { listeners.add(listener); return () => { listeners.delete(listener) } },
    prepare,
    retry() { if (!unresolved(state.controller)) { clear(); return prepare() }; return Promise.resolve(false) },
    changeRoute(next: string) {
      selecting++; selectionFlight?.abort(); selectionFlight = null
      if (state.target?.origin !== next) publish({ target: null })
      if (!next.startsWith('/report?')) state.controller?.close()
    },
    async beginBlock(userId: string, scope: PrivacyScope) {
      if (!current(scope) || userId === scope.userId || !uuid.safeParse(userId).success || unresolved(state.controller)) return false
      const origin = path(), ticket = ++selecting
      selectionFlight?.abort(); selectionFlight = null
      if (!await prepare() || !current(scope) || !state.blocks || unresolved(state.controller)) return false
      const abort = new AbortController(); selectionFlight = abort
      try {
        await authorizeUser(auth, scope, userId, abort.signal)
        if (!current(scope) || ticket !== selecting || path() !== origin) return false
        publish({ target: Object.freeze({ userId, scope, origin }), message: '' }); return true
      } catch { if (current(scope) && ticket === selecting) publish({ message: 'This user cannot be blocked from the current view.' }); return false }
      finally { if (selectionFlight === abort) selectionFlight = null }
    },
    dismissBlock() { selecting++; selectionFlight?.abort(); selectionFlight = null; publish({ target: null }); state.controller?.close() },
    async confirmBlock() {
      const target = state.target, safety = state.controller
      if (!target || !safety || !current(target.scope) || path() !== target.origin || unresolved(safety)) return
      const ticket = ++selecting
      selectionFlight?.abort(); const abort = new AbortController(); selectionFlight = abort
      try {
        await authorizeUser(auth, target.scope, target.userId, abort.signal)
        if (current(target.scope) && ticket === selecting && state.target === target && state.controller === safety && path() === target.origin) await safety.block(target.userId)
      } catch { if (current(target.scope) && ticket === selecting) publish({ message: 'The current user is unavailable. No block was sent.' }) }
      finally { if (selectionFlight === abort) selectionFlight = null }
    },
    dispose() { if (disposed) return; disposed = true; stopPrivacy(); clear(); listeners.clear() },
  }
}
export type SafetyEntry = ReturnType<typeof createSafetyEntry>
