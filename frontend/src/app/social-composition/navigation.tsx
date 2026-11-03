import { useCallback, useSyncExternalStore, type ReactNode } from 'react'
import { useBlocker } from 'react-router-dom'
import { useSocialOwner } from './provider'
import './composition.css'
const noSubscribe = () => () => {}, empty = () => null
/** Install inside SocialFeatures in the existing data router. Covers links, Back and POP. */
export function SocialNavigationBoundary({ children }: { readonly children: ReactNode }) {
  const owner = useSocialOwner()
  useSyncExternalStore(owner?.subscribe ?? noSubscribe, owner?.read ?? empty, empty)
  const blocked = useCallback(({ nextLocation }: { nextLocation: { pathname: string; search: string } }) => !!owner && !owner.canNavigate(nextLocation.pathname + nextLocation.search), [owner])
  const blocker = useBlocker(blocked)
  const path = blocker.state === 'blocked' ? blocker.location.pathname + blocker.location.search : null
  return <>{children}{blocker.state === 'blocked' && <div className="social-navigation" role="status"><p>The current action or edit needs attention. Your command identity and draft are retained.</p>
    <button type="button" onClick={() => blocker.reset()}>Stay with current action</button>
    <button type="button" disabled={!path || !owner?.canNavigate(path)} onClick={() => { if (path && owner?.canNavigate(path)) blocker.proceed() }}>Continue navigation</button>
  </div>}</>
}
