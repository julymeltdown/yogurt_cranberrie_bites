import type { SessionController } from '../../../features/auth/model/session'
import { createReportReview, type ModeratorAccess, type ModerationController } from '../../../features/admin/reports'
import type { PrivacyScope } from '../../../shared/query/privacy'
import type { OperationsEntry, OperationsAccessContext } from '../../operations-entry'
import { sameOperationsScope } from '../../operations-entry/contract'
import { createReportEvidence, registeredReportEvidenceOperation } from './evidence'

/** A retained Reports child for one exact role decision, never a route-owned command. */
export function createReportsEntry(options: {
  auth: SessionController
  scope: PrivacyScope
  access: OperationsEntry
  decision: OperationsAccessContext
  signal: AbortSignal
}) {
  const { auth, access, decision } = options, scope = Object.freeze({ ...options.scope })
  const lifetime = new AbortController()
  let disposed = false, controller: ModerationController | null = null
  let stopAccess = () => {}, stopPrivacy = () => {}
  function current() {
    try { return !disposed && !lifetime.signal.aborted && !options.signal.aborted
      && sameOperationsScope(scope, decision.scope) && auth.privacy.isCurrent(scope) && auth.session.isCurrent(scope)
      && access.read() === decision && (decision.role === 'MODERATOR' || decision.role === 'OPERATOR')
      && access.moderator.read()?.decisionId === decision.decisionId && Number.isFinite(access.moderatorNow())
    } catch { return false }
  }
  function dispose() {
    if (disposed) return
    disposed = true
    lifetime.abort()
    for (const release of [() => controller?.dispose(), stopAccess, stopPrivacy,
      () => options.signal.removeEventListener('abort', dispose)]) {
      try { release() } catch { /* Release every registration without disturbing sibling owners. */ }
    }
  }
  const moderator: ModeratorAccess = {
    read: () => current() ? access.moderator.read() : null,
    subscribe: listener => access.subscribe(() => { if (!current()) dispose(); listener() }),
  }
  try {
    if (!current()) throw Error('Report decision unavailable')
    const evidence = registeredReportEvidenceOperation() ? createReportEvidence({ auth, scope, signal: lifetime.signal, current }) : null
    controller = createReportReview({ auth, access: moderator, ...(evidence ? { evidence } : {}), now: access.moderatorNow })
    stopAccess = access.subscribe(() => { if (!current()) dispose() })
    stopPrivacy = auth.privacy.registerCleanup(dispose)
    options.signal.addEventListener('abort', dispose, { once: true })
    if (!current()) dispose()
  } catch { dispose() }
  return Object.freeze({ decision,
    /** Borrowers never dispose this child on navigation; Root owns its fixed decision lifetime. */
    borrow() { if (!current()) { dispose(); return null }; return controller },
    isCurrent: current, isDisposed: () => disposed, dispose,
  })
}
export type ReportsEntry = ReturnType<typeof createReportsEntry>
