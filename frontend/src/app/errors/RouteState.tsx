import { Link } from 'react-router-dom'
import { ConcertAction, ConcertWindow } from '../../shared/ui/concert-window'
import '../../features/connection-status/status.css'
import './parade-ink/status.css'

/** Root owner explicitly selects the safe category; no thrown value enters the DOM. */
export function RouteState({ kind, reload }: { kind: 'not-found' | 'unavailable' | 'maintenance' | 'version'; reload: () => void }) {
  const copy = {
    'not-found': ['Page not found', 'This page is unavailable or no longer exists.'],
    unavailable: ['This page is unavailable', 'Try reloading or return to another section.'],
    maintenance: ['Temporarily under maintenance', 'Please return later.'],
    version: ['Reload to continue', 'This page needs the current application version. Live actions are unavailable.'],
  }[kind]
  return <div className="connection-status pi-route-status"><ConcertWindow title={copy[0]!} headingLevel={1}>
    <p role="status">{copy[1]}</p><div className="connection-status__actions">
      {kind !== 'not-found' && <ConcertAction onClick={reload}>Reload page</ConcertAction>}
      <Link to="/world">Back to world</Link><Link to="/">Back to home</Link>
    </div>
  </ConcertWindow></div>
}
