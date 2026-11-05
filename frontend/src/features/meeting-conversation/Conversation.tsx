import { useEffect, useId, useRef } from 'react'
import type { MeetingLeafOwner } from '../../app/venue-community-entry/owner'
import { MeetingParticipants, MeetingPlace } from '../venue-invitations/Invitation'
import { commandUnresolved, type MeetingLeafState } from '../venue-invitations/model'

export function MeetingConversation({ owner, state }: { readonly owner: MeetingLeafOwner; readonly state: MeetingLeafState }) {
  const inputId = useId(), context = state.context
  const input = useRef<HTMLTextAreaElement>(null)
  useEffect(() => { if (state.composerOpen && context?.canSend) input.current?.focus() }, [state.composerOpen, context?.contextId, context?.canSend])
  if (!context) return null
  const pending = commandUnresolved(state.command), invitation = context.invitation
  return <section className="venue-meeting-conversation" aria-labelledby="venue-conversation-title" aria-busy={pending}>
    <h2 id="venue-conversation-title">{context.landmarkLabel ? `Meet at ${context.landmarkLabel}` : `Message ${context.friend.displayName}`}</h2>
    <MeetingParticipants context={context} />
    <p className="venue-meeting-guidance">{context.landmarkLabel ? 'Use the shared place below to meet up.' : 'A conversation with your friend for this event.'}</p>
    <MeetingPlace context={context} {...(state.canViewOnMap ? { onView: () => { owner.viewOnMap() } } : {})} />
    <ol className="venue-meeting-messages" aria-label="Messages about this place">
      {context.messages.map(message => {
        const self = message.authorId === context.self.userId
        const author = self ? context.self : context.friend
        return <li key={message.id} className={self ? 'venue-meeting-message-self' : undefined}>
          <span className="venue-meeting-message-portrait" aria-hidden="true">{author.portrait}</span>
          <div><div className="venue-meeting-message-heading"><strong>{self ? 'You' : author.displayName}</strong><small>{message.timeLabel}</small></div>
            <p>{message.body}</p></div>
        </li>
      })}
    </ol>
    {context.messages.length === 0 && <p className="venue-meeting-empty">Start a conversation about meeting here.</p>}
    {context.conversation?.earlierCursor && <p className="venue-meeting-explanation">Showing the current message window.</p>}
    {invitation && <div className={`venue-meeting-status venue-meeting-status-${invitation.status}`}>
      <span>{invitation.status === 'accepted' ? 'Meeting accepted' : `Invitation ${invitation.status}`}</span>
      {invitation.status === 'accepted' && <span aria-hidden="true">✓</span>}
    </div>}
    {context.canSend && state.composerOpen ? <form className="venue-meeting-composer" onSubmit={event => { event.preventDefault(); void owner.execute('send') }}>
      <label htmlFor={inputId}>Message</label>
      <textarea ref={input} id={inputId} rows={2} value={state.draft} disabled={pending}
        placeholder="Message about this meeting place…" onChange={event => owner.setDraft(event.target.value)}
        aria-describedby={`${inputId}-policy`} />
      <p id={`${inputId}-policy`} className="venue-meeting-explanation">{context.messagePolicy.hint}</p>
      <button type="submit" className="venue-meeting-primary" disabled={pending || state.draft.trim().length === 0}>Send message</button>
    </form> : !context.canSend && <p className="venue-meeting-explanation">Sending is currently unavailable for this context.</p>}
    <div className="venue-meeting-actions">
      {context.canSend && !state.composerOpen
        ? <button type="button" onClick={() => owner.surface('conversation')}>Message</button>
        : context.landmarkLabel && <button type="button" onClick={() => owner.surface('invitation')}>Invitation</button>}
      {context.invitationActions.includes('cancel') && <button type="button" disabled={pending} onClick={() => { void owner.execute('cancel') }}>Cancel meetup</button>}
    </div>
  </section>
}
