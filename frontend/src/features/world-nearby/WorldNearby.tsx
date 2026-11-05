import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react'
import { AvatarPopover } from '../avatar/parade-ink/AvatarPopover'
import type { NearbyPerson } from './projection'
import { ShellLink } from '../../app/navigation'
import { useWorldAccess } from '../world-rooms/ui/WorldRooms'
import type { WorldAccessController } from '../world-rooms/model/controller'
export function WorldNearby({ controller, renderAvatar }: {
  controller: WorldAccessController; renderAvatar?: (person: NearbyPerson) => ReactNode
}) {
  const state = useWorldAccess(controller), self = state.nearby.find(p => p.self), others = state.nearby.filter(p => !p.self)
  const peopleRegion = useRef<HTMLElement>(null), focusedPerson = useRef<string | null>(null)
  const [selection, setSelection] = useState<{ controller: WorldAccessController; userId: string; roomId: string | null } | null>(null)
  const origin = useRef<HTMLButtonElement | null>(null)
  const focusClose = useCallback((button: HTMLButtonElement | null) => { button?.focus() }, [])
  // Observe every invalidation, including reset/rebind in one React batch. Never
  // carry a selected identity across an authorization or visibility withdrawal.
  useEffect(() => {
    const withdraw = () => {
      const next = controller.read()
      setSelection(previous => previous && previous.controller === controller && next.available
        && ['live', 'switching'].includes(next.phase) && previous.roomId === next.currentRoomId
        && next.nearby.some(person => person.userId === previous.userId) ? previous : null)
    }
    const stop = controller.subscribe(withdraw)
    withdraw()
    return stop
  }, [controller])
  const dismiss = () => {
    setSelection(null)
    if (origin.current?.isConnected) origin.current.focus()
    else peopleRegion.current?.focus()
  }
  useEffect(() => {
    if (!focusedPerson.current || state.nearby.some(person => person.userId === focusedPerson.current)) return
    if (document.activeElement === document.body) peopleRegion.current?.focus()
    focusedPerson.current = null
  }, [state.nearby])
  const live = state.available && ['live', 'switching'].includes(state.phase)
  // Derive all displayed identity and portrait input from this render's current
  // authorized roster. The previous selection never stores private name/config.
  const selectedPerson = live && selection?.controller === controller && selection.roomId === state.currentRoomId
    ? state.nearby.find(person => person.userId === selection.userId) : null
  return <div className="world-access world-nearby pi-map-family"><nav className="world-access-nav" aria-label="Nearby navigation"><ShellLink id="nearby-rooms" to="/world/rooms">Choose a room</ShellLink><ShellLink id="nearby-world" to="/world">Back to world</ShellLink></nav>
    <h2>Nearby in world</h2><p>People currently visible to you in this virtual room. This list does not show physical location or distance.</p>
    {!state.available && <p role="status">Nearby people are unavailable until your current world access is verified.</p>}
    {state.available && !live && <p role="status">{state.phase === 'resync' ? 'Updating who is visible. The previous list has been cleared.' : state.phase === 'suspended' ? 'The world is paused. The previous list has been cleared.' : 'The world is not connected. The previous list has been cleared. Return to the world to check your connection.'}</p>}
    {live && <><section aria-label="You in this world"><h3>You</h3>{self ? <p><strong>{self.displayName}</strong> · You · {self.moving ? 'Moving in world' : 'Standing in world'}</p> : <p role="status">Waiting for your current world presence.</p>}</section>
      <section ref={peopleRegion} tabIndex={-1} className="world-nearby-people" aria-label="Other people in this world" onFocusCapture={event => { focusedPerson.current = event.target instanceof HTMLElement ? event.target.dataset.nearbyUserId ?? null : null }}><h3>Other people</h3><div aria-live="polite" aria-atomic="true" className="world-access-sr">{others.length ? 'The visible people list was updated.' : 'No other people are currently visible to you.'}</div>
        {others.length ? <ul className="world-access-list" aria-label="People authorized in your current virtual room">{others.map(person => <li key={person.userId} className="world-nearby-person"><button type="button" className="world-nearby-portrait" data-nearby-user-id={person.userId} aria-label={`View ${person.displayName} avatar`} aria-haspopup="dialog" onClick={event => {
          origin.current = event.currentTarget; setSelection({ controller, userId: person.userId, roomId: state.currentRoomId })
        }}>{renderAvatar?.(person) ?? <span>Portrait unavailable</span>}</button><div><h3><ShellLink data-nearby-user-id={person.userId} id={`nearby-person-${person.userId}`} to={`/people/${person.userId}`}>{person.displayName}</ShellLink></h3><p className="world-access-meta">{person.moving ? 'Moving in world' : 'Standing in world'}</p></div></li>)}</ul> : <p>No other people are currently visible to you.</p>}
      </section></>}
    {selectedPerson && <div className="world-nearby-popover"><AvatarPopover displayName={selectedPerson.displayName}
      portrait={renderAvatar?.(selectedPerson) ?? null} closeButtonRef={focusClose} onClose={dismiss}
      profileAction={<ShellLink id={`nearby-profile-${selectedPerson.userId}`} to={`/people/${selectedPerson.userId}`}>View profile</ShellLink>} /></div>}
    <p className="world-access-meta">Visibility can change when someone leaves, access changes or blocking applies. Hidden people are not listed.</p>
  </div>
}
