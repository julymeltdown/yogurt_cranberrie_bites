import { createContext, useContext, useLayoutEffect, useState, useSyncExternalStore, type ReactNode } from 'react'
import { createSocialOwner, type SocialOwner } from './owner'
import type { SocialOptions } from './ports'
const Context = createContext<SocialOwner | null>(null)
const noSubscribe = () => () => {}, empty = () => null
/** Place once inside the authorized shell, outside route-switched children. */
export function SocialFeatures({ children, ...options }: SocialOptions & { readonly children: ReactNode }) {
  const { auth, bootstrap, scope, availability, navigate, requests, media, avatars, verification } = options
  const [owner, setOwner] = useState<SocialOwner | null>(null)
  useLayoutEffect(() => {
    let candidate: SocialOwner
    try { candidate = createSocialOwner({ auth, bootstrap, scope, availability, navigate, ...(requests ? { requests } : {}), ...(media ? { media } : {}), ...(avatars ? { avatars } : {}), ...(verification ? { verification } : {}) }) } catch { setOwner(null); return }
    setOwner(candidate)
    return candidate.dispose
  }, [auth, bootstrap, scope, availability, navigate, requests, media, avatars, verification])
  useSyncExternalStore(owner?.subscribe ?? noSubscribe, owner?.read ?? empty, empty)
  const current = owner && !owner.isDisposed() && owner.auth === auth && owner.bootstrap === bootstrap && owner.scope === scope
    && owner.options.availability === availability && owner.options.navigate === navigate && owner.options.requests === requests && owner.options.media === media
    && owner.options.avatars === avatars && owner.options.verification === verification && auth.privacy.isCurrent(scope) ? owner : null
  return <Context value={current}>{children}</Context>
}
export function useSocialOwner() { return useContext(Context) }
