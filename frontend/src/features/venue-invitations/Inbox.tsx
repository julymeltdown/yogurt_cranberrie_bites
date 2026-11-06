import { Link } from 'react-router-dom'
import type { MeetingLeafOwner } from '../../app/venue-community-entry/owner'
import { commandUnresolved, invitationExpired, W04_INVITATION_POLICY,
  type InboxDirection, type InvitationInboxView, type InvitationStatus, type InvitationView,
  type MeetingLeafState } from './model'
import './meeting.css'

const statusLabel: Readonly<Record<InvitationStatus, string>> = Object.freeze({ pending: 'Pending',
  accepted: 'Accepted', declined: 'Declined', cancelled: 'Cancelled', expired: 'Expired' })

function expiryLabel(invitation: InvitationView, nowMs: number | null): string {
  if (nowMs === null) return 'Expiry unavailable'
  const remaining = invitation.expiresAtMs - nowMs
  if (remaining <= 0) return `Expired after ${W04_INVITATION_POLICY.expiryHours} hours`
  const minutes = Math.max(1, Math.ceil(remaining / 60_000))
  return minutes >= 60 ? `Expires in ${Math.floor(minutes / 60)}h${minutes % 60 ? ` ${minutes % 60}m` : ''}` : `Expires in ${minutes}m`
}

const inboxNoticeText: Readonly<Record<NonNullable<InvitationInboxView['notice']>, string>> = Object.freeze({
  rejected: 'The action was not applied. The current server status is shown.',
  denied: 'The server denied this action. Current permissions and invitation status were refreshed.',
  conflict: 'This invitation changed. The current server status is shown; check it before trying again.',
  unavailable: 'Invitation details are unavailable. The server did not authorize this context.',
  unknown: 'The result is uncertain. The original action is retained and will not be sent again.',
})

export function InvitationInbox({ owner, state, onDirection, onRefresh }: {
  readonly owner: MeetingLeafOwner
  readonly state: MeetingLeafState
  readonly onDirection?: (direction: InboxDirection) => void
  readonly onRefresh?: () => void
}) {
  const inbox = state.inbox, pending = commandUnresolved(state.command) || inbox.phase === 'loading'
  const direction = inbox.direction
  const items = inbox.items
  return <section className="venue-invitation-inbox" aria-labelledby="venue-invitation-inbox-title" aria-busy={pending}>
    <header className="venue-invitation-inbox-header">
      <div><h2 id="venue-invitation-inbox-title">Invitations</h2>
        <p>Event-scoped meeting invitations for authorized friends.</p></div>
      {onRefresh && <button type="button" disabled={pending || inbox.phase === 'idle'} onClick={onRefresh}>Refresh</button>}
    </header>
    <div className="venue-invitation-tabs" role="tablist" aria-label="Invitation direction">
      {(['INCOMING', 'OUTGOING'] as const).map((value, index) => <button type="button" role="tab" key={value}
        id={`venue-invitation-tab-${value}`} aria-controls="venue-invitation-list-panel" aria-selected={direction === value}
        tabIndex={direction === value ? 0 : -1} disabled={pending}
        onKeyDown={event => {
          if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return
          event.preventDefault()
          const tabs = ['INCOMING', 'OUTGOING'] as const
          const target = event.key === 'Home' ? 0 : event.key === 'End' ? 1
            : (index + (event.key === 'ArrowRight' ? 1 : -1) + tabs.length) % tabs.length
          onDirection?.(tabs[target]!)
          document.getElementById(`venue-invitation-tab-${tabs[target]}`)?.focus()
        }}
        onClick={() => onDirection?.(value)}>
        {value === 'INCOMING' ? 'Incoming' : 'Outgoing'}
      </button>)}
    </div>
    <section className="venue-invitation-policy" aria-labelledby="venue-invitation-policy-title">
      <h3 id="venue-invitation-policy-title">Party invitation policy</h3>
      <ul>
        <li>Invitee consent: only the invited friend can accept or decline.</li>
        <li>{W04_INVITATION_POLICY.inviterGrantsPerParty} inviter grant per party.</li>
        <li>Capacity {W04_INVITATION_POLICY.capacity}: the party is full at {W04_INVITATION_POLICY.capacity} people.</li>
        <li>{W04_INVITATION_POLICY.expiryHours}-hour expiry from the server-issued invitation time.</li>
        <li>The host can revoke an invitation before the server deadline.</li>
      </ul>
    </section>
    {inbox.capacity && <p className={`venue-invitation-capacity${inbox.capacity.joined >= inbox.capacity.limit ? ' venue-invitation-capacity-full' : ''}`}
      data-capacity-full={inbox.capacity.joined >= inbox.capacity.limit ? 'true' : 'false'} role="status">
      {inbox.capacity.joined} of {inbox.capacity.limit} party places{inbox.capacity.joined >= inbox.capacity.limit ? ' · Full' : ''}
    </p>}
    {inbox.notice && <p className="venue-invitation-inbox-notice" role="status">{inboxNoticeText[inbox.notice]}</p>}
    {state.command && <p className="venue-invitation-inbox-command" role="status" aria-live="polite">
      {state.command.phase === 'pending' && 'Sending your action. Keep this screen open until the result is known.'}
      {state.command.phase === 'checking' && 'Checking the original action…'}
      {state.command.phase === 'unknown' && 'Result uncertain. The original action is retained; it will not be sent again.'}
      {state.command.phase === 'applied' && 'Action applied.'}
      {state.command.phase === 'rejected' && 'Action not applied.'}
      {state.command.phase === 'unknown' && <button type="button" onClick={() => { void owner.resolve() }}>Check original action</button>}
    </p>}
    <div id="venue-invitation-list-panel" role="tabpanel" aria-labelledby={`venue-invitation-tab-${direction}`}>
    {inbox.phase === 'loading' && items.length === 0 && <p className="venue-invitation-inbox-empty" role="status">Reading invitations…</p>}
    {inbox.phase === 'error' && <div className="venue-invitation-inbox-empty" role="status">
      <p>Invitations are unavailable. The server did not authorize the current read.</p>
      {onRefresh && <button type="button" onClick={onRefresh}>Try again</button>}
    </div>}
    {inbox.phase !== 'error' && items.length === 0 && inbox.phase !== 'idle' && inbox.phase !== 'loading'
      && <p className="venue-invitation-inbox-empty" role="status">
        {direction === 'INCOMING' ? 'No incoming invitations.' : 'No outgoing invitations.'}
      </p>}
    {items.length > 0 && <ul className="venue-invitation-list" aria-label={direction === 'INCOMING' ? 'Incoming invitations' : 'Outgoing invitations'}>
      {items.map(item => {
        const invitation = item.invitation, expired = invitationExpired(invitation, state.nowMs)
        const status: InvitationStatus = expired ? 'expired' : invitation.status
        const actions = expired || !item.target || !item.presentation ? [] : invitation.allowedActions
        const name = item.presentation?.friend.displayName ?? null
        const place = item.presentation?.landmarkLabel ?? null
        return <li className="venue-invitation-row" key={invitation.id} data-invitation-id={invitation.id}
          data-server-status={invitation.status} data-display-status={status} aria-busy={pending}>
          <div className="venue-invitation-row-heading">
            {item.presentation?.friend.portrait != null && <span className="venue-invitation-portrait" aria-hidden="true">{item.presentation.friend.portrait}</span>}
            <div className="venue-invitation-row-copy">
              <strong>{name ?? 'Participant unavailable'}</strong>
              <span>{direction === 'INCOMING' ? 'From' : 'To'} {name ?? 'an unavailable participant'}</span>
            </div>
            <span className={`venue-invitation-state venue-invitation-state-${status}`}>{statusLabel[status]}</span>
          </div>
          <p className="venue-invitation-place">{place ?? 'Place details unavailable'}</p>
          <p className="venue-invitation-expiry">{expiryLabel(invitation, state.nowMs)}</p>
          <div className="venue-invitation-row-actions">
            <Link className="venue-invitation-open" to={`/meet/invitations/${invitation.id}`}
              aria-label={`Open invitation ${direction === 'INCOMING' ? 'from' : 'to'} ${name ?? 'unavailable participant'}`}>Open</Link>
            {actions.includes('accept') && <button type="button" className="venue-invitation-accept" disabled={pending}
              onClick={() => { void owner.decideInbox(invitation.id, 'accept') }}>Accept</button>}
            {actions.includes('decline') && <button type="button" disabled={pending}
              onClick={() => { void owner.decideInbox(invitation.id, 'decline') }}>Decline</button>}
            {actions.includes('cancel') && <button type="button" disabled={pending}
              onClick={() => { void owner.decideInbox(invitation.id, 'cancel') }}>Cancel invitation</button>}
          </div>
          {!item.target || !item.presentation ? <p className="venue-invitation-explanation">
            Current participant or place details are unavailable, so no invitation action can be issued from this row.
          </p> : null}
        </li>
      })}
    </ul>}
    {inbox.nextCursor && inbox.phase === 'ready' && <button type="button" className="venue-invitation-more"
      disabled={pending} onClick={() => { void owner.loadMoreInbox() }}>Load more invitations</button>}
    </div>
  </section>
}
