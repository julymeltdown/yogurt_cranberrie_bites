import { createContext, useContext, useLayoutEffect, useState, useSyncExternalStore, useCallback, type ReactNode } from 'react'
import type { PrivacyScope } from '../../shared/query/privacy'
import { createMediaOwner, type MediaOwner, type MediaOwnerAuth } from './index'

export const ROOT_MEDIA_LIMITS = Object.freeze({ maxEntries: 8, maxConsumers: 32 })
export const mediaScopeKey = (scope: PrivacyScope) => JSON.stringify([scope.generation, scope.userId, scope.eventId, scope.policyGeneration, scope.gate])
export interface RootMediaBinding { readonly auth: MediaOwnerAuth; readonly scope: PrivacyScope; readonly owner: MediaOwner }
export interface RootMediaProviderProps { readonly auth: MediaOwnerAuth | null | undefined; readonly scope: PrivacyScope | null; readonly children: ReactNode }
const Context = createContext<RootMediaBinding | null>(null)
const none = () => () => undefined

/** Place above panel routes, inside the validated shell. It grants no readiness. */
export function RootMediaProvider({ auth, scope, children }: RootMediaProviderProps) {
  const expected = scope ? mediaScopeKey(scope) : null
  const subscribe = useCallback((notify: () => void) => {
    try { return auth ? auth.privacy.subscribe(() => notify()) : none() } catch { return none() }
  }, [auth])
  const snapshot = useCallback(() => auth ? mediaScopeKey(auth.privacy.capture()) : null, [auth])
  const observed = useSyncExternalStore(subscribe, snapshot, snapshot)
  const [binding, setBinding] = useState<RootMediaBinding | null>(null)
  useLayoutEffect(() => {
    const captured = auth?.privacy.capture()
    if (!auth || !captured || mediaScopeKey(captured) !== expected || !auth.privacy.isCurrent(captured)) return
    let owner: MediaOwner
    try { owner = createMediaOwner({ auth, scope: captured, ...ROOT_MEDIA_LIMITS }) } catch { return }
    setBinding({ auth, scope: captured, owner })
    return owner.dispose
  }, [auth, expected, observed])
  const current = binding && binding.auth === auth && mediaScopeKey(binding.scope) === expected && expected === observed
    && auth?.privacy.isCurrent(binding.scope) && !binding.owner.inspect().disposed ? binding : null
  return <Context value={current}>{children}</Context>
}
export function useRootMediaOwner() { return useContext(Context) }
