import { createContext, useContext, useLayoutEffect, useState, type ReactNode } from 'react'
import type { SessionController } from '../../features/auth/model/session'
import type { ShellBootstrap } from '../ports'
import type { PrivacyScope } from '../../shared/query/privacy'
import type { AccountAssetsPort } from './assets'
import { createAccountOwner, type AccountOwner } from './owner'

const Context = createContext<AccountOwner | null>(null)
/** Stable shell provider: ordinary panel navigation must not unmount this owner. */
export default function AccountProvider({ auth, bootstrap, privacyScope, assets, children }: {
  auth?: SessionController | undefined; bootstrap: ShellBootstrap; privacyScope: PrivacyScope | null;
  assets?: AccountAssetsPort | undefined; children: ReactNode
}) {
  const [owner, setOwner] = useState<AccountOwner | null>(null)
  useLayoutEffect(() => {
    if (!auth || !privacyScope || !auth.privacy.isCurrent(privacyScope)) return
    let candidate: AccountOwner
    try { candidate = createAccountOwner({ auth, bootstrap, scope: privacyScope, assets }) } catch { return }
    setOwner(candidate)
    return candidate.dispose
  }, [auth, bootstrap, privacyScope, assets])
  const current = owner && !owner.isDisposed() && owner.auth === auth && owner.bootstrap === bootstrap && owner.scope === privacyScope
    && owner.assets === assets && privacyScope && auth?.privacy.isCurrent(privacyScope) ? owner : null
  return <Context value={current}>{children}</Context>
}
export function useAccountOwner() { return useContext(Context) }
