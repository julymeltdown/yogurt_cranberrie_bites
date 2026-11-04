import type { PrivacyScope } from '../../../shared/query/privacy'
import { sameScope } from '../../../features/mural/sync/contract'
const listeners = new WeakMap<object, Set<{ scope: PrivacyScope; notify(): void }>>()
/** Internal invalidation only: a notification can withdraw/refetch; it cannot grant eligibility. */
export function subscribeMuralEligibilityRefresh(owner: object, scope: PrivacyScope, notify: () => void): () => void {
  let entries = listeners.get(owner)
  if (!entries) { entries = new Set(); listeners.set(owner, entries) }
  const entry = { scope: Object.freeze({ ...scope }), notify }
  entries.add(entry)
  return () => { entries.delete(entry); if (entries.size === 0) listeners.delete(owner) }
}
export function invalidateMuralEligibility(owner: object, scope: PrivacyScope) {
  for (const entry of Array.from(listeners.get(owner) ?? [])) {
    if (sameScope(entry.scope, scope)) { try { entry.notify() } catch { /* Independent subscribers still withdraw. */ } }
  }
}
