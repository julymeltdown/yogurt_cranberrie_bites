import { useEffect, useRef, useState, useSyncExternalStore } from 'react'
import { Link } from 'react-router-dom'
import { usePorts } from '../../../app/access'
import type { EnrollmentAdmission } from '../model/enrollment-contract'
import { exactAcknowledgements } from '../model/enrollment-contract'
import { EnrollmentPolicy } from './EnrollmentPolicy'
import { enrollmentMessages } from './SignUp'
import './enrollment.css'
const noSubscribe = () => () => {}, noSnapshot = () => null
/** Root owns mounting before bootstrap and the canonical continuation. No side-effectful constructor. */
export function ConsentAdmission({ onAdmitted }: { onAdmitted?(receipt: EnrollmentAdmission): void }) {
  const auth = usePorts().auth, enrollment = auth?.enrollment
  const state = useSyncExternalStore(enrollment?.subscribe ?? noSubscribe, enrollment?.read ?? noSnapshot)
  const [ids, setIds] = useState<string[]>([]), [now, setNow] = useState(() => performance.now())
  const submitting = useRef(false), feedback = useRef<HTMLDivElement>(null)
  const policy = state?.policy, kind = state?.result?.kind, pending = Boolean(state?.pending)
  const expired = Boolean(state?.policyDeadline && now >= state.policyDeadline)
  const wait = Math.max(0, Math.ceil(((state?.retryDeadline ?? 0) - now) / 1000))
  useEffect(() => { const timer = setInterval(() => setNow(performance.now()), 250); return () => { clearInterval(timer); enrollment?.cancel() } }, [enrollment])
  useEffect(() => { if (kind && kind !== 'ready' && !pending) feedback.current?.focus({ preventScroll: true }) }, [kind, pending])
  async function continueAccount(retry = false) {
    if (!enrollment || submitting.current) return
    submitting.current = true
    try {
      const result = retry ? await enrollment.retryUnknown() : await enrollment.acceptPolicy(ids)
      if (result.kind !== 'admitted') return
      const checked = await enrollment.checkAdmission(), receipt = enrollment.read().admission
      if (checked.kind === 'admitted' && receipt?.decision === 'ADMITTED') onAdmitted?.(receipt)
    } finally { submitting.current = false }
  }
  return <main className="auth-enrollment" id="main" tabIndex={-1}>
    <Link className="enrollment-home" to="/">MCR Concert World</Link>
    <h1>Review current agreements</h1>
    <p>Your account needs the current published agreements before entering the concert world. Restricted features have separate eligibility checks.</p>
    <div ref={feedback} role="status" aria-live="polite" tabIndex={-1} className="enrollment-feedback">
      {pending ? <p>Checking your account request…</p> : kind && kind !== 'ready' && <p>{enrollmentMessages[kind]}</p>}
    </div>
    <button type="button" disabled={pending || Boolean(state?.unknown) || wait > 0} onClick={() => void enrollment?.loadPolicy()}>Load current agreements</button>
    {!policy && <p>{enrollmentMessages['policy-unavailable']}</p>}
    {expired && <p role="status">These agreements expired. Load the current agreements before continuing.</p>}
    {policy && <EnrollmentPolicy policy={policy} disabled={pending || expired || Boolean(state?.unknown)} onChange={setIds} />}
    {wait > 0 && <p>Try again in {wait} seconds.</p>}
    {state?.unknown ? <button type="button" disabled={pending || wait > 0} onClick={() => void continueAccount(true)}>Retry original request</button>
      : <button className="enrollment-submit" type="button" disabled={pending || wait > 0 || !policy || expired || !exactAcknowledgements(policy, ids)} onClick={() => void continueAccount()}>Agree and continue</button>}
    <Link className="enrollment-link enrollment-consent-back" to="/">Back to home</Link>
  </main>
}
