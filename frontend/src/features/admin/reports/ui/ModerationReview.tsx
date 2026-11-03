import { useEffect, useRef, useSyncExternalStore } from 'react';
import { actions, rowId, rowKey, type Row } from '../model/contract';
import type { ModerationController } from '../model/controller';
import './moderation.css';
const titles = { reports: 'Report queue', members: 'Member review', content: 'Content review' };
const label = (r: Row) => 'report_id' in r ? r.target_type + ' report' : 'user_id' in r ? r.profile.display_name : r.target_type + ' content';
export function ModerationReview({ controller }: {
    controller: ModerationController;
}) {
    const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read), heading = useRef<HTMLHeadingElement>(null), confirm = useRef<HTMLButtonElement>(null), opener = useRef<HTMLButtonElement>(null);
    const locked = ['loading', 'sending', 'pending', 'unknown', 'checking', 'applied'].includes(state.phase), target = state.selected;
    useEffect(() => {
        void controller.load();
    }, [controller]);
    useEffect(() => {
        if (target)
            heading.current?.focus();
    }, [state.epoch, target]);
    useEffect(() => {
        if (state.confirm)
            confirm.current?.focus();
    }, [state.confirm]);
    if (!state.access)
        return <main className="moderation"><h1>Moderator access unavailable</h1><p role="alert">A current server authorization for this event is required. No private records are open.</p></main>;
    return <main className="moderation"><header><p>Event moderation</p><h1>{titles[controller.lane]}</h1><p>Private review. Use only the information needed for this action.</p>
    {controller.lane === 'members' && <p>Suspension applies only to this event. It does not delete an account or change access to other events.</p>}
    <button disabled={['sending', 'pending', 'unknown', 'checking'].includes(state.phase)} onClick={() => void controller.load()}>Refresh records</button></header>
    {state.message && <p role={['failed', 'conflict', 'unknown'].includes(state.phase) ? 'alert' : 'status'}>{state.message}</p>}
    {state.phase === 'loading' && <p role="status">Reading current review records…</p>}
    {!!state.rows.length && <table><caption>{titles[controller.lane]} records</caption><thead><tr><th scope="col">Record</th><th scope="col">State</th><th scope="col">Review</th></tr></thead><tbody>{state.rows.map(r => <tr key={rowKey(r)}><td>{label(r)}<small>Ref {rowId(r).slice(-8)} · v{r.revision}</small></td><td>{r.state}</td><td><button disabled={locked} onClick={() => void controller.select(rowKey(r))}>Review <span className="moderation-sr">{rowId(r)}</span></button></td></tr>)}</tbody></table>}
    {state.phase === 'ready' && !state.rows.length && <p>No records in this page.</p>}
    {state.cursor && <button disabled={locked} onClick={() => void controller.load(true)}>Next page</button>}
    {target && <section aria-labelledby="moderation-record"><h2 id="moderation-record" ref={heading} tabIndex={-1}>Review {label(target)}</h2><dl><dt>Target reference</dt><dd>{rowId(target)}</dd>{'report_id' in target && <><dt>Reported target</dt><dd>{target.target_type} · {target.target_id}</dd></>}<dt>Current state</dt><dd>{target.state}</dd><dt>Revision</dt><dd>{target.revision}</dd></dl>
      {state.detail && <section aria-label="Private report evidence"><h3>Report evidence</h3><p>{state.detail.reason}</p><p className="moderation-text">{state.detail.text || 'No additional statement.'}</p><p>Reporter identity is not included in this review.</p>{state.detail.evidence_media_ids.map((id, i) => <button key={id} disabled={locked} onClick={() => void controller.evidence(id)}>Open private evidence {i + 1}</button>)}
        {state.evidenceUrl && <img className="moderation-evidence" src={state.evidenceUrl} alt="Private report evidence"/>}</section>}
      {!!actions(target).length && <form onSubmit={event => {
                event.preventDefault();
                controller.review();
            }}><fieldset disabled={locked || ['conflict', 'failed'].includes(state.phase)}><legend>Prepare an action</legend>
        <label htmlFor="moderation-action">Action</label><select id="moderation-action" value={state.action} onChange={e => controller.draft(e.target.value, state.reason)}><option value="">Choose an action</option>{actions(target).map(a => <option key={a} value={a}>{a}</option>)}</select>
        <label htmlFor="moderation-reason">Reason for this action</label><textarea id="moderation-reason" value={state.reason} maxLength={controller.lane === 'reports' ? 2000 : 500} onChange={e => controller.draft(state.action, e.target.value)} required rows={4}/>
        <p>Keep the reason factual. Do not add unrelated personal data. {controller.lane === 'reports' ? 'A report decision does not automatically hide content.' : controller.lane === 'members' ? 'Unsuspending a member never restores an earlier authority lease.' : 'Restoring content does not reverse other moderation actions.'}</p>
        <button ref={opener} type="submit" disabled={!state.action || !state.reason.trim()}>Review action</button></fieldset></form>}
    </section>}
    {state.confirm && target && <section role="dialog" aria-modal="false" aria-labelledby="moderation-confirm"><h2 id="moderation-confirm">Confirm {state.action.toLowerCase()}</h2>{state.action === 'SUSPEND' && <p>This event suspension continues until explicitly removed.</p>}<p>Target {rowId(target)}, revision {target.revision}</p><p className="moderation-text">{state.reason}</p><p>This sends one server command. Pending or uncertain results will require a status check.</p><button ref={confirm} disabled={locked} onClick={() => void controller.submit()}>Confirm action</button><button onClick={() => {
        controller.cancel();
        opener.current?.focus();
    }}>Cancel confirmation</button></section>}
    {['pending', 'unknown'].includes(state.phase) && <button onClick={() => void controller.checkStatus()}>Check original command</button>}
    {state.receipt && <p>Audit receipt reference: <code>{state.receipt}</code></p>}
  </main>;
}
