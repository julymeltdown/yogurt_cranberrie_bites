import { useSyncExternalStore } from 'react'
import { Link } from 'react-router-dom'
import { ConcertAction, ConcertWindow } from '../../shared/ui/concert-window'
import { connectionCopy, type ConnectionStatusController, type ConnectionStatusView } from './model'
import './status.css'
export type { ConnectionKind, ConnectionStatusController, ConnectionStatusView } from './model'

/** In-flow notice. The shell keeps ownership of input suspension and recovery. */
export function ConnectionStatus({ controller, reload }: { controller: ConnectionStatusController; reload: () => void }) {
  const view = useSyncExternalStore(controller.subscribe, controller.read, controller.read)
  return <ConnectionStatusNotice view={view} retry={() => { void controller.retry() }} reload={reload} />
}
export function ConnectionStatusNotice({ view, retry, reload }: {
  view: ConnectionStatusView; retry: () => void; reload: () => void
}) {
  if (view.kind === 'ready') return null
  const copy = connectionCopy[view.kind]
  return <div className="connection-status">
    <ConcertWindow title={copy.title} icon="world">
      <div role="status" aria-live="polite" aria-atomic="true">
        <p>{copy.description}</p>
        {view.stale && <p className="connection-status__stale">Previously shown world information may be out of date.</p>}
        {view.pending && <p>Waiting for the world…</p>}
      </div>
      <div className="connection-status__actions">
        {view.canRetry && <ConcertAction onClick={retry}>Reconnect</ConcertAction>}
        {!view.pending && view.kind !== 'suspended' && <ConcertAction tone={view.canRetry ? 'paper' : 'red'} onClick={reload}>Reload page</ConcertAction>}
        <Link to="/world">Back to world</Link><Link to="/">Back to home</Link>
      </div>
    </ConcertWindow>
  </div>
}
