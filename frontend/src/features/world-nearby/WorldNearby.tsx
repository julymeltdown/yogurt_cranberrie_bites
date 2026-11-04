import { useEffect, useRef } from 'react'
import { ShellLink } from '../../app/navigation'
import { useWorldAccess } from '../world-rooms/ui/WorldRooms'
import type { WorldAccessController } from '../world-rooms/model/controller'
export function WorldNearby({ controller }: { controller: WorldAccessController }) {
  const state = useWorldAccess(controller), self = state.nearby.find(p => p.self), others = state.nearby.filter(p => !p.self)
  const peopleRegion = useRef<HTMLElement>(null), focusedPerson = useRef<string | null>(null)
  useEffect(() => {
    if (!focusedPerson.current || state.nearby.some(person => person.userId === focusedPerson.current)) return
    if (document.activeElement === document.body) peopleRegion.current?.focus()
    focusedPerson.current = null
  }, [state.nearby])
  const live = state.available && ['live', 'switching'].includes(state.phase)
  return <div className="world-access"><nav className="world-access-nav" aria-label="Nearby navigation"><ShellLink id="nearby-rooms" to="/world/rooms">Choose a room</ShellLink><ShellLink id="nearby-world" to="/world">Back to world</ShellLink></nav>
    <h2>Nearby in world</h2><p>People currently visible to you in this virtual room. This list does not show physical location or distance.</p>
    {!state.available && <p role="status">Nearby people are unavailable until your current world access is verified.</p>}
    {state.available && !live && <p role="status">{state.phase === 'resync' ? 'Updating who is visible. The previous list has been cleared.' : state.phase === 'suspended' ? 'The world is paused. The previous list has been cleared.' : 'The world is not connected. The previous list has been cleared. Return to the world to check your connection.'}</p>}
    {live && <><section aria-label="You in this world"><h3>You</h3>{self ? <p><strong>{self.displayName}</strong> · You · {self.moving ? 'Moving in world' : 'Standing in world'}</p> : <p role="status">Waiting for your current world presence.</p>}</section>
      <section ref={peopleRegion} tabIndex={-1} className="world-nearby-people" aria-label="Other people in this world" onFocusCapture={event => { focusedPerson.current = event.target instanceof HTMLAnchorElement ? event.target.dataset.nearbyUserId ?? null : null }}><h3>Other people</h3><div aria-live="polite" aria-atomic="true" className="world-access-sr">{others.length ? 'The visible people list was updated.' : 'No other people are currently visible to you.'}</div>
        {others.length ? <ul className="world-access-list" aria-label="People authorized in your current virtual room">{others.map(person => <li key={person.userId}><div><h3><ShellLink data-nearby-user-id={person.userId} id={`nearby-person-${person.userId}`} to={`/people/${person.userId}`}>{person.displayName}</ShellLink></h3><p className="world-access-meta">{person.moving ? 'Moving in world' : 'Standing in world'}</p></div></li>)}</ul> : <p>No other people are currently visible to you.</p>}
      </section></>}
    <p className="world-access-meta">Visibility can change when someone leaves, access changes or blocking applies. Hidden people are not listed.</p>
  </div>
}
