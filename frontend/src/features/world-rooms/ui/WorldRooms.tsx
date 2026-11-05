import { useEffect, useRef, useSyncExternalStore } from 'react'
import { ShellLink } from '../../../app/navigation'
import { roomErrorText, type WorldAccessController } from '../model/controller'
import './world-access.css'
export function useWorldAccess(controller: WorldAccessController) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read)
  useEffect(() => { controller.start() }, [controller]); return state
}
export function WorldRooms({ controller }: { controller: WorldAccessController }) {
  const state = useWorldAccess(controller), catalog = state.rooms.data
  const switchOrigin = useRef<HTMLButtonElement | null>(null), feedback = useRef<HTMLElement>(null)
  useEffect(() => {
    if (!['admitted', 'rejected', 'interrupted'].includes(state.switch.phase) || !switchOrigin.current) return
    // Admission replaces the originating button. Restore a useful keyboard position
    // without stealing focus from a user who moved to another control.
    if (document.activeElement === switchOrigin.current || document.activeElement === document.body) feedback.current?.focus()
    switchOrigin.current = null
  }, [state.switch.phase])
  useEffect(() => { void controller.loadRooms() }, [controller])
  const current = catalog?.items.find(r => r.room_id === state.currentRoomId)
  const target = catalog?.items.find(r => r.room_id === state.switch.targetId)
  const live = state.phase === 'live' || state.phase === 'switching'
  return <div className="world-access world-rooms pi-map-family"><nav className="world-access-nav" aria-label="Room navigation"><ShellLink id="rooms-nearby" to="/world/nearby">Nearby in world</ShellLink><ShellLink id="rooms-world" to="/world">Back to world</ShellLink></nav>
    <div className="world-access-heading"><h2>Choose your room</h2><button type="button" disabled={state.rooms.phase === 'loading' || !state.available} onClick={() => void controller.loadRooms()}>Refresh rooms</button></div>
    <p>Rooms are shared virtual spaces. Switching does not confirm physical arrival.</p>
    {state.currentRoomId && <p>{live ? 'Current room' : 'Last confirmed room'}: <strong>{current?.label ?? 'Room details unavailable'}</strong></p>}
    {!state.available && <p role="status">Room access is unavailable until your current world access is verified.</p>}
    {state.available && !live && <p role="status">{state.phase === 'resync' ? 'Updating the world. Room switching is paused.' : state.phase === 'suspended' ? 'The world is paused while this page is hidden.' : 'Waiting for a live world connection. Room switching is paused.'}</p>}
    {state.switch.phase !== 'idle' && <section ref={feedback} tabIndex={-1} className="world-access-status" aria-label="Room switch status" aria-live="polite">
      {state.switch.phase === 'switching' && <p role="status">Switching to {target?.label ?? 'the selected room'}… Your current room remains in place until the server confirms admission.</p>}
      {state.switch.phase === 'admitted' && <><p role="status">Room switch confirmed.</p><p>{target ? `You are now in ${target.label}.` : 'Your current membership has been updated.'}</p></>}
      {state.switch.phase === 'rejected' && <p role="alert">The switch was not accepted. Your confirmed room remains unchanged. Refresh room information before trying again.</p>}
      {state.switch.phase === 'interrupted' && <p role="status">World access changed during the switch. Check the current connection before switching again.</p>}
    </section>}
    {state.rooms.phase === 'loading' && <p role="status">Reading rooms…</p>}
    {state.rooms.error && <div role="alert"><p>{roomErrorText[state.rooms.error]}</p><button type="button" onClick={() => void controller.loadRooms()}>Read rooms again</button></div>}
    {catalog && <><ul className="world-access-list" aria-label="Available virtual rooms">{catalog.items.map(room => {
      const reason = controller.switchReason(room.room_id), isCurrent = state.currentRoomId === room.room_id
      return <li key={room.room_id} aria-current={isCurrent ? 'true' : undefined}><div><h3>{room.label}</h3><p className="world-access-meta">Capacity {room.capacity} · {room.availability === 'AVAILABLE' ? 'Available' : room.availability === 'FULL' ? 'Full' : 'Unavailable'}</p>
        {isCurrent ? <p>{live ? 'You are in this room.' : 'Your last confirmed room.'}</p> : <><button type="button" disabled={!!reason} aria-describedby={reason ? `room-reason-${room.room_id}` : undefined} onClick={event => { switchOrigin.current = event.currentTarget; void controller.switchRoom(room.room_id) }}>Switch to {room.label}</button>
          {reason && <p className="world-access-meta" id={`room-reason-${room.room_id}`}>{reason}</p>}</>}</div></li>
    })}</ul>{!catalog.items.some(r => r.availability === 'AVAILABLE' && r.room_id !== state.currentRoomId) && <p>No other room is available to join right now. Your current room is unaffected.</p>}</>}
    {state.partyNavigation && <section className="world-access-footer"><h3>Make a plan</h3><p>Party plans are separate from virtual rooms.</p><ShellLink id="rooms-parties" to="/meet">Find a party</ShellLink></section>}
  </div>
}
