import { useEffect, useLayoutEffect, useRef, useState, useSyncExternalStore, type FormEvent } from 'react'
import { Link, useLocation, useNavigate } from 'react-router-dom'
import { usePorts } from '../../../app/access'
import type { EnrollmentKind } from '../model/enrollment'
import { enrollmentUuid, exactAcknowledgements, validVerificationCode, validEnrollmentEmail, validEnrollmentPassword } from '../model/enrollment-contract'
import { EnrollmentPolicy } from './EnrollmentPolicy'
import './enrollment.css'
export const enrollmentMessages: Record<EnrollmentKind, string> = {
  ready: 'Ready.',
  created: 'Request accepted. If email verification is available for this address, instructions may arrive. Account creation and email delivery are not confirmed here.',
  verified: 'Email verified. Sign in to continue.',
  received: 'Request accepted. Email delivery is not confirmed. Use the latest code if instructions arrive.',
  admitted: 'Your account meets the current consent requirement. Continuing requires the server admission check.',
  'consent-required': 'Read and accept the current published agreements to continue.',
  'invalid-input': 'Check your form details. Passwords must be 8–128 UTF-8 bytes; verification codes contain exactly six digits.',
  'invalid-code': 'We could not verify this code. It may be invalid, expired or already used. You can request another code.',
  'rate-limited': 'Wait before trying again. The server controls the resend time and request limits.',
  unavailable: 'This account action is unavailable right now. Please try again later.',
  uncertain: 'The request may have completed, but its result is unknown. An explicit retry uses the same request; it does not create a new attempt.',
  cancelled: 'We stopped waiting. The result is not confirmed.',
  busy: 'Another account action is in progress. Wait for it to finish before trying again.',
  'policy-unavailable': 'Signup agreements are unavailable. Account creation cannot continue until the approved documents are published.',
  'policy-stale': 'The published agreements changed. Reload them and review every agreement before starting again.',
  'attempt-expired': 'This signup attempt expired. Start again with the current published agreements.',
  'attempt-invalid': 'This attempt is unavailable in this browser. Open the instructions in the original browser, or start a new signup attempt.',
  'acknowledgements-required': 'Read and explicitly accept every required agreement.',
  'idempotency-conflict': 'This request does not match the original attempt. Do not submit changed details to the same attempt.',
  'account-inactive': 'This account cannot continue. Contact support if you need help.',
}
const noSubscribe = () => () => {}, noSnapshot = () => null
export function SignUp() { return <EnrollmentView mode="signup" /> }
/** Query secrets are captured once and removed before any account request. */
export function EnrollmentView({ mode }: { mode: 'signup' | 'verify' }) {
  const auth = usePorts().auth, location = useLocation(), navigate = useNavigate(), enrollment = auth?.enrollment
  const state = useSyncExternalStore(enrollment?.subscribe ?? noSubscribe, enrollment?.read ?? noSnapshot)
  const authState = useSyncExternalStore(auth?.subscribe ?? noSubscribe, auth?.read ?? noSnapshot)
  const [email, setEmail] = useState(''), [password, setPassword] = useState(''), [code, setCode] = useState(''), [attemptId, setAttemptId] = useState('')
  const [ids, setIds] = useState<string[]>([]), [now, setNow] = useState(() => performance.now()), [changed, setChanged] = useState(false)
  const heading = useRef<HTMLHeadingElement>(null), feedback = useRef<HTMLDivElement>(null)
  const passwordInput = useRef<HTMLInputElement>(null), codeInput = useRef<HTMLInputElement>(null)
  const submitting = useRef(false), captured = useRef<string | null>(null)
  const dirty = Boolean(location.search || location.hash || location.state !== null && location.state !== undefined)
  const available = Boolean(enrollment?.available() && !auth?.session.read().userId)
  const policy = state?.policy, pending = Boolean(state?.pending), kind = state?.result?.kind
  const id = attemptId || state?.accepted?.attemptId || state?.attempt?.attemptId || ''
  const expired = Boolean(state?.attemptDeadline && state.attemptDeadline <= now)
  const policyExpired = Boolean(state?.policyDeadline && state.policyDeadline <= now)
  const remaining = Math.max(0, Math.ceil(((state?.retryDeadline ?? 0) - now) / 1000))
  const resendRemaining = Math.max(remaining, state?.accepted?.attemptId === id ? Math.ceil((state.resendDeadline - now) / 1000) : 0)
  function clearSecrets() {
    setPassword(''); setCode('')
    if (passwordInput.current) passwordInput.current.value = ''
    if (codeInput.current) codeInput.current.value = ''
  }
  useLayoutEffect(() => {
    if (dirty) {
      if (captured.current !== location.key) {
        captured.current = location.key
        const query = new URLSearchParams(location.search), keys = [...query.keys()]
        const allowed = mode === 'verify' && !location.hash && keys.every(key => key === 'attemptId' || key === 'code')
          && keys.length === new Set(keys).size
        clearSecrets()
        if (allowed) {
          const value = query.get('attemptId') ?? '', prefill = query.get('code') ?? ''
          setAttemptId(enrollmentUuid.safeParse(value).success ? value : '')
          setCode(validVerificationCode(prefill) ? prefill : '')
        }
      }
      void navigate(mode === 'signup' ? '/signup' : '/verify-email', { replace: true, state: null })
    } else heading.current?.focus({ preventScroll: true })
  }, [dirty, location.key, location.search, location.hash, mode, navigate])
  useEffect(() => {
    const timer = setInterval(() => setNow(performance.now()), 250)
    return () => clearInterval(timer)
  }, [])
  useEffect(() => {
    const stop = auth?.privacy.registerCleanup(() => {
      clearSecrets(); setEmail(''); setAttemptId(''); setIds([]); setChanged(true)
    })
    return () => { stop?.(); enrollment?.cancel()
      if (passwordInput.current) passwordInput.current.value = ''
      if (codeInput.current) codeInput.current.value = '' }
  }, [auth, enrollment])
  useLayoutEffect(() => { if (kind && kind !== 'ready' && !pending && !dirty) feedback.current?.focus({ preventScroll: true }) }, [kind, pending, dirty])
  async function submit(action: 'signup' | 'verify' | 'resend' | 'retry') {
    if (!enrollment || submitting.current || dirty) return
    submitting.current = true
    const enteredPassword = password, enteredCode = code
    clearSecrets(); setChanged(false)
    try {
      if (action === 'signup') {
        if (!validEnrollmentEmail(email) || !validEnrollmentPassword(enteredPassword)) { await enrollment.signup(email, enteredPassword, ids); return }
        if (!state?.attempt && (await enrollment.beginAttempt()).kind !== 'ready') return
        await enrollment.signup(email, enteredPassword, ids)
      } else if (action === 'verify') await enrollment.verify(id, enteredCode)
      else if (action === 'resend') await enrollment.resend(id)
      else await enrollment.retryUnknown()
    } finally { submitting.current = false; setNow(performance.now()) }
  }
  function onSubmit(event: FormEvent<HTMLFormElement>) { event.preventDefault(); void submit(mode) }
  const disabled = pending || !available || remaining > 0
  const completed = kind === 'verified'
  return <main className="auth-enrollment" id="main" tabIndex={-1}>
    <Link className="enrollment-home" to="/">MCR Concert World</Link>
    <p className="enrollment-eyebrow">BEFORE THE SHOW</p>
    <h1 ref={heading} tabIndex={-1}>{mode === 'signup' ? 'Create account' : 'Verify email'}</h1>
    {!dirty && <>
      <p className="enrollment-intro">{mode === 'signup' ? 'One account for your concert world.' : 'Use the attempt and six-digit code from your instructions in the browser where you started signup.'}</p>
      {!available && <><p role="status">Email account actions are not available right now.</p>
        {auth && <button type="button" disabled={pending || authState?.capabilities === 'loading'} onClick={() => void auth.checkEnrollmentAvailability()}>Check email availability</button>}</>}
      <div className="enrollment-feedback" ref={feedback} role="status" aria-live="polite" tabIndex={-1}>
        {pending ? <p>Checking your account request…</p> : changed ? <p>Your session changed. Account details have been cleared.</p>
          : kind && kind !== 'ready' && <p>{enrollmentMessages[kind]}</p>}
      </div>
      {mode === 'signup' && <>
        {!policy && <p>{enrollmentMessages['policy-unavailable']}</p>}
        <button type="button" disabled={pending || Boolean(state?.unknown)} onClick={() => void enrollment?.loadPolicy()}>Load current agreements</button>
        {policyExpired && <p role="status">These agreements expired. Reload the current agreements.</p>}
      </>}
      {remaining > 0 && <p className="enrollment-help">Server requested a wait. Try again in {remaining} seconds.</p>}
      {expired && <p role="status">This attempt expired. Start again to continue.</p>}
      {state?.unknown && <button type="button" disabled={pending || remaining > 0 || expired} onClick={() => void submit('retry')}>Retry original request</button>}
      {!completed && !(mode === 'signup' && state?.accepted) && <form onSubmit={onSubmit} aria-label={mode === 'signup' ? 'Create account' : 'Verify email'} aria-busy={pending} noValidate>
        {mode === 'signup' ? <>
          <label htmlFor="enrollment-email">Email</label>
          <input id="enrollment-email" name="email" type="email" autoComplete="username" maxLength={254} required disabled={disabled || Boolean(state?.unknown)}
            value={email} onChange={event => setEmail(event.target.value)} />
          <label htmlFor="enrollment-password">Password</label>
          <input id="enrollment-password" ref={passwordInput} name="password" type="password" autoComplete="new-password" required disabled={disabled || Boolean(state?.unknown)}
            value={password} onChange={event => setPassword(event.target.value)} aria-describedby="enrollment-password-help" />
          <p id="enrollment-password-help" className="enrollment-help">8–128 UTF-8 bytes. Spaces count; your password is not changed or trimmed.</p>
          {policy && <EnrollmentPolicy policy={policy} disabled={disabled || policyExpired || Boolean(state?.unknown)} onChange={setIds} />}
        </> : <>
          <label htmlFor="enrollment-attempt">Signup attempt</label>
          <input id="enrollment-attempt" autoComplete="off" spellCheck={false} maxLength={36} disabled={disabled} value={id} onChange={event => setAttemptId(event.target.value)} />
          <label htmlFor="enrollment-code">Verification code</label>
          <input id="enrollment-code" ref={codeInput} name="code" type="password" inputMode="numeric" autoComplete="one-time-code" maxLength={6} required disabled={disabled || expired}
            value={code} onChange={event => setCode(event.target.value)} aria-describedby="enrollment-code-help" />
          <p id="enrollment-code-help" className="enrollment-help">Codes can expire or be replaced. The server decides whether a code is usable.</p>
        </>}
        <button className="enrollment-submit" type="submit" disabled={disabled || expired || (mode === 'signup'
          ? !policy || policyExpired || !exactAcknowledgements(policy, ids) || Boolean(state?.unknown) : !enrollmentUuid.safeParse(id).success)}>{mode === 'signup' ? 'Create account' : 'Verify email'}</button>
        {mode === 'verify' && <><button type="button" disabled={disabled || expired || resendRemaining > 0 || Boolean(state?.unknown) || !enrollmentUuid.safeParse(id).success} onClick={() => void submit('resend')}>Request another code</button>
          {resendRemaining > 0 && <p className="enrollment-help">Another code can be requested in {resendRemaining} seconds.</p>}</>}
      </form>}
      {mode === 'signup' && state?.accepted && <Link className="enrollment-link" to="/verify-email">Continue to email verification</Link>}
      {(expired || kind === 'attempt-invalid' || kind === 'policy-stale' || kind === 'idempotency-conflict') && <button type="button" disabled={pending} onClick={() => { enrollment?.reset(); setAttemptId(''); setIds([]); clearSecrets(); void navigate('/signup') }}>Start again</button>}
      <nav aria-label="Account help">
        {mode === 'signup' && <Link className="enrollment-link" to="/verify-email">I have a verification code</Link>}
        <Link className="enrollment-link" to="/login">Sign in</Link>
        <Link className="enrollment-link" to="/reset-password">Reset password</Link>
      </nav>
    </>}
    <Link className="enrollment-link" to="/">Back to home</Link>
  </main>
}
