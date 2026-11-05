import { useEffect } from 'react'
import { useLocation, useNavigate } from 'react-router-dom'
import { BlocksPanel, ReportDialog } from '../../features/safety/panels'
import { useSafetySnapshot } from './context'
import '../../features/safety/parade-ink/report/report.css'

function SafetyPanel({ report }: { report: boolean }) {
  const { entry, state } = useSafetySnapshot(), location = useLocation(), navigate = useNavigate()
  useEffect(() => { if (entry) void entry.prepare() }, [entry])
  if (!entry || !state?.controller || !state.scope || !entry.auth.privacy.isCurrent(state.scope)) return <section aria-label="Safety controls">
    <h1>{report ? 'Report a concern' : 'Blocked users'}</h1><p role="status">{state?.message || 'Checking safety controls…'}</p>
    {entry && !state?.preparing && <button type="button" onClick={() => { void entry.retry() }}>Check safety availability</button>}
  </section>
  const back = () => { void navigate('/me') }
  if (!(report ? state.reports : state.blocks)) return <section aria-label="Safety controls">
    <h1>{report ? 'Report a concern' : 'Blocked users'}</h1><p role="status">This safety service is unavailable. No action has been sent.</p>
    <button type="button" onClick={() => { void entry.retry() }}>Check safety availability</button>
    <button type="button" onClick={back}>Back to Me</button>
  </section>
  if (report) return <>
    <p>Evidence attachments are unavailable in this connection. You can submit a text report when the target is authorized.</p>
    <ReportDialog controller={state.controller} search={location.search} onDismiss={() => { void navigate('/world') }} />
  </>
  return <BlocksPanel controller={state.controller} onBack={back} />
}
export function ReportPanel() { return <div className="pi-report-entry"><SafetyPanel report /></div> }
export function BlockedUsersPanel() { return <SafetyPanel report={false} /> }
