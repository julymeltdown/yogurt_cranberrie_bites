import { createAccountController, type AccountController } from '../../features/account/controller'
import type { SessionController } from '../../features/auth/model/session'
import type { PrivacyScope } from '../../shared/query/privacy'

type Navigate = (to: string | number, replace?: boolean) => void
const same = (a: PrivacyScope, b: PrivacyScope) => a.generation === b.generation && a.userId === b.userId && a.eventId === b.eventId && a.policyGeneration === b.policyGeneration && a.gate === b.gate
/** Retains only the existing consumer and an in-memory navigation opener. No world or Auth resources. */
export function createAccountDeletionEntry(auth: SessionController, navigate: Navigate) {
  let disposed = false, controller: AccountController | null = null, created: PrivacyScope | null = null
  let opener: { key: string; scope: PrivacyScope } | null = null, stop = () => {}
  let snapshot = Object.freeze({ controller }) as Readonly<{ controller: AccountController | null }>
  const listeners = new Set<() => void>()
  const emit = () => { snapshot = Object.freeze({ controller }); for (const f of listeners) { try { f() } catch { /* Isolate view observers. */ } } }
  return {
    auth, read: () => snapshot,
    subscribe(f: () => void) { listeners.add(f); return () => { listeners.delete(f) } },
    acquire() {
      const scope = auth.privacy.capture()
      if (disposed || !auth.privacy.isCurrent(scope)) return
      if (controller && created && same(created, scope) && controller.read().phase !== 'closed') return
      stop(); controller?.dispose(); created = scope
      controller = createAccountController({ auth, deletion: auth.accountDeletion,
        onDeletionRestricted: () => navigate('/account/deleted', true),
        onDeletionComplete: () => navigate('/account/deleted', true),
        onLogout: () => navigate('/login', true),
      })
      stop = controller.subscribe(emit); emit()
    },
    rememberSettings(key: string) { const scope = auth.privacy.capture(); if (!disposed && auth.privacy.isCurrent(scope)) opener = { key, scope } },
    back(state: unknown) {
      const key = state && typeof state === 'object' && 'shellPreviousKey' in state ? state.shellPreviousKey : null
      if (opener && key === opener.key && auth.privacy.isCurrent(opener.scope)) navigate(-1)
      else navigate('/me/settings', true)
    },
    dispose() { if (disposed) return; disposed = true; stop(); controller?.dispose(); controller = null; created = null; opener = null; emit(); listeners.clear() },
  }
}
export type AccountDeletionEntry = ReturnType<typeof createAccountDeletionEntry>
