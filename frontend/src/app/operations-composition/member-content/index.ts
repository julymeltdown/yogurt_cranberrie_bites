import type { SessionController } from '../../../features/auth/model/session'
import { createMemberReview } from '../../../features/admin/members'
import { createContentReview } from '../../../features/admin/content'
import type { ModerationController, ModeratorAccess } from '../../../features/admin/reports'
import { uuid } from '../../../features/admin/reports/model/contract'
import type { PrivacyScope } from '../../../shared/query/privacy'
import type { OperationsEntry, OperationsAccessContext } from '../../operations-entry'
import { sameOperationsScope } from '../../operations-entry/contract'

export const MEMBER_REVIEW_LIMIT = 8
/** One decision owns every retained intent; route changes never replace a command. */
export function createMemberContentEntry(options: {
  auth: SessionController; scope: PrivacyScope; access: OperationsEntry
  decision: OperationsAccessContext; signal: AbortSignal
}) {
  const { auth, access, decision } = options, scope = Object.freeze({ ...options.scope })
  const members = new Map<string, ModerationController>()
  let content: ModerationController | null = null, disposed = false
  let stopAccess = () => {}, stopPrivacy = () => {}
  function current(): boolean {
    try { return !disposed && !options.signal.aborted && sameOperationsScope(scope, decision.scope)
      && auth.privacy.isCurrent(scope) && auth.session.isCurrent(scope) && access.read() === decision
      && (decision.role === 'MODERATOR' || decision.role === 'OPERATOR')
      && access.moderator.read()?.decisionId === decision.decisionId && Number.isFinite(access.moderatorNow())
    } catch { return false }
  }
  function dispose() {
    if (disposed) return
    disposed = true
    // Withdraw and scrub every controller before an observer can borrow again.
    const children = [...members.values(), ...(content ? [content] : [])]
    members.clear(); content = null
    for (const release of [...children.map(child => () => child.dispose()), stopAccess, stopPrivacy,
      () => options.signal.removeEventListener('abort', dispose)]) {
      try { release() } catch { /* Release all independent children and registrations. */ }
    }
  }
  const moderator: ModeratorAccess = {
    read: () => current() ? access.moderator.read() : null,
    subscribe: listener => access.subscribe(() => { if (!current()) dispose(); listener() }),
  }
  try {
    if (!current()) throw Error('Moderation decision unavailable')
    content = createContentReview({ auth, access: moderator, now: access.moderatorNow })
    stopAccess = access.subscribe(() => { if (!current()) dispose() })
    stopPrivacy = auth.privacy.registerCleanup(dispose)
    options.signal.addEventListener('abort', dispose, { once: true })
    if (!current()) dispose()
  } catch { dispose() }
  return Object.freeze({ decision, isCurrent: current, dispose,
    content() { if (!current()) { dispose(); return null }; return content },
    /** Called by the route binding effect, never during React render. No eviction. */
    member(memberId: string) {
      if (!current()) { dispose(); return null }
      if (!uuid(memberId)) return null
      const existing = members.get(memberId)
      if (existing) return existing
      if (members.size >= MEMBER_REVIEW_LIMIT) return null
      const controller = createMemberReview({ auth, access: moderator, memberId, now: access.moderatorNow })
      if (!current()) { controller.dispose(); dispose(); return null }
      members.set(memberId, controller)
      return controller
    },
  })
}
export type MemberContentEntry = ReturnType<typeof createMemberContentEntry>
