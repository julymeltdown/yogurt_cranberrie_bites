import { useEffect, useState, useSyncExternalStore } from 'react'
import { Link, useLocation } from 'react-router-dom'
import { EventOperations } from '../../features/admin/event'
import { PlacesOperations } from '../../features/admin/places'
import { AuditOperations } from '../../features/admin/audit'
import { MuralClosurePanel } from '../admin-entry/mural-closure/MuralClosurePanel'
import { usePorts } from '../access'
import type { ActiveScope, ShellPorts } from '../ports'
import type { AdminComposition, AdminCompositionState } from './owner'

const initial: AdminCompositionState = Object.freeze({ phase: 'idle', decision: null, settings: null, operations: null, muralClosure: null })
const noSubscription = () => () => {}
const initialRead = () => initial
const navigation = [['/admin', 'Control room'], ['/admin/event', 'Event'], ['/admin/places', 'Places'],
  ['/admin/mural', 'Wall'], ['/admin/archive', 'Closure'], ['/admin/audit', 'Audit'], ['/admin/reports', 'Reports']] as const

/** The production port owns the entry; route unmount never discards a command. */
export function DefaultAdminPanel({ scope }: { scope: ActiveScope }) {
  const ports = usePorts(), { pathname } = useLocation()
  const [slot, setSlot] = useState<{ source: ShellPorts['admin']; entry: AdminComposition | null } | null>(null)
  const entry = slot?.source === ports.admin ? slot.entry : null
  const state = useSyncExternalStore(entry?.subscribe ?? noSubscription, entry?.read ?? initialRead, initialRead)
  useEffect(() => {
    const opened = ports.admin.open?.(scope) ?? null
    setSlot({ source: ports.admin, entry: opened }); void opened?.start()
    // The shared production owner survives all sibling Admin route elements.
  }, [ports, scope.generation, scope.userId, scope.eventId])
  function refresh() {
    const opened = ports.admin.open?.(scope) ?? null
    setSlot({ source: ports.admin, entry: opened })
    if (opened === entry) void opened?.refresh()
    else void opened?.start()
  }
  const pending = state.phase === 'idle' || state.phase === 'checking'
  const ready = state.phase === 'ready' && state.operations && state.muralClosure
  const unsupported = ['/admin/reports', '/admin/content'].includes(pathname) || pathname.startsWith('/admin/users/')
  return <main id="main" tabIndex={-1} className="shell-gate">
    <Link className="shell-link" to="/">MCR Concert World</Link>
    <h1>Event administration</h1>
    <nav className="shell-actions" aria-label="Administration sections">{navigation.map(([path, label]) => <Link key={path}
      className="shell-link" to={path} aria-current={pathname === path ? 'page' : undefined}>{label}</Link>)}</nav>
    {ready ? <>
      <p>Current event: {state.settings?.event.title} · {state.settings?.event.phase} · revision {state.settings?.event.revision}.</p>
      {pathname === '/admin' || pathname === '/admin/event' ? <EventOperations controller={state.operations!} summaryOnly={pathname === '/admin'} />
        : pathname === '/admin/places' ? <PlacesOperations controller={state.operations!} />
          : pathname === '/admin/audit' ? <AuditOperations controller={state.operations!} />
            : pathname === '/admin/mural' || pathname === '/admin/archive' ? <MuralClosurePanel entry={state.muralClosure!} view={pathname === '/admin/mural' ? 'mural' : 'closure'} />
              : <UnavailableReports />}
    </> : <section aria-label="Administration access"><h2>{pending ? 'Checking current access' : unsupported ? 'Review tools unavailable' : 'Administration unavailable'}</h2>
      <p role="status">{pending ? 'Checking current operator access and authorized event information.'
        : state.decision?.role === 'MODERATOR' ? 'Moderator access does not grant event operator controls. Report and evidence review is currently unavailable.'
          : 'Current operator access and authorized event information are required. Previous controls and private progress are hidden.'}</p></section>}
    <div className="shell-actions"><button type="button" disabled={pending} onClick={refresh}>Check current access</button>
      <Link className="shell-link" to="/world">Back to world</Link><Link className="shell-link" to="/">Back to home</Link></div>
  </main>
}
function UnavailableReports() {
  return <section aria-label="Review tools unavailable"><h2>Review tools unavailable</h2>
    <p role="status">Report evidence, member review and moderation are currently unavailable. Event operator access does not grant these tools.</p></section>
}
