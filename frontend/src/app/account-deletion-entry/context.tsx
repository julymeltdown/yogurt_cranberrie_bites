import { createContext, useContext, useEffect, useLayoutEffect, useState, useSyncExternalStore, type ReactNode } from 'react'
import { useNavigate } from 'react-router-dom'
import { usePorts } from '../access'
import type { PrivacyScope } from '../../shared/query/privacy'
import type { SessionController } from '../../features/auth/model/session'
import { createAccountDeletionEntry, type AccountDeletionEntry } from './owner'
const Context = createContext<AccountDeletionEntry | null>(null)
const empty = Object.freeze({ controller: null })
/** Above WorldShell: restrictive cleanup must not destroy manual deletion recovery. */
export function AccountDeletionOwnership({ children }: { children: ReactNode }) {
  const { auth } = usePorts(), navigate = useNavigate()
  const [owner, setOwner] = useState<AccountDeletionEntry | null>(null)
  const [destination, setDestination] = useState<{ to: string | number; replace: boolean; auth: SessionController; scope: PrivacyScope } | null>(null)
  useLayoutEffect(() => {
    if (!auth) { setOwner(null); return }
    const entry = createAccountDeletionEntry(auth, (to, replace = false) => setDestination({ to, replace, auth, scope: auth.privacy.capture() }))
    setOwner(entry); return entry.dispose
  }, [auth])
  useEffect(() => {
    if (!destination) return
    // Privacy invalidation removes private navigation blockers in this commit.
    // Navigate after their teardown, rather than asking a withdrawn owner for permission.
    setDestination(null)
    const current = auth?.privacy.capture(), expected = destination.scope
    if (auth !== destination.auth || !auth.session.isCurrent(expected) || current?.policyGeneration !== expected.policyGeneration || current.gate !== expected.gate) return
    if (typeof destination.to === 'number') void navigate(destination.to)
    else void navigate(destination.to, { replace: destination.replace })
  }, [auth, destination, navigate])
  return <Context value={owner?.auth === auth ? owner : null}>{children}</Context>
}
export function useAccountDeletionEntry() { return useContext(Context) }
export function useRetainedAccount() {
  const owner = useAccountDeletionEntry()
  const snapshot = useSyncExternalStore(owner?.subscribe ?? (() => () => {}), owner?.read ?? (() => empty))
  return { owner, controller: snapshot.controller }
}
