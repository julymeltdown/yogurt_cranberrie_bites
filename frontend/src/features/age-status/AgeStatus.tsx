import './parade-ink-private/private.css'
import { useEffect, useSyncExternalStore } from 'react'
import { Link } from 'react-router-dom'
import type { AgeStatusController } from './controller'
import '../people/people.css'
const labels = { UNKNOWN: 'Not verified', VERIFIED: 'Verification current', EXPIRED: 'Verification expired', UNSUPPORTED: 'Verification unsupported', PROVIDER_UNAVAILABLE: 'Verification provider unavailable', REVOKED: 'Verification revoked' }
export function AgeStatus({ controller }: { controller: AgeStatusController }) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read)
  useEffect(() => { controller.start() }, [controller])
  return <section className="intro-surface pi-private-age"><h1>AGE &amp; PRIVACY</h1><p>Verified age and claims stay private. Sharing eligibility is checked by the server for each request.</p>
    <h2>{!state.available ? 'Verification status unavailable' : state.status.phase === 'loading' ? 'Checking verification…' : labels[state.effective]}</h2>
    <p>Self-declared information does not grant verified status. Unavailable verification never unlocks restricted content, and does not by itself block basic world use.</p>
    {state.status.data?.expires_at && <p>Verification validity ends: <time>{state.status.data.expires_at}</time></p>}
    <p role="status" aria-live="polite">{state.command.message || state.message}</p>
    <div className="intro-actions"><button disabled={!controller.canVerify()} onClick={() => void controller.startVerification()}>Start verification</button><button disabled={!state.available || state.command.locked} onClick={() => void controller.load()}>Refresh age status</button></div>
    {!controller.canVerify() && !state.pending && <p>Verification is currently unavailable. You can check again later.</p>}
    {state.pending && <p><a href={state.pending.authorization_url} rel="noreferrer noopener">Continue with verification provider</a></p>}
    {state.command.locked && ['pending', 'uncertain', 'reconciling', 'failed'].includes(state.command.phase) && <button onClick={() => void controller.checkStatus()}>Check original command</button>}
    <p>Service sharing conditions are separate from ticket or event admission rules.</p><Link to="/meet/people">Back to people</Link>
  </section>
}
