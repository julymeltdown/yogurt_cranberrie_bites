import { useState } from 'react'
import { Link } from 'react-router-dom'
import { Button, Feedback } from '../shared/ui'
import { AccessNotice, scopeKey, useAccess, usePorts, useScopedLoad } from './access'
import type { ActiveScope } from './ports'
import { DefaultAdminPanel } from './operations-composition/panel'
import { AdmissionBoundary } from '../features/auth/ui/AdmissionBoundary'

export default function AdminBoundary() {
  const { scope } = useAccess()
  return <AdmissionBoundary>{scope ? <AdminAccess key={scopeKey(scope)} scope={scope} /> : <AccessNotice />}</AdmissionBoundary>
}
function AdminAccess({ scope }: { scope: ActiveScope }) {
  const ports = usePorts()
  return ports.admin.open ? <DefaultAdminPanel scope={scope} /> : <LegacyAdminAccess scope={scope} />
}
// Compatible older injected shell ports never gain controls from a boolean.
function LegacyAdminAccess({ scope }: { scope: ActiveScope }) {
  const ports = usePorts()
  const [attempt, setAttempt] = useState(0)
  const access = useScopedLoad(scope, signal => ports.admin.authorize(scope, signal), attempt)
  const title = access.status === 'loading' ? 'Checking access'
    : access.status === 'error' ? 'Could not check access'
      : access.value ? 'Control room is not available yet' : 'Access denied'
  return <main id="main" tabIndex={-1} className="shell-gate">
    <Link className="shell-link" to="/">MCR Concert World</Link><h1>{title}</h1>
    <Feedback kind={access.status === 'error' || (access.status === 'ready' && !access.value) ? 'error' : 'status'} title="Event administration">
      <p>{access.status === 'ready' && access.value ? 'Administration tools are still being prepared. No changes have been made.'
        : 'A current authorization check is required before opening these tools.'}</p>
    </Feedback>
    <div className="shell-actions">
      {access.status === 'error' && <Button onClick={() => setAttempt(n => n + 1)}>Try again</Button>}
      <Link className="shell-link" to="/world">Back to world</Link><Link className="shell-link" to="/">Back to home</Link>
    </div>
  </main>
}
