import { useSyncExternalStore, type ReactNode } from 'react'
import { Link } from 'react-router-dom'
import { usePorts } from '../../../app/access'
import { ConsentAdmission } from './ConsentAdmission'
import './enrollment.css'
const noSubscribe = () => () => {}, noSnapshot = () => null
/** Protect the existing destination without inventing a route, identity or local admission. */
export function AdmissionBoundary({ children }: { children: ReactNode }) {
  const auth = usePorts().auth
  const state = useSyncExternalStore(auth?.subscribe ?? noSubscribe, auth?.read ?? noSnapshot)
  if (!auth || !auth.session.read().userId || state?.admission === 'ADMITTED') return children
  // Keep the active sign-in form mounted while its canonical read completes, so
  // its existing safe continuation remains live. It cannot mount a private shell.
  if (state?.admission === 'checking' && state.pending === 'login') return children
  if (state?.admission === 'CONSENT_REQUIRED') return <ConsentAdmission onAdmitted={() => { void auth.checkAdmission() }} />
  const checking = state?.admission === 'checking', inactive = state?.admission === 'ACCOUNT_INACTIVE'
  return <main className="auth-enrollment" id="main" tabIndex={-1}>
    <Link className="enrollment-home" to="/">MCR Concert World</Link>
    <h1>{checking ? 'Checking account entry' : inactive ? 'Account entry unavailable' : 'World entry paused'}</h1>
    <p role="status">{checking ? 'Checking the current server admission decision before opening your destination.'
      : inactive ? 'This account cannot enter the concert world. Contact support if you need help.'
        : state?.admission === 'POLICY_UNAVAILABLE' ? 'The current agreements are unavailable. World entry will stay paused until approved documents are published.'
          : 'We could not confirm the current account requirements. Your identity has not been replaced.'}</p>
    {!inactive && <button type="button" disabled={checking || Boolean(state?.admissionPending)} onClick={() => void auth.checkAdmission()}>Check entry again</button>}
    <button type="button" disabled={state?.pending === 'logout'} onClick={() => void auth.logout()}>Sign out</button>
    <Link className="enrollment-link" to="/">Back to home</Link>
  </main>
}
