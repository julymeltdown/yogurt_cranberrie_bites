import { useEffect, useRef, useState, useSyncExternalStore, type FormEvent } from 'react'
import { Link, useLocation, useNavigate } from 'react-router-dom'
import { usePorts } from '../../../app/access'
import { safeReturnTarget } from '../../../app/routes'
import type { AuthState } from '../model/session'
import { ConcertAction, ConcertWindow } from '../../../shared/ui/concert-window'
import { Cemetery } from './reference-sign-in/Cemetery'
import { ProviderMark } from './reference-sign-in/ProviderMark'
import './sign-in.css'

const unavailable: AuthState = { readiness: 'unavailable', pending: null, capabilities: 'error', emailEnabled: false, notice: null, logout: 'none' }
const messages = {
  'invalid-input': 'Enter a valid email and a password of 1–128 UTF-8 bytes.',
  credentials: 'We could not sign you in. Check your details or try again later.',
  'rate-limited': 'Too many attempts. Wait a while before trying again.',
  unavailable: 'Sign-in could not be completed. Try again when the service is available.',
  expired: 'Your session has ended. Sign in again to continue.',
  changed: 'Your session changed in another tab. Sign in again to continue.',
} as const
export function SignIn() {
  const auth = usePorts().auth, location = useLocation(), navigate = useNavigate()
  const state = useSyncExternalStore(auth?.subscribe ?? (() => () => undefined), auth?.read ?? (() => unavailable))
  const target = safeReturnTarget(new URLSearchParams(location.search).get('returnTo'))
  const [email, setEmail] = useState(''), [password, setPassword] = useState('')
  const alive = useRef(true), passwordInput = useRef<HTMLInputElement>(null)
  useEffect(() => {
    alive.current = true
    const unregister = auth?.privacy.registerCleanup(() => {
      setPassword(''); setEmail('')
      if (passwordInput.current) passwordInput.current.value = ''
    })
    return () => { alive.current = false; unregister?.() }
  }, [auth])
  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (!auth || state.pending) return
    // Clear the rendered password immediately; only the active request retains it in memory.
    const submitted = password; setPassword('')
    const accepted = await auth.login(email, submitted)
    if (accepted && alive.current && auth.read().readiness === 'ready') void navigate(target, { replace: true })
  }
  const busy = state.pending !== null, signedIn = state.readiness === 'ready'
  return <main className="auth-sign-in auth-sign-in--concert" id="main" tabIndex={-1}>
    <ConcertWindow title={signedIn ? 'You’re signed in' : 'Sign in'} headingLevel={1}
      onClose={() => { void navigate('/') }} closeLabel="Close sign-in and return home">
    <Cemetery edge="top" />
    <div className="auth-paper-content">
    <p className="auth-display-heading" aria-hidden="true">{signedIn ? 'You’re signed in' : 'SIGN IN'}</p>
    <div role="status" aria-live="polite" className="auth-message">
      {state.pending === 'login' && <p>Signing you in…</p>}
      {state.pending === 'logout' && <p>Local access cleared. Signing out…</p>}
      {state.notice && <p>{messages[state.notice]}</p>}
      {state.logout === 'confirmed' && <p>You’re signed out.</p>}
      {state.logout === 'unconfirmed' && <p>Local access is cleared. Server sign-out could not be confirmed.</p>}
    </div>
    {signedIn ? <div className="auth-actions">
      <Link className="auth-submit" to={target}>Continue</Link>
      <button type="button" disabled={busy} onClick={() => { void auth?.logout() }}>Sign out</button>
    </div> : <>
      {state.readiness === 'unavailable' ? <p>Event entry is not open yet. Please check back later.</p> : <>
        {state.capabilities === 'loading' && <p role="status">Checking sign-in options…</p>}
        {state.capabilities === 'error' && <div><p>Sign-in options are unavailable.</p>
          <button type="button" onClick={() => auth?.retryCapabilities()}>Check options again</button></div>}
        {state.capabilities === 'ready' && !state.emailEnabled && <p>Email sign-in is temporarily unavailable.</p>}
        <form onSubmit={submit} aria-label="Email sign-in" aria-busy={busy}>
          <label htmlFor="sign-in-email">Email</label>
          <input id="sign-in-email" name="email" type="email" autoComplete="username" required maxLength={254}
            disabled={busy || !state.emailEnabled} value={email} onChange={event => setEmail(event.target.value)} />
          <label htmlFor="sign-in-password">Password</label>
          <input ref={passwordInput} id="sign-in-password" name="password" type="password" autoComplete="current-password"
            required maxLength={128} disabled={busy || !state.emailEnabled} value={password} onChange={event => setPassword(event.target.value)} />
          <ConcertAction className="auth-submit" type="submit" aria-label={state.pending === 'login' ? 'Signing in…' : 'Sign in with email'} pending={state.pending === 'login'} disabled={busy || !state.emailEnabled}>{state.pending === 'login' ? 'Signing in…' : 'SIGN IN'}</ConcertAction>
        </form>
        <Link className="auth-link auth-forgot" to="/reset-password">Forgot password?</Link>
        <div className="auth-providers" aria-label="Other sign-in options">
          <p className="auth-or">Or continue with</p>
          <div className="auth-provider-buttons">{(['X', 'Discord', 'Google'] as const).map(provider => <button key={provider} type="button" disabled aria-label={`Continue with ${provider}`} aria-describedby="provider-unavailable"><ProviderMark provider={provider} /></button>)}</div>
          <p id="provider-unavailable">Provider sign-in is not available yet.</p>
        </div>
      </>}
      {state.logout === 'unconfirmed' && <button type="button" disabled={busy} onClick={() => { void auth?.logout() }}>Try server sign-out again</button>}
      <p className="auth-create">No account? <Link className="auth-link" to="/signup">Create an account</Link></p>
    </>}
    {!signedIn && new URLSearchParams(location.search).has('returnTo') && <Link className="auth-link auth-back" to={target}>Back to your destination</Link>}
    </div>
    <Cemetery edge="bottom" />
    </ConcertWindow>
  </main>
}
