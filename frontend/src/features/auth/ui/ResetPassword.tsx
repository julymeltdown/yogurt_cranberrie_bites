import { useEffect, useLayoutEffect, useRef, useState, type FormEvent } from 'react'
import { Link, useLocation, useNavigate } from 'react-router-dom'
import { usePorts } from '../../../app/access'
import { readResetLocation } from '../model/callback'
import type { RecoveryResult } from '../model/recovery'
import './recovery.css'

const messages: Record<RecoveryResult | 'invalid-link' | 'changed', string> = {
  received: 'Request received. If password recovery is available for this address, instructions may arrive. Account details and email delivery are not confirmed here.',
  confirmed: 'Password reset completed. Sign in again with your new password.',
  'invalid-input': 'Enter a valid email, or a new password of 8–128 UTF-8 bytes. Passwords are used exactly as entered.',
  'invalid-token': 'We could not use this reset token. It may be invalid, expired or already used. You can request another reset.',
  'invalid-link': 'This reset link could not be used. Request another reset, or enter the token from your reset instructions.',
  'rate-limited': 'Too many attempts. Wait a while before trying again.',
  unavailable: 'Password recovery could not be completed. Please try again when the service is available.',
  uncertain: 'The reset may have completed, but we could not confirm the result. Try signing in, or request another reset if needed.',
  cancelled: 'We stopped waiting. The result is not confirmed.',
  busy: 'Another account action is in progress. Wait for it to finish before trying again.',
  changed: 'Your session changed. Reset details have been cleared.',
}
export function ResetPassword() {
  const auth = usePorts().auth, location = useLocation(), navigate = useNavigate()
  const [mode, setMode] = useState<'request' | 'confirm'>('request')
  const [email, setEmail] = useState(''), [token, setToken] = useState(''), [password, setPassword] = useState('')
  const [linkReady, setLinkReady] = useState(false), [pending, setPending] = useState(false)
  const [result, setResult] = useState<keyof typeof messages | null>(null)
  const incoming = useRef(''), target = useRef('/world'), alive = useRef(false), submitting = useRef(false)
  const attempt = useRef(0)
  const routeKey = useRef(location.key), scrubbing = useRef(false)
  const heading = useRef<HTMLHeadingElement>(null), feedback = useRef<HTMLDivElement>(null)
  const tokenInput = useRef<HTMLInputElement>(null), passwordInput = useRef<HTMLInputElement>(null)
  const dirty = Boolean(location.search || location.hash || location.state !== null && location.state !== undefined)
  const clearSecrets = () => {
    incoming.current = ''; setToken(''); setPassword(''); setLinkReady(false)
    if (tokenInput.current) tokenInput.current.value = ''
    if (passwordInput.current) passwordInput.current.value = ''
  }
  useLayoutEffect(() => {
    const moved = routeKey.current !== location.key; routeKey.current = location.key
    if (!dirty) {
      if (scrubbing.current) scrubbing.current = false
      else if (moved) {
        auth?.cancelRecovery(); ++attempt.current; submitting.current = false; setPending(false)
        clearSecrets(); setEmail(''); setMode('request'); setResult(null); target.current = '/world'
      }
      heading.current?.focus({ preventScroll: true }); return
    }
    scrubbing.current = true
    auth?.cancelRecovery(); ++attempt.current; submitting.current = false; setPending(false)
    setToken(''); setPassword('')
    const intake = readResetLocation(location.search, location.hash)
    incoming.current = intake.token ?? ''; target.current = intake.target
    setLinkReady(Boolean(intake.token)); setMode(intake.token ? 'confirm' : 'request')
    setResult(intake.invalid ? 'invalid-link' : null)
    // Replace, do not push: secrets and arbitrary state must not remain in the active URL.
    void navigate('/reset-password', { replace: true, state: null })
  }, [auth, dirty, location.key, location.search, location.hash, navigate])
  useEffect(() => {
    alive.current = true
    const unregister = auth?.privacy.registerCleanup(() => {
      ++attempt.current; submitting.current = false; setPending(false)
      clearSecrets(); setEmail(''); setResult('changed')
    })
    return () => { alive.current = false; incoming.current = ''; auth?.cancelRecovery(); unregister?.()
      if (tokenInput.current) tokenInput.current.value = ''
      if (passwordInput.current) passwordInput.current.value = '' }
  }, [auth])
  useLayoutEffect(() => { if (result && !pending && !dirty) feedback.current?.focus({ preventScroll: true }) }, [result, pending, dirty])
  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (submitting.current || dirty) return
    submitting.current = true; setPending(true); setResult(null)
    // Keep the link token out of input values, URL state, caches and rendered text.
    const submittedToken = incoming.current || token, submittedPassword = password
    clearSecrets()
    const operation = !auth ? Promise.resolve<RecoveryResult>('unavailable') : mode === 'request'
      ? auth.requestPasswordReset(email) : auth.confirmPasswordReset(submittedToken, submittedPassword)
    // Confirmation synchronously clears the prior identity before its request starts.
    const expected = auth?.session.read(), ticket = ++attempt.current
    submitting.current = true; setPending(true)
    const outcome = await operation
    if (alive.current && ticket === attempt.current && (!auth || (expected && auth.session.isCurrent(expected)))) {
      setResult(outcome); setPending(false)
    }
    if (ticket === attempt.current) submitting.current = false
  }
  function switchMode(next: typeof mode) {
    if (submitting.current) return
    clearSecrets(); setResult(null); setMode(next)
  }
  return <main className="auth-recovery" id="main" tabIndex={-1}>
    <Link className="recovery-home" to="/">MCR Concert World</Link>
    <p className="recovery-eyebrow">YOUR WAY BACK</p>
    <h1 ref={heading} tabIndex={-1}>Reset password</h1>
    {!dirty && <>
      <p className="recovery-intro">{mode === 'request' ? 'Start with the email you use to sign in.' : 'Choose a new password for your account.'}</p>
      <div ref={feedback} className="recovery-feedback" role="status" aria-live="polite" tabIndex={-1}>
        {pending ? <p>{mode === 'request' ? 'Submitting your request…' : 'Confirming your reset…'}</p> : result && <p>{result === 'uncertain' && mode === 'request'
          ? 'Your request may have been processed, but the result and email delivery are not confirmed. You can try again later.' : messages[result]}</p>}
      </div>
      {result !== 'confirmed' && <form onSubmit={submit} aria-label={mode === 'request' ? 'Request password reset' : 'Confirm password reset'} aria-busy={pending} noValidate>
        {mode === 'request' ? <>
          <label htmlFor="reset-email">Email</label>
          <input id="reset-email" name="email" type="email" autoComplete="username" maxLength={254} required disabled={pending || !auth}
            value={email} onChange={event => setEmail(event.target.value)} />
        </> : <>
          {linkReady ? <p>Reset link ready. Enter your new password below.</p> : <>
            <label htmlFor="reset-token">Reset token</label>
            <input ref={tokenInput} id="reset-token" name="token" type="password" autoComplete="off" spellCheck={false} maxLength={8192} required disabled={pending || !auth}
              value={token} onChange={event => setToken(event.target.value)} aria-describedby="reset-token-help" />
            <p id="reset-token-help" className="recovery-help">Use the token from your password reset instructions.</p>
          </>}
          <label htmlFor="reset-password">New password</label>
          <input ref={passwordInput} id="reset-password" name="newPassword" type="password" autoComplete="new-password" maxLength={128} required disabled={pending || !auth}
            value={password} onChange={event => setPassword(event.target.value)} aria-describedby="reset-password-help" />
          <p id="reset-password-help" className="recovery-help">8–128 UTF-8 bytes. Spaces count; your password is not changed or trimmed.</p>
        </>}
        <button className="recovery-submit" type="submit" disabled={pending || !auth}>{mode === 'request' ? 'Request reset' : 'Reset password'}</button>
      </form>}
      {!auth && <p role="status">Password recovery is not available right now.</p>}
      {result !== 'confirmed' && <button className="recovery-switch" type="button" disabled={pending} onClick={() => switchMode(mode === 'request' ? 'confirm' : 'request')}>
        {mode === 'request' ? 'I have a reset token' : 'Request another reset'}</button>}
      <Link className="recovery-link" to={`/login?returnTo=${encodeURIComponent(target.current)}`}>Back to sign in</Link>
    </>}
    <Link className="recovery-link" to="/">Back to home</Link>
  </main>
}
