import { useEffect, useRef, useState, useSyncExternalStore } from 'react'
import type { EventClosure as EventClosureController } from './controller'
import '../mural/admin.css'
const labels = { READ_ONLY: 'Make event read-only', FREEZE_MURAL: 'Freeze final wall', SNAPSHOT: 'Capture final snapshot', EXPIRE_GRANTS: 'Expire sharing grants', EXPIRE_BEACONS: 'Expire meeting beacons', EXPORT: 'Export permitted archive', CLEANUP: 'Complete retention cleanup' }
export function EventClosure({ controller }: { controller: EventClosureController }) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read), [reason, setReason] = useState(''), [jobId, setJobId] = useState(''), [invalid, setInvalid] = useState(false)
  const focus = useRef<HTMLDivElement>(null), busy = state.commandPhase === 'sending' || state.commandPhase === 'checking' || state.phase === 'loading'
  useEffect(() => { if (state.draft) focus.current?.focus() }, [state.draft])
  useEffect(() => { if (state.phase === 'unavailable' || state.phase === 'disposed') { setReason(''); setJobId(''); setInvalid(false) } }, [state.phase])
  return <section className="mcr-ops" aria-label="Event closure"><header><p>Event operations</p><h1>CLOSE THE EVENT</h1></header>
    <p>Make the event read-only, freeze the final wall, expire private access and prepare the permitted archive. Completed cleanup steps remain complete.</p>
    <p role="status" aria-label="Event closure status" aria-live="polite">{state.message}</p>
    {!state.command && !state.jobId && !state.draft && state.phase !== 'unavailable' && state.phase !== 'disposed' && <form className="mcr-ops__form" aria-label="Prepare event closure" onSubmit={e => { e.preventDefault(); setInvalid(!controller.prepare(reason)) }}>
      <label>Closure reason<textarea required value={reason} onChange={e => setReason(e.target.value)} maxLength={500} rows={3} /></label><button type="submit" disabled={busy}>Review event closure</button>
    </form>}
    {invalid && <p role="alert">A current operator must provide a reason for an open or live event.</p>}
    {state.draft && <div role="group" aria-label="Confirm event closure" tabIndex={-1} ref={focus} className="mcr-ops__review"><h2>Close this event?</h2>
      <p>This immediately stops new event interactions. Archival completes only after every required cleanup step succeeds.</p>
      <p>Event revision {state.draft.expected_revision}. Reason: {state.draft.reason}</p>
      <div className="mcr-ops__actions"><button onClick={() => { setReason(''); void controller.confirm() }}>Confirm close event</button><button onClick={controller.cancel}>Cancel event closure</button></div>
    </div>}
    {state.command && <><p>Original closure: {state.commandPhase}. No new closure will be sent.</p><button disabled={busy} onClick={() => void controller.check()}>Check original closure command</button></>}
    {state.jobId && <><h2>Closure progress</h2><p>Job <code>{state.jobId}</code></p>
      {state.job && <><p aria-label="Closure job state">{state.job.state} · revision {state.job.revision}</p><ol aria-label="Closure steps" className="mcr-ops__steps">{state.job.steps.map(s => <li key={s.name}><strong>{labels[s.name]}</strong> — {s.state}</li>)}</ol>
        {state.job.state === 'PARTIAL_FAILURE' && <><p>Some steps failed. Refresh progress to check whether the server has resumed them.</p><button disabled>Retry failed closure steps</button><p>Job recovery is currently unavailable. Contact the event operator if progress does not resume.</p></>}
      </>}
      <button disabled={busy} onClick={() => void controller.refresh()}>Refresh closure progress</button>
    </>}
    {!state.command && !state.jobId && state.phase !== 'unavailable' && state.phase !== 'disposed' && <form className="mcr-ops__form" aria-label="Resume closure observation" onSubmit={e => { e.preventDefault(); void controller.refresh(jobId) }}>
      <label>Existing closure job ID<input required value={jobId} onChange={e => setJobId(e.target.value)} maxLength={36} autoComplete="off" spellCheck={false} pattern="[0-9a-f-]{36}" /></label>
      <button type="submit" disabled={busy}>Load existing closure job</button>
    </form>}
  </section>
}
