import { useCallback, useSyncExternalStore } from 'react'
import { usePorts } from '../../../app/access'
import { ShellLink } from '../../../app/navigation'
import { ConcertIcon } from '../../../shared/ui/concert-window'
import { projectNearby } from '../../world-nearby/projection'

const empty = () => null
const noop = () => () => {}

/** Observe already authorized replication only; never fetch a second roster. */
export function WorldContextStrip({ expanded }: { expanded: boolean }) {
  const { worldAccess, auth } = usePorts()
  const binding = useSyncExternalStore(worldAccess?.subscribe ?? noop, worldAccess?.read ?? empty, empty)
  const read = useCallback(() => {
    if (!binding || binding.release.signal.aborted || !auth?.session.isCurrent(binding.release.scope)
      || !auth.privacy.isCurrent(binding.release.privacyScope)) return null
    return binding.replication.read()
  }, [binding, auth])
  const view = useSyncExternalStore(binding?.replication.subscribe ?? noop, read, empty)
  const people = view && binding ? projectNearby(view, binding.release.scope) : []
  const live = view?.phase === 'live' && view.visible && people.length > 0
  const status = live ? `${people.length} in view` : view?.phase === 'switching' ? 'Changing rooms…' : 'Presence unavailable'
  return <section id="pi08-world-context" className="pi08-world-context" aria-label="World context" data-expanded={expanded}>
    <div className="pi08-context-copy">
      <p><ConcertIcon name="map" /><strong>World</strong><span aria-hidden="true">/</span><span>{status}</span></p>
      {expanded && <p className="pi08-context-caption">Virtual presence, not physical arrival.</p>}
    </div>
    <ShellLink id="pi08-context-action" to={live ? '/world/nearby' : '/world/rooms'} className="mcr-button mcr-button--primary pi08-context-action">
      {live ? 'Nearby' : 'Rooms'}
    </ShellLink>
  </section>
}
