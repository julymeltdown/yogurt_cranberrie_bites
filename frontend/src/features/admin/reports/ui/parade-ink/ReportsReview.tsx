import type { RefObject } from 'react'
import { Button } from '../../../../../shared/ui/controls'
import { Feedback } from '../../../../../shared/ui/feedback'
import { actions, rowKey } from '../../model/contract'
import type { ModerationController, ModerationState } from '../../model/controller'
import './reports.css'

const phaseTitle: Record<ModerationState['phase'], string> = {
  idle: 'Ready to read', loading: 'Reading records', ready: 'Current review',
  failed: 'Request failed', conflict: 'Record changed', sending: 'Sending action',
  pending: 'Action pending', unknown: 'Outcome unknown', checking: 'Checking original command',
  applied: 'Server confirmed', denied: 'Access unavailable',
}
const actionName = (action: string) => action === 'RESOLVED' ? 'Resolve report' : 'Dismiss report'
// Keep the shared owner's later Member/Content branches outside TS narrowing.
export const isReportsLane = (lane: ModerationController['lane']): boolean => lane === 'reports'

/** Presentation only. The incumbent owner retains loading, focus and every command. */
export function ReportsReview({ controller, state, heading, confirm, opener }: {
  controller: ModerationController
  state: ModerationState
  heading: RefObject<HTMLHeadingElement | null>
  confirm: RefObject<HTMLButtonElement | null>
  opener: RefObject<HTMLButtonElement | null>
}) {
  const locked = ['loading', 'sending', 'pending', 'unknown', 'checking', 'applied'].includes(state.phase)
  const commandLocked = ['sending', 'pending', 'unknown', 'checking'].includes(state.phase)
  const target = state.selected && 'report_id' in state.selected ? state.selected : null
  const uncertain = state.phase === 'pending' || state.phase === 'unknown'
  const error = ['failed', 'conflict', 'unknown'].includes(state.phase)
  return <main className="reports-ink" aria-label="Report review">
    <header className="reports-ink__title"><h1>Report queue</h1><span>MODERATION</span></header>
    <div className="reports-ink__body">
      {!state.access ? <Feedback kind="error" title="Moderator access unavailable">
        A current server authorization for this event is required. No private records are open.
      </Feedback> : <>
        <div className="reports-ink__intro"><p>Private review<br/><span>Use only the information needed for this action.</span></p>
          <Button variant="secondary" disabled={commandLocked} onClick={() => void controller.load()}>Refresh records</Button>
        </div>
        {(state.message || state.phase === 'loading' || state.phase === 'checking') && <Feedback
          kind={error ? 'error' : 'status'} title={phaseTitle[state.phase]}
          action={uncertain ? <Button variant="secondary" onClick={() => void controller.checkStatus()}>Check original command</Button> : undefined}>
          {state.message || 'Reading current review records…'}
        </Feedback>}
        <section className="reports-ink__queue" aria-labelledby="reports-records">
          <div className="reports-ink__section-heading"><h2 id="reports-records">Records on this page</h2><span>{state.rows.length}</span></div>
          {!!state.rows.length && <ul className="reports-ink__records">{state.rows.map(report => 'report_id' in report && <li
            key={rowKey(report)} className={target?.report_id === report.report_id ? 'is-current' : undefined}>
            <div><strong>{report.target_type} report</strong><small>Ref {report.report_id.slice(-8)} · v{report.revision}</small></div>
            <span className="reports-ink__state">{report.state.replaceAll('_', ' ')}</span>
            <Button variant="secondary" disabled={locked} aria-pressed={target?.report_id === report.report_id}
              aria-label={`Review ${report.report_id}`} onClick={() => void controller.select(rowKey(report))}>Review</Button>
          </li>)}</ul>}
          {state.phase === 'ready' && !state.rows.length && <Feedback kind="empty" title="No reports on this page">There are no records in the current server response.</Feedback>}
          {state.cursor && <Button variant="secondary" disabled={locked} onClick={() => void controller.load(true)}>Next page</Button>}
        </section>
        {target && <section className="reports-ink__record" aria-labelledby="moderation-record">
          <h2 id="moderation-record" className="reports-ink__bar" ref={heading} tabIndex={-1}>Review {target.target_type} report</h2>
          <dl className="reports-ink__facts"><dt>Report reference</dt><dd>{target.report_id}</dd>
            <dt>Reported target</dt><dd>{target.target_type} · {target.target_id}</dd>
            <dt>Current state</dt><dd>{target.state.replaceAll('_', ' ')}</dd><dt>Revision</dt><dd>{target.revision}</dd>
          </dl>
          {state.detail && <section className="reports-ink__evidence" aria-label="Private report evidence">
            <h3>Report evidence</h3><p className="reports-ink__reason">{state.detail.reason.replaceAll('_', ' ')}</p>
            <p className="reports-ink__statement">{state.detail.text || 'No additional statement.'}</p>
            <p className="reports-ink__hint">Reporter identity is not included in this review.</p>
            <div className="reports-ink__evidence-actions">{state.detail.evidence_media_ids.map((id, i) => <Button key={id} variant="secondary"
              disabled={locked} onClick={() => void controller.evidence(id)}>Open private evidence {i + 1}</Button>)}</div>
            {state.evidenceUrl && <img className="reports-ink__image" src={state.evidenceUrl} alt="Private report evidence"/>}
          </section>}
          {!!actions(target).length && <form onSubmit={event => { event.preventDefault(); controller.review() }}>
            <fieldset className="reports-ink__form" disabled={locked || ['conflict', 'failed'].includes(state.phase)}>
              <legend>Prepare an action</legend>
              <label htmlFor="moderation-action">Action</label><select className="mcr-input" id="moderation-action" value={state.action}
                onChange={event => controller.draft(event.target.value, state.reason)}>
                <option value="">Choose an action</option>{actions(target).map(action => <option key={action} value={action}>{actionName(action)}</option>)}
              </select>
              <label htmlFor="moderation-reason">Reason for this action</label><textarea className="mcr-input" id="moderation-reason"
                value={state.reason} maxLength={2000} required rows={4} aria-describedby="reports-reason-hint"
                onChange={event => controller.draft(state.action, event.target.value)}/>
              <p id="reports-reason-hint" className="reports-ink__hint">Keep the reason factual. Do not add unrelated personal data. A report decision does not automatically hide content.</p>
              <Button ref={opener} type="submit" disabled={!state.action || !state.reason.trim()}>Review action</Button>
            </fieldset>
          </form>}
        </section>}
        {state.confirm && target && <section className="reports-ink__confirmation" role="dialog" aria-modal="false" aria-labelledby="moderation-confirm">
          <h2 id="moderation-confirm" className="reports-ink__bar">Confirm {state.action.toLowerCase()}</h2>
          <div><p>Report {target.report_id} · revision {target.revision}</p><p className="reports-ink__statement">{state.reason}</p>
            <p className="reports-ink__hint">This sends one server command. Pending or uncertain results will require a status check.</p>
            <div className="reports-ink__buttons"><Button ref={confirm} disabled={locked} onClick={() => void controller.submit()}>Confirm action</Button>
              <Button variant="secondary" onClick={() => { controller.cancel(); opener.current?.focus() }}>Cancel confirmation</Button></div>
          </div>
        </section>}
        {state.receipt && <p className="reports-ink__receipt">Audit receipt reference<br/><code>{state.receipt}</code></p>}
      </>}
    </div>
  </main>
}
