import { createContext, useContext, useLayoutEffect, useRef, type ReactNode, type ComponentProps } from 'react'
import { Link, useLocation, useNavigate, useNavigationType } from 'react-router-dom'
import { parentPath, safeReturnTarget } from './routes'

interface Visit { path: string; focusId: string | null }
export const MAX_SHELL_LOCATIONS = 100
const Visits = createContext<Map<string, Visit> | null>(null)
export function NavigationMemory({ children }: { children: ReactNode }) {
  const visits = useRef(new Map<string, Visit>())
  const location = useLocation()
  const navigation = useNavigationType()
  useLayoutEffect(() => {
    const visit = visits.current.get(location.key)
    if (navigation === 'POP' && visit?.focusId) document.getElementById(visit.focusId)?.focus({ preventScroll: true })
    visits.current.set(location.key, { path: location.pathname + location.search, focusId: visit?.focusId ?? null })
    // Bound memory without storing content/drafts or private metadata.
    if (visits.current.size > MAX_SHELL_LOCATIONS) visits.current.delete(visits.current.keys().next().value!)
  }, [location, navigation])
  return <Visits value={visits.current}>{children}</Visits>
}

/** Internal panel links carry only a visited location key; never arbitrary navigation targets. */
export function ShellLink({ to, children, onClick, ...props }: Omit<ComponentProps<typeof Link>, 'to' | 'state'> & { to: string }) {
  const location = useLocation()
  const visits = useContext(Visits)
  const target = safeReturnTarget(to)
  return <Link {...props} to={target} state={{ shellPreviousKey: location.key }} onClick={event => {
    onClick?.(event)
    if (event.defaultPrevented || event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return
    const visit = visits?.get(location.key)
    if (visit) visit.focusId = event.currentTarget.id || null
  }}>{children}</Link>
}

export function usePanelBack() {
  const location = useLocation()
  const navigate = useNavigate()
  const visits = useContext(Visits)
  return () => {
    const parent = parentPath(location.pathname)
    const state: unknown = location.state
    const key = state && typeof state === 'object' && 'shellPreviousKey' in state ? state.shellPreviousKey : null
    // Only traverse history when the actual known origin is this panel's parent.
    // A direct link/foreign history falls back to replacing with its safe parent.
    if (typeof key === 'string' && visits?.get(key)?.path === parent) void navigate(-1)
    else void navigate(parent, { replace: true })
  }
}
