import { createContext, useContext, useEffect, useLayoutEffect, useRef, useState, useSyncExternalStore, type ReactNode } from 'react'
import { useLocation, useNavigate } from 'react-router-dom'
import { usePorts } from '../access'
import type { PrivacyScope } from '../../shared/query/privacy'
import { sameScope } from '../../features/safety/contract'
import { SafetyDialog } from '../../features/safety/Dialog'
import { createSafetyEntry, type SafetyEntry } from './owner'
import '../../features/safety/safety.css'

const Context = createContext<SafetyEntry | null>(null)
const noop = () => () => {}, empty = () => null
export function useSafetyEntry() { return useContext(Context) }

function SafetyOverlay({ entry }: { entry: SafetyEntry }) {
  const state = useSyncExternalStore(entry.subscribe, entry.read), safety = state.controller
  const command = safety?.read(), target = state.target
  if (!target || !safety || !entry.auth.privacy.isCurrent(target.scope)) return state.message ? <p role="status">{state.message}</p> : null
  const busy = ['submitting', 'pending', 'unknown', 'failed', 'closed'].includes(command!.phase)
  return <SafetyDialog title="Block this user?" onDismiss={entry.dismissBlock}>
    <p>This closes affected private interactions. Unblocking never restores previous sharing consent.</p>
    <button type="button" data-initial-focus onClick={entry.dismissBlock}>Cancel block</button>
    <button type="button" disabled={busy} onClick={() => { void entry.confirmBlock() }}>Confirm block</button>
    <p role="status">{state.message || command!.message}</p>
    {['pending', 'unknown', 'failed'].includes(command!.phase) && <button type="button" onClick={() => { void safety.checkStatus() }}>Check original request status</button>}
  </SafetyDialog>
}

/** Retained above the route-switched private shell. Privacy cleanup still clears
 * all controllers/drafts synchronously; this creates no alternate session. */
export function SafetyOwnership({ children }: { children: ReactNode }) {
  const { auth } = usePorts(), location = useLocation(), navigate = useNavigate()
  const path = location.pathname + location.search, route = useRef(path); route.current = path
  const [entry, setEntry] = useState<SafetyEntry | null>(null)
  const [destination, setDestination] = useState<{ auth: NonNullable<typeof auth>; scope: PrivacyScope } | null>(null)
  useLayoutEffect(() => {
    if (!auth) { setEntry(null); return }
    const owner = createSafetyEntry(auth, () => route.current, scope => setDestination({ auth, scope }))
    setEntry(owner); return owner.dispose
  }, [auth])
  const current = entry?.auth === auth ? entry : null
  useLayoutEffect(() => { current?.changeRoute(path) }, [current, path])
  useEffect(() => {
    if (!destination) return
    setDestination(null)
    if (auth === destination.auth && auth.privacy.isCurrent(destination.scope) && sameScope(auth.privacy.capture(), destination.scope)) void navigate('/me/blocks', { replace: true })
  }, [auth, destination, navigate])
  return <Context value={current}>{children}{current && <SafetyOverlay entry={current} />}</Context>
}

export function useSafetySnapshot() {
  const entry = useSafetyEntry()
  const state = useSyncExternalStore(entry?.subscribe ?? noop, entry?.read ?? empty)
  return { entry, state }
}
