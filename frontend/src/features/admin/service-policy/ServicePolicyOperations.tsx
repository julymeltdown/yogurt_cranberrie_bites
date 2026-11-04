import { Fragment, useEffect, useId, useRef, useSyncExternalStore } from 'react'
import type { ServicePolicyController } from './controller'

/** Child of the existing Admin frame; no second navigation, authority owner or stylesheet. */
export function ServicePolicyOperations({ controller }: { controller: ServicePolicyController }) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read)
  const id = useId(), heading = useRef<HTMLHeadingElement>(null), reviewButton = useRef<HTMLButtonElement>(null)
  const previousStage = useRef(state.command?.stage)
  useEffect(() => { void controller.refresh() }, [controller])
  useEffect(() => {
    if (state.command?.stage === 'review') heading.current?.focus()
    else if (previousStage.current === 'review' && !state.command) reviewButton.current?.focus()
    previousStage.current = state.command?.stage
  }, [state.command])
  if (state.access !== 'ready') return <section aria-label="Service policy unavailable"><h2>Service policy unavailable</h2>
    <p role="status">Current server-qualified operator access is required. Previous drafts and command information are hidden.</p></section>
  const decision = state.decision, draft = state.draft, command = state.command
  const locked = state.loading || command !== null
  const fields = decision ? [['Event revision', decision.event_revision], ['Policy revision', decision.policy_revision ?? 'No publication history'],
    ['Current decision', decision.kind], ['Minimum client version', decision.minimum_client_version ?? 'Unavailable'],
    ['Server time', decision.server_time], ['Decision valid until', decision.valid_until]] : []
  return <section className="event-ops" aria-labelledby={id + '-title'}>
    <h2 id={id + '-title'}>Service policy</h2>
    <p>Publish or withdraw the service policy for this event. The server enforces current access; a successful command does not itself grant access.</p>
    <p>Event: {state.eventId}</p>
    <button type="button" disabled={state.loading || command?.stage === 'review' || command?.stage === 'sending'} onClick={() => { void controller.refresh() }}>Refresh service policy</button>
    {state.loading && <p role="status">Reading authorized policy information…</p>}
    {state.message && <p role="status">{state.message}</p>}
    {decision ? <dl>{fields.map(([label, value]) => <Fragment key={label}><dt>{label}</dt><dd>{value}</dd></Fragment>)}</dl>
      : <p role="status">A fresh policy decision is unavailable. Refresh before reviewing another change.</p>}
    <p>Policy source and review metadata are unavailable in this response. Publication history, original expiry and reviewer details are not supplied.</p>
    <p>Your draft stays separate from refreshed information. Compare the current revision before reviewing a change.</p>
    <form onSubmit={event => { event.preventDefault(); controller.review() }}>
      <fieldset disabled={locked}><legend>Policy change draft</legend>
        <label htmlFor={id + '-action'}>Action</label><select id={id + '-action'} value={draft.action} onChange={event => controller.edit({ action: event.target.value as typeof draft.action })}>
          <option value="PUBLISH">Publish policy</option><option value="WITHDRAW">Withdraw policy</option></select>
        {draft.action === 'PUBLISH' && <>
          <label htmlFor={id + '-mode'}>Service mode</label><select id={id + '-mode'} value={draft.mode} onChange={event => controller.edit({ mode: event.target.value as typeof draft.mode })}>
            <option value="NORMAL">Normal</option><option value="MAINTENANCE">Maintenance</option></select>
          <label htmlFor={id + '-version'}>Minimum client version</label><input id={id + '-version'} required maxLength={17} placeholder="1.0.0" value={draft.minimumClientVersion} onChange={event => controller.edit({ minimumClientVersion: event.target.value })} />
          <label htmlFor={id + '-expiry'}>Publication expiry (UTC)</label><input id={id + '-expiry'} required placeholder="YYYY-MM-DDTHH:mm:ssZ" value={draft.expiresAt} onChange={event => controller.edit({ expiresAt: event.target.value })} />
          <p>Expiry must be in the future and within 24 hours of current server time.</p>
        </>}
        <label htmlFor={id + '-reason'}>Reason</label><textarea id={id + '-reason'} required maxLength={500} value={draft.reason} onChange={event => controller.edit({ reason: event.target.value })} />
        <p>Use a trimmed reason of at most 500 UTF-8 bytes.</p>
        <button ref={reviewButton} type="submit" disabled={!decision}>Review policy change</button>
      </fieldset>
    </form>
    {command && <section aria-labelledby={id + '-review'}>
      <h3 id={id + '-review'} ref={heading} tabIndex={-1}>{command.stage === 'review' ? 'Confirm service policy change' : 'Policy command outcome'}</h3>
      <dl><dt>Event</dt><dd>{state.eventId}</dd><dt>Expected policy revision</dt><dd>{command.input.expected_policy_revision}</dd>
        <dt>Action</dt><dd>{command.input.change.action}</dd>
        {command.input.change.action === 'PUBLISH' && <><dt>Service mode</dt><dd>{command.input.change.mode}</dd><dt>Minimum client version</dt><dd>{command.input.change.minimum_client_version}</dd>
          <dt>Publication expiry</dt><dd>{command.input.change.expires_at}</dd></>}
        <dt>Reason</dt><dd>{command.input.reason}</dd><dt>Command state</dt><dd>{command.stage}</dd>
        {command.receipt && <><dt>Receipt</dt><dd>{command.receipt.receipt_id}</dd><dt>Receipt state</dt><dd>{command.receipt.state}</dd>
          <dt>Result event revision</dt><dd>{command.receipt.resource_revision ?? 'Not supplied'}</dd><dt>Failure reason</dt><dd>{command.receipt.failure_reason ?? 'None supplied'}</dd></>}
      </dl><p role="status">{command.message}</p>
      {command.stage === 'review' && <><p>{command.input.change.action === 'WITHDRAW' ? 'Withdrawal makes policy unavailable and never revives an older publication.' : 'This change affects service availability and supported client versions.'}</p>
        <button type="button" onClick={() => { void controller.confirm() }}>Confirm policy change</button><button type="button" onClick={controller.cancel}>Cancel confirmation</button></>}
      {['pending', 'unknown'].includes(command.stage) && <button type="button" disabled={state.loading} onClick={() => { void controller.checkStatus() }}>Check policy command status</button>}
      {['succeeded', 'failed', 'conflict'].includes(command.stage) && <button type="button" disabled={state.loading || !decision} onClick={controller.dismiss}>Return to retained draft</button>}
    </section>}
  </section>
}
