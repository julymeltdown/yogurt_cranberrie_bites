import { createContext, useContext, useEffect, useState, useSyncExternalStore, type ReactNode } from 'react'
import { Link, useLocation } from 'react-router-dom'
import { Button, Feedback } from '../shared/ui'
import type { SessionScope } from '../shared/api/http'
import { canonicalId, safeReturnTarget } from './routes'
import type { ActiveScope, ShellBootstrap, ShellPorts } from './ports'
import { ShellLoadError, type ShellFailureReason } from './ports'
import { validCapabilities } from './capabilities'

const PortsContext = createContext<ShellPorts | null>(null)
export function PortsProvider({ ports, children }: { ports: ShellPorts; children: ReactNode }) {
  return <PortsContext value={ports}>{children}</PortsContext>
}
export function usePorts() {
  const ports = useContext(PortsContext)
  if (!ports) throw new Error('Shell ports are required')
  return ports
}
export function scopeKey(scope: SessionScope) { return `${scope.generation}:${scope.userId}:${scope.eventId}` }

export function useAccess() {
  const ports = usePorts()
  const readiness = useSyncExternalStore(ports.readiness.subscribe, ports.readiness.read)
  // FE01 publishes a new generation before abort; reattach on every replacement.
  const session = useSyncExternalStore(listener => {
    let signal = ports.session.read().signal
    const change = () => {
      signal.removeEventListener('abort', change)
      signal = ports.session.read().signal
      signal.addEventListener('abort', change, { once: true })
      listener()
    }
    signal.addEventListener('abort', change, { once: true })
    return () => signal.removeEventListener('abort', change)
  }, () => ports.session.read())
  const scope: ActiveScope | null = readiness === 'ready' && session.accessToken && session.userId && session.eventId
    && canonicalId(session.userId) && canonicalId(session.eventId) && !session.signal.aborted
    ? { generation: session.generation, userId: session.userId, eventId: session.eventId } : null
  return { readiness, scope, session }
}

export function AccessNotice() {
  const ports = usePorts()
  const { readiness } = useAccess()
  const location = useLocation()
  const target = safeReturnTarget(location.pathname + location.search)
  const pending = readiness === 'checking' || readiness === 'unknown'
  const title = pending ? 'Checking your session' : readiness === 'error' ? 'Could not check your session'
    : readiness === 'unavailable' ? 'World entry is not open yet' : 'Sign in to continue'
  return <div className="shell-gate"><Link className="shell-link" to="/">MCR Concert World</Link>
    <h1 tabIndex={-1}>{title}</h1>
    <Feedback kind={readiness === 'error' ? 'error' : 'status'} title={pending ? 'Your destination is saved' : 'Your link is still here'}>
      <p>{pending ? 'You can leave this page while we check access.'
        : readiness === 'unavailable' ? 'Sign-in and community features are still being prepared.'
          : 'Entry requires a current account and event. This page does not grant access.'}</p>
    </Feedback>
    <div className="shell-actions">
      {ports.readiness.retry && <Button variant="secondary" onClick={() => ports.readiness.retry?.()}>Check again</Button>}
      {!pending && readiness !== 'unavailable' && <Link className="shell-link" to={`/login?returnTo=${encodeURIComponent(target)}`}>Sign in</Link>}
      <Link className="shell-link" to="/">Back to home</Link>
    </div>
  </div>
}

export type LoadState<T> = { status: 'loading' } | { status: 'ready'; value: T } | { status: 'error'; reason: ShellFailureReason }
/** Bounded async seam; late ignored-abort completions never commit across identity/unmount.
 * The key includes FE01 generation; refreshed bearer versions do not rebuild the shell.
 */
export function useScopedLoad<T>(scope: ActiveScope, load: (signal: AbortSignal) => Promise<T>,
  attempt: number | string, timeoutMs = 12_000): LoadState<T> {
  const ports = usePorts()
  const key = scopeKey(scope)
  const [state, setState] = useState<{ source: ShellPorts; key: string; attempt: number | string; result: LoadState<T> }>(
    { source: ports, key, attempt, result: { status: 'loading' } })
  useEffect(() => {
    const controller = new AbortController()
    const signal = ports.session.read().signal
    // The rendered scope can already be obsolete before passive setup runs.
    // Do not subscribe to the replacement identity or allocate a timer for it.
    if (signal.aborted || !ports.session.isCurrent(scope)) { controller.abort(); return }
    const abort = () => controller.abort()
    signal.addEventListener('abort', abort, { once: true })
    const timer = setTimeout(() => {
      if (!controller.signal.aborted && ports.session.isCurrent(scope)) {
        setState({ source: ports, key, attempt, result: { status: 'error', reason: 'unavailable' } }); controller.abort()
      }
    }, timeoutMs)
    Promise.resolve().then(() => {
      // Fence dispatch as well as commit: a replacement can occur after setup
      // but before this microtask. Never pair an old scope with new credentials.
      if (controller.signal.aborted || signal.aborted || !ports.session.isCurrent(scope)) {
        controller.abort()
        throw new Error('Aborted')
      }
      return load(controller.signal)
    }).then(value => {
      if (!controller.signal.aborted && ports.session.isCurrent(scope)) setState({ source: ports, key, attempt, result: { status: 'ready', value } })
    }, error => {
      if (!controller.signal.aborted && ports.session.isCurrent(scope)) setState({ source: ports, key, attempt,
        result: { status: 'error', reason: error instanceof ShellLoadError ? error.reason : 'unavailable' } })
    }).finally(() => clearTimeout(timer))
    return () => { clearTimeout(timer); signal.removeEventListener('abort', abort); controller.abort() }
    // load is an injected stable port; location and short-lived scope objects must not restart it.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ports, key, attempt, timeoutMs])
  return state.source === ports && state.key === key && state.attempt === attempt ? state.result : { status: 'loading' }
}

export function validBootstrap(value: ShellBootstrap, scope: ActiveScope) {
  return value.contractVersion === '0.1.0' && value.event.eventId === scope.eventId
    && value.participant.eventId === scope.eventId && value.participant.userId === scope.userId
    && value.event.title.length > 0 && value.event.title.length <= 120
    && typeof value.worldAvailable === 'boolean'
    && validCapabilities(value.capabilities) && value.worldAvailable === value.capabilities.world.available
    && (!['READ_ONLY', 'ARCHIVED'].includes(value.event.phase) || !value.capabilities.mutations.available)
    && ['DRAFT', 'OPEN', 'LIVE', 'READ_ONLY', 'ARCHIVED'].includes(value.event.phase)
}
