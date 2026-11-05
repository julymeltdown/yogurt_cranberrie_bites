import { useLayoutEffect, useRef } from 'react'
import { Link, useLocation } from 'react-router-dom'
import { useRetainedAccount } from './context'
import '../../features/auth/ui/parade-ink-auth/entry.css'
export function AccountDeletionRecovery() {
  const { controller } = useRetainedAccount(), location = useLocation(), heading = useRef<HTMLHeadingElement>(null)
  const state = controller?.read()
  useLayoutEffect(() => { heading.current?.focus() }, [location.key])
  const retained = state && ['requesting', 'checking', 'pending', 'partial-failure', 'unknown', 'complete'].includes(state.phase)
  return <main className="parade-auth-entry parade-deletion-entry" id="main">
    <h1 ref={heading} tabIndex={-1}>Account deletion status</h1>
    {!retained ? <><p role="status">No deletion completion is confirmed in this session. Retained recovery is unavailable after expiry, sign-out or a session change.</p>
      <Link className="shell-link" to="/login">Sign in</Link></>
      : <><p role="status">{state.message}</p>
        {state.progress && <ul aria-label="Deletion progress">{state.progress.steps.map(step => <li key={step.target}>{step.target.toLowerCase().replaceAll('_', ' ')}: {step.state.toLowerCase().replaceAll('_', ' ')}</li>)}</ul>}
        {['pending', 'partial-failure', 'unknown'].includes(state.phase) && <button type="button" onClick={() => void controller?.checkDeletion()}>Check deletion progress</button>}
        {state.phase !== 'complete' && <button type="button" onClick={() => void controller?.logout()}>Sign out</button>}
      </>}
    <p>Completion requires all eight cleanup steps. A pending request or disabled account is not completed cleanup.</p>
    <Link className="shell-link" to="/">Back to home</Link>
  </main>
}
