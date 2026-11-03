import { useEffect, useRef, useState, useSyncExternalStore } from 'react'
import type { FormEvent } from 'react'
import type { MuralControl } from './controller'
import type { MuralAdminState, MuralIntent } from './contract'
import './admin.css'

function WallForm({ controller, wall }: { controller: MuralControl; wall: MuralAdminState }) {
  const [action, setAction] = useState(wall.state === 'PAUSED' ? 'RESUME' : 'PAUSE'), [reason, setReason] = useState(''), [snapshot, setSnapshot] = useState(wall.snapshot_id)
  const [bounds, setBounds] = useState(['0', '0', '128', '128']), [error, setError] = useState(false)
  function submit(event: FormEvent) {
    event.preventDefault()
    const common = { expected_revision: wall.revision, reason }, rectangle = { x_min: Number(bounds[0]), y_min: Number(bounds[1]), x_max_exclusive: Number(bounds[2]), y_max_exclusive: Number(bounds[3]) }
    const intent: MuralIntent = action === 'RESTORE' ? { operation: 'restoreConcertMural', body: { ...common, snapshot_id: snapshot } }
      : action === 'LOCK' || action === 'UNLOCK' ? { operation: 'lockConcertMuralRegion', body: { ...common, ...rectangle, locked: action === 'LOCK' } }
        : { operation: 'controlConcertMural', body: { ...common, action: action as 'PAUSE' | 'RESUME' | 'FREEZE' } }
    setError(!controller.prepare(intent))
  }
  return <form onSubmit={submit} className="mcr-ops__form" aria-label="Prepare wall action">
    <label>Wall action<select aria-label="Wall action" value={action} onChange={e => { setAction(e.target.value); setError(false) }}>
      <option value="PAUSE" disabled={wall.state !== 'EDITABLE'}>Pause wall</option>
      <option value="RESUME" disabled={wall.state !== 'PAUSED'}>Resume wall</option>
      <option value="LOCK">Lock region</option><option value="UNLOCK">Unlock region</option>
      <option value="RESTORE" disabled={wall.state !== 'PAUSED'}>Restore snapshot</option><option value="FREEZE">Freeze wall permanently</option>
    </select></label>
    {(action === 'LOCK' || action === 'UNLOCK') && <fieldset><legend>Region · inclusive start, exclusive end</legend><div className="mcr-ops__coordinates">
      {['X start', 'Y start', 'X end exclusive', 'Y end exclusive'].map((label, i) => <label key={label}>{label}<input type="number" required step={1} min={i < 2 ? 0 : 1} max={i < 2 ? 127 : 128}
        value={bounds[i]} onChange={e => setBounds(old => old.map((v, j) => i === j ? e.target.value : v))} /></label>)}
    </div></fieldset>}
    {action === 'RESTORE' && <label>Snapshot to restore<input required value={snapshot} onChange={e => setSnapshot(e.target.value)} maxLength={36} autoComplete="off" spellCheck={false} /></label>}
    <label>Reason<textarea required value={reason} onChange={e => setReason(e.target.value)} maxLength={500} rows={3} /></label>
    {error && <p role="alert">Check the action, reason and region or snapshot. The current server state must allow this action.</p>}
    <button type="submit">Review wall action</button>
  </form>
}
export function WallControl({ controller }: { controller: MuralControl }) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read), draft = state.draft, focus = useRef<HTMLDivElement>(null)
  useEffect(() => { if (draft) focus.current?.focus() }, [draft])
  const busy = state.commandPhase === 'sending' || state.commandPhase === 'checking'
  return <section className="mcr-ops" aria-label="Wall control"><header><p>Event operations</p><h1>WALL CONTROL</h1></header>
    <p role="status" aria-label="Wall control status" aria-live="polite">{state.message}</p>
    {state.mural && <><p>Wall {state.mural.state} · revision {state.mural.revision}</p><p>Current snapshot <code>{state.mural.snapshot_id}</code></p>
      <h2>Locked regions</h2>{state.mural.locks.length ? <ul>{state.mural.locks.map((r, i) => <li key={i}>X [{r.x_min}, {r.x_max_exclusive}) · Y [{r.y_min}, {r.y_max_exclusive})</li>)}</ul> : <p>No locked regions.</p>}
      {state.mural.state === 'FROZEN' && <p>The final wall is frozen. Editing, resume and restore are unavailable.</p>}
      {!draft && !state.command && state.phase === 'ready' && state.mural.state !== 'FROZEN' && <WallForm key={state.mural.revision} controller={controller} wall={state.mural} />}
    </>}
    {draft && <div role="group" aria-label="Confirm wall action" tabIndex={-1} ref={focus} className="mcr-ops__review">
      <h2>{draft.operation === 'restoreConcertMural' ? 'Restore this exact snapshot?' : draft.operation === 'lockConcertMuralRegion' ? draft.body.locked ? 'Lock this region?' : 'Unlock this region?' : `${draft.body.action} the wall?`}</h2>
      {draft.operation === 'restoreConcertMural' && <p>Snapshot <strong><code>{draft.body.snapshot_id}</code></strong>. Copy this snapshot into a new revision; keep the wall paused.</p>}
      {draft.operation === 'lockConcertMuralRegion' && <p>X [{draft.body.x_min}, {draft.body.x_max_exclusive}) · Y [{draft.body.y_min}, {draft.body.y_max_exclusive})</p>}
      {draft.operation === 'controlConcertMural' && draft.body.action === 'FREEZE' && <p>Freezing is permanent. The wall cannot resume or be restored afterward.</p>}
      <p>Expected wall revision {draft.body.expected_revision}. Other clients will reload the confirmed wall.</p><p>Reason: {draft.body.reason}</p>
      <div className="mcr-ops__actions"><button type="button" onClick={() => void controller.confirm()}>Confirm wall action</button><button type="button" onClick={controller.cancel}>Cancel wall action</button></div>
    </div>}
    {state.command && <><p>Original wall action: {state.commandPhase}. Another wall action is blocked until its outcome is verified.</p><button disabled={busy} onClick={() => void controller.check()}>Check original wall command</button></>}
    <button disabled={busy || state.phase === 'loading' || state.phase === 'disposed'} onClick={() => void controller.load()}>Refresh wall controls</button>
  </section>
}
