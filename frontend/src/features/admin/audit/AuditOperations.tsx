import { OperationsSurface, OperationsFrame } from '../parade-ink-operations/OperationsSurface'
import { useEffect, useRef, useState } from 'react'
import type { OperationsController } from '../event/model/controller'
import { Field, OperationFeedback, OperationsGate, useOperations } from '../event/ui/common'
import type { AuditQuery } from './contract'
export function AuditOperations({ controller }: { controller: OperationsController }) {
  const state = useOperations(controller), detail = useRef<HTMLHeadingElement>(null)
  const [query, setQuery] = useState({ actor: '', operation: '', outcome: '', from: '', until: '' })
  const filters = (): AuditQuery => ({ ...(query.actor ? { actor_id: query.actor } : {}), ...(query.operation ? { operation_id: query.operation } : {}),
    ...(query.outcome ? { outcome: query.outcome as AuditQuery['outcome'] } : {}), ...(query.from ? { from: query.from } : {}), ...(query.until ? { until: query.until } : {}) })
  useEffect(() => { void controller.loadAudit() }, [controller])
  useEffect(() => {
    if (!state.auditDetail) return
    const opener = document.activeElement; detail.current?.focus()
    return () => { if (opener instanceof HTMLElement && opener.isConnected) opener.focus() }
  }, [state.auditDetail])
  useEffect(() => { if (state.access !== 'ready') setQuery({ actor: '', operation: '', outcome: '', from: '', until: '' }) }, [state.access])
  return <OperationsSurface view="audit"><OperationsGate state={state}><OperationsFrame title="Audit log"><p>Minimal action metadata only. Report text, private media, notice contents and free-text reasons are not opened here.</p>
    <form onSubmit={e => { e.preventDefault(); void controller.loadAudit(filters()) }}><fieldset disabled={state.loading}><legend>Search action metadata</legend>
      <div className="ops-fields"><Field label="Actor UUID">{id => <input id={id} value={query.actor} onChange={e => setQuery({ ...query, actor: e.target.value })} />}</Field>
        <Field label="Operation ID">{id => <input id={id} maxLength={100} value={query.operation} onChange={e => setQuery({ ...query, operation: e.target.value })} />}</Field>
        <Field label="Outcome">{id => <select id={id} value={query.outcome} onChange={e => setQuery({ ...query, outcome: e.target.value })}><option value="">All</option><option>PENDING</option><option>SUCCEEDED</option><option>FAILED</option></select>}</Field>
        <Field label="From (inclusive UTC)">{id => <input id={id} placeholder="YYYY-MM-DDTHH:mm:ssZ" value={query.from} onChange={e => setQuery({ ...query, from: e.target.value })} />}</Field>
        <Field label="Until (exclusive UTC)">{id => <input id={id} placeholder="YYYY-MM-DDTHH:mm:ssZ" value={query.until} onChange={e => setQuery({ ...query, until: e.target.value })} />}</Field></div>
      <button type="submit">Search audit log</button></fieldset></form>
    <OperationFeedback state={state} controller={controller} />
    {state.audit.length === 0 && !state.loading && <p>No audit entries in this authorized page.</p>}
    <div className="ops-table-wrap" role="region" aria-label="Scrollable action outcomes" tabIndex={0}><table><caption>Action outcomes</caption><thead><tr><th scope="col">Operation</th><th scope="col">Outcome</th><th scope="col">Details</th></tr></thead>
      <tbody>{state.audit.map(row => <tr key={row.audit_id}><th scope="row">{row.operation_id}<br /><time dateTime={row.created_at}>{row.created_at}</time></th><td className="ops-state">{row.outcome}</td>
        <td><button type="button" onClick={() => controller.selectAudit(row.audit_id)}>View audit entry</button></td></tr>)}</tbody></table></div>
    {state.auditCursor && <button type="button" disabled={state.loading} onClick={() => { void controller.loadAudit(filters(), true) }}>Next audit page</button>}
    {state.auditDetail && <section><h2 ref={detail} tabIndex={-1}>Audit entry details</h2><dl className="ops-audit-detail">
      {Object.entries(state.auditDetail).map(([key, value]) => <div key={key}><dt>{key.replaceAll('_', ' ')}</dt><dd>{value ?? 'None'}</dd></div>)}
    </dl><p>This entry records an outcome. It is not proof of an externally delivered notification or a completed closure job.</p><button type="button" onClick={() => controller.selectAudit(null)}>Close audit detail</button></section>}
  </OperationsFrame></OperationsGate></OperationsSurface>
}
