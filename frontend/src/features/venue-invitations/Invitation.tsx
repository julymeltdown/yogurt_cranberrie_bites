import type { MeetingLeafOwner } from '../../app/venue-community-entry/owner'
import { commandUnresolved, type MeetingContextView, type MeetingLeafState } from './model'
import './meeting.css'

export function MeetingParticipants({ context }: { readonly context: MeetingContextView }) {
  return <div className="venue-meeting-participants">
    <span className="venue-meeting-portrait" aria-label={context.self.displayName}>{context.self.portrait}</span>
    <span className="venue-meeting-plus" aria-hidden="true">+</span>
    <span className="venue-meeting-portrait" aria-label={context.friend.displayName}>{context.friend.portrait}</span>
    <span>You + {context.friend.displayName}</span>
  </div>
}
export function MeetingPlace({ context, onView }: { readonly context: MeetingContextView; readonly onView?: () => void }) {
  if (!context.landmarkLabel) return null
  return <div className="venue-meeting-place"><span className="venue-meeting-pin" aria-hidden="true">⚑</span>
    <div><strong>{context.landmarkLabel}</strong><span>{context.landmarkDescription}</span></div>
    {onView && <button type="button" onClick={onView}>View on map</button>}
  </div>
}
export function Invitation({ owner, state }: { readonly owner: MeetingLeafOwner; readonly state: MeetingLeafState }) {
  const context = state.context
  if (!context) return null
  if (!context.landmarkLabel) return <p role="status">Choose a reviewed fixed meeting place before sending an invitation.</p>
  const invitation = context.invitation, pending = commandUnresolved(state.command)
  const actions = context.invitationActions
  return <section aria-labelledby="venue-invitation-title" aria-busy={pending}>
    <div className="venue-meeting-invitation">
    <div className="venue-meeting-invitation-heading">
      <div className="venue-meeting-pair" aria-hidden="true"><span>{context.self.portrait}</span><span>{context.friend.portrait}</span></div>
      <div><h2 id="venue-invitation-title">Meet at {context.landmarkLabel}{!invitation || invitation.status === 'pending' ? '?' : ''}</h2>
        <p>You + {context.friend.displayName}</p></div>
    </div>
    <div className="venue-meeting-actions">
      {actions.includes('decline') && <button type="button" disabled={pending} onClick={() => { void owner.execute('decline') }}>Decline</button>}
      {actions.includes('accept') && <button type="button" className="venue-meeting-primary" disabled={pending} onClick={() => { void owner.execute('accept') }}>Accept</button>}
      {actions.includes('create') && <button type="button" className="venue-meeting-primary" disabled={pending} onClick={() => { void owner.execute('create') }}>Send invitation</button>}
      {actions.includes('cancel') && <button type="button" disabled={pending} onClick={() => { void owner.execute('cancel') }}>Cancel meetup</button>}
    </div>
    </div>
    {invitation && <p className={`venue-meeting-status venue-meeting-status-${invitation.status}`}>
      {invitation.status === 'accepted' ? 'Meeting accepted' : `Invitation ${invitation.status}`}
      {invitation.status === 'accepted' && <span aria-hidden="true">✓</span>}
    </p>}
    {!invitation && <p className="venue-meeting-explanation">Invite your friend to this fixed meeting place.</p>}
    <MeetingPlace context={context} {...(state.canViewOnMap ? { onView: () => { owner.viewOnMap() } } : {})} />
    {actions.length === 0 && <p className="venue-meeting-explanation">No invitation action is currently available.</p>}
    <button type="button" className="venue-meeting-text-button" onClick={() => owner.surface('conversation')}>Message about this place</button>
  </section>
}
