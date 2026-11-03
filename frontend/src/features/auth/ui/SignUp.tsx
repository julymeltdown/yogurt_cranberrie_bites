import { useEffect, useLayoutEffect, useRef, useState, useSyncExternalStore, type FormEvent } from 'react'
import { Link, useLocation, useNavigate } from 'react-router-dom'
import { usePorts } from '../../../app/access'
import type { EnrollmentKind, EnrollmentResult } from '../model/enrollment'
import './enrollment.css'
const messages: Record<EnrollmentKind | 'changed', string> = {
  created: 'Account created. Verify your email before signing in. Email delivery is not confirmed here.',
  verified: 'Email verified. Sign in to continue.',
  received: 'Request received. If verification is available for this address, instructions may arrive. Account details and email delivery are not confirmed here.',
  'invalid-input': 'Check your email and form details. Passwords must be 8–128 UTF-8 bytes; verification codes contain exactly six digits.',
  'invalid-code': 'We could not verify this code. It may be invalid, expired or already used. You can request another code.',
  'not-created': 'We could not create this account. You can try signing in or resetting your password.',
  'rate-limited': 'Too many attempts. Wait before trying again.',
  unavailable: 'This account action is unavailable right now. Please try again later.',
  uncertain: 'Your action may have completed, but we could not confirm the result. Try signing in or request new verification instructions if needed.',
  cancelled: 'We stopped waiting. The result is not confirmed.',
  busy: 'Another account action is in progress. Wait for it to finish before trying again.',
  'policy-unavailable': 'Account creation is not open yet. We are preparing the signup requirements and notices.',
  changed: 'Your session changed. Account details have been cleared.',
}
const noSubscribe = () => () => {}, noSnapshot = () => null
export function SignUp() { return <EnrollmentView mode="signup" /> }
/** Shared presentation lifetime for mapped signup and verification routes. */
export function EnrollmentView({ mode }: { mode: 'signup' | 'verify' }) {
  const auth = usePorts().auth, location = useLocation(), navigate = useNavigate()
  const authState = useSyncExternalStore(auth?.subscribe ?? noSubscribe, auth?.read ?? noSnapshot)
  const [email, setEmail] = useState(''), [password, setPassword] = useState(''), [code, setCode] = useState('')
  const [eligibility, setEligibility] = useState(false), [consent, setConsent] = useState(false)
  const [pending, setPending] = useState<'signup' | 'verify' | 'resend' | null>(null)
  const [result, setResult] = useState<EnrollmentResult | { kind: 'changed' } | null>(null)
  const [now, setNow] = useState(Date.now)
  const [lastAction, setLastAction] = useState<'signup' | 'verify' | 'resend'>(mode)
  const heading = useRef<HTMLHeadingElement>(null), feedback = useRef<HTMLDivElement>(null)
  const passwordInput = useRef<HTMLInputElement>(null), codeInput = useRef<HTMLInputElement>(null)
  const alive = useRef(false), submitting = useRef(false), attempt = useRef(0)
  const dirty = Boolean(location.search || location.hash || location.state !== null && location.state !== undefined)
  const synthetic = auth?.enrollment.syntheticQualification === true
  const available = Boolean(auth?.enrollment.available() && !auth.session.read().userId)
  const policyReady = mode !== 'signup' || synthetic
  const retryAt = auth?.enrollment.retryAt() ?? 0
  const remaining = Math.max(0, Math.ceil((retryAt - now) / 1000))
  const completed = result?.kind === 'created' || result?.kind === 'verified'
  function clearSecrets() {
    setPassword(''); setCode('')
    if (passwordInput.current) passwordInput.current.value = ''
    if (codeInput.current) codeInput.current.value = ''
  }
  useLayoutEffect(() => {
    ++attempt.current; submitting.current = false; auth?.enrollment.cancel()
    clearSecrets(); setEmail(''); setEligibility(false); setConsent(false); setPending(null); setResult(null)
    if (dirty) void navigate(mode === 'signup' ? '/signup' : '/verify-email', { replace: true, state: null })
    else heading.current?.focus({ preventScroll: true })
  }, [auth, location.key, dirty, mode, navigate])
  useEffect(() => {
    alive.current = true
    const unregister = auth?.privacy.registerCleanup(() => {
      ++attempt.current; submitting.current = false; setPending(null); clearSecrets(); setEmail('')
      setEligibility(false); setConsent(false); setResult({ kind: 'changed' })
    })
    return () => { alive.current = false; ++attempt.current; auth?.enrollment.cancel(); unregister?.()
      if (passwordInput.current) passwordInput.current.value = ''
      if (codeInput.current) codeInput.current.value = '' }
  }, [auth])
  useEffect(() => {
    if (retryAt <= Date.now()) return
    const timer = setInterval(() => {
      const current = Date.now(); setNow(current)
      if (current >= retryAt) clearInterval(timer)
    }, 250)
    return () => clearInterval(timer)
  }, [retryAt])
  useLayoutEffect(() => { if (result && !pending && !dirty) feedback.current?.focus({ preventScroll: true }) }, [result, pending, dirty])
  async function submit(kind: 'signup' | 'verify' | 'resend') {
    if (submitting.current || dirty) return
    submitting.current = true
    const ticket = ++attempt.current, expected = auth?.session.read(), enteredPassword = password, enteredCode = code
    clearSecrets(); setPending(kind); setLastAction(kind); setResult(null)
    const outcome = !auth ? { kind: 'unavailable' as const } : kind === 'signup'
      ? await auth.enrollment.signup(email, enteredPassword, { eligibility, consent })
      : kind === 'verify' ? await auth.enrollment.verify(email, enteredCode) : await auth.enrollment.resend(email)
    if (alive.current && ticket === attempt.current && (!auth || expected && auth.session.isCurrent(expected))) {
      setResult(outcome); setNow(Date.now()); setPending(null); submitting.current = false
    }
  }
  function onSubmit(event: FormEvent<HTMLFormElement>) { event.preventDefault(); void submit(mode) }
  const disabled = Boolean(pending || !available || !policyReady || remaining > 0)
  return <main className="auth-enrollment" id="main" tabIndex={-1}>
    <Link className="enrollment-home" to="/">MCR Concert World</Link>
    <p className="enrollment-eyebrow">BEFORE THE SHOW</p>
    <h1 ref={heading} tabIndex={-1}>{mode === 'signup' ? 'Create account' : 'Verify email'}</h1>
    {!dirty && <>
      <p className="enrollment-intro">{mode === 'signup' ? 'One account for your concert world.' : 'Use the email and six-digit code from your verification instructions.'}</p>
      {mode === 'signup' && !synthetic && <p role="status">{messages['policy-unavailable']}</p>}
      {mode === 'signup' && synthetic && <p className="enrollment-preview" role="note">Synthetic preview. These eligibility and consent choices are test inputs. They are not recorded as consent.</p>}
      {!available && <p role="status">Email account actions are not available right now.</p>}
      {mode === 'verify' && !available && auth && <button className="enrollment-availability" type="button"
        disabled={Boolean(pending) || authState?.capabilities === 'loading'} onClick={() => void auth.checkEnrollmentAvailability()}>
        {authState?.capabilities === 'loading' ? 'Checking availability…' : 'Check email availability'}</button>}
      <div className="enrollment-feedback" ref={feedback} role="status" aria-live="polite" tabIndex={-1}>
        {pending ? <p>{pending === 'signup' ? 'Creating your account…' : pending === 'verify' ? 'Checking your code…' : 'Requesting instructions…'}</p>
          : result && <p>{result.kind === 'uncertain' && lastAction === 'resend'
            ? 'Your request may have been processed, but the result and email delivery are not confirmed. You can try again later.' : messages[result.kind]}</p>}
      </div>
      {remaining > 0 && <p className="enrollment-help">Server requested a wait. Try again in {remaining} seconds.</p>}
      {!completed && <form onSubmit={onSubmit} aria-label={mode === 'signup' ? 'Create account' : 'Verify email'} aria-busy={Boolean(pending)} noValidate>
        <label htmlFor="enrollment-email">Email</label>
        <input id="enrollment-email" name="email" type="email" autoComplete="username" maxLength={254} required disabled={disabled}
          value={email} onChange={e => { setEmail(e.target.value); setResult(null) }} />
        {mode === 'signup' ? <>
          <label htmlFor="enrollment-password">Password</label>
          <input id="enrollment-password" ref={passwordInput} name="password" type="password" autoComplete="new-password" maxLength={128} required disabled={disabled}
            value={password} onChange={e => setPassword(e.target.value)} aria-describedby="enrollment-password-help" />
          <p id="enrollment-password-help" className="enrollment-help">8–128 UTF-8 bytes. Spaces count; your password is not changed or trimmed.</p>
          {synthetic && <>
            <label className="enrollment-choice"><input type="checkbox" disabled={disabled} checked={eligibility} onChange={e => setEligibility(e.target.checked)} />I meet the eligibility requirements in this test scenario.</label>
            <label className="enrollment-choice"><input type="checkbox" disabled={disabled} checked={consent} onChange={e => setConsent(e.target.checked)} />I agree to the terms and privacy requirements in this test scenario.</label>
          </>}
        </> : <>
          <label htmlFor="enrollment-code">Verification code</label>
          <input id="enrollment-code" ref={codeInput} name="code" type="password" inputMode="numeric" autoComplete="one-time-code" maxLength={6} required disabled={disabled}
            value={code} onChange={e => setCode(e.target.value)} aria-describedby="enrollment-code-help" />
          <p id="enrollment-code-help" className="enrollment-help">Codes can expire. Request new instructions if your code no longer works.</p>
        </>}
        <button className="enrollment-submit" type="submit" disabled={disabled || mode === 'signup' && (!eligibility || !consent)}>{mode === 'signup' ? 'Create account' : 'Verify email'}</button>
        {mode === 'verify' && <button type="button" disabled={disabled} onClick={() => void submit('resend')}>Request another code</button>}
      </form>}
      {result?.kind === 'created' && <Link className="enrollment-link" to="/verify-email">Continue to email verification</Link>}
      <nav aria-label="Account help">
        {mode === 'signup' && <Link className="enrollment-link" to="/verify-email">I have a verification code</Link>}
        <Link className="enrollment-link" to="/login">Sign in</Link>
        <Link className="enrollment-link" to="/reset-password">Reset password</Link>
        {mode === 'signup' && <><Link className="enrollment-link" to="/legal/terms">Terms</Link><Link className="enrollment-link" to="/legal/privacy">Privacy</Link></>}
      </nav>
    </>}
    <Link className="enrollment-link" to="/">Back to home</Link>
  </main>
}
