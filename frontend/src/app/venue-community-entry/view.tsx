import { useSyncExternalStore } from 'react'
import { Invitation } from '../../features/venue-invitations/Invitation'
import { MeetingConversation } from '../../features/meeting-conversation/Conversation'
import type { MeetingLeafOwner } from './owner'

const noticeText = {
  unavailable: 'Current meeting information could not be read. Refresh before taking another action.',
  changed: 'Meeting access changed. Open this place again from the map.',
  'invalid-message': 'Check the message requirements before sending.',
  rejected: 'The action was not applied. Current permissions and invitation details have been refreshed.',
  unknown: 'The result is uncertain. Check the original action before starting another one.',
} as const
export function MeetingLeaf({ owner, onClose }: { readonly owner: MeetingLeafOwner; readonly onClose?: () => void }) {
  const state = useSyncExternalStore(owner.subscribe, owner.read, owner.read)
  if (state.phase === 'withdrawn' || state.phase === 'idle') return <MeetingUnavailable />
  const command = state.command
  return <div className="venue-meeting">
    <header className="venue-meeting-header"><strong>{state.context?.landmarkLabel ? 'Real meeting place' : 'Event conversation'}</strong>
      <button type="button" disabled={command?.phase === 'pending' || command?.phase === 'unknown' || command?.phase === 'checking'} onClick={() => { if (owner.close()) onClose?.() }}>Close</button></header>
    {state.phase === 'loading' && <p role="status">Reading current meeting…</p>}
    {state.phase === 'error' && <div><p role="status">Meeting information is unavailable.</p>
      <button type="button" onClick={() => { void owner.refresh() }}>Refresh meeting</button></div>}
    {state.phase === 'ready' && (state.surface === 'conversation'
      ? <MeetingConversation owner={owner} state={state} /> : <Invitation owner={owner} state={state} />)}
    {command && <div className="venue-meeting-command" role="status" aria-live="polite">
      {command.phase === 'pending' && 'Sending your action. Keep this context open until the result is known.'}
      {command.phase === 'checking' && 'Checking the original action…'}
      {command.phase === 'unknown' && 'Result uncertain. Your original action is retained; it will not be sent again.'}
      {command.phase === 'applied' && 'Action applied.'}
      {command.phase === 'rejected' && 'Action not applied.'}
      {command.phase === 'unknown' && <button type="button" onClick={() => { void owner.resolve() }}>Check original action</button>}
    </div>}
    {state.notice && <p role="status" className="venue-meeting-explanation">{noticeText[state.notice]}</p>}
    <p className="venue-meeting-safety">A shared place is fixed. A friend’s phone location or virtual position does not confirm arrival.</p>
  </div>
}
export function MeetingUnavailable() {
  return <div className="venue-meeting"><p role="status">Meeting invitations and messages are unavailable until current participant and real-place permissions are verified.</p></div>
}
