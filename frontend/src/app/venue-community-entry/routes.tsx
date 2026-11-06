import { useEffect, useState, useSyncExternalStore } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import { InvitationInbox } from '../../features/venue-invitations/Inbox'
import { emptyMeetingLeaf, type LeafSurface } from '../../features/venue-invitations/model'
import { canonicalId } from '../routes'
import { useMeetingLeafOwner } from './provider'
import { MeetingLeaf, MeetingUnavailable } from './view'

const noSubscribe = () => () => {}, empty = () => emptyMeetingLeaf

function InvitationRoute({ surface }: { readonly surface: LeafSurface }) {
  const owner = useMeetingLeafOwner(), id = useParams().invitationId ?? ''
  const navigate = useNavigate()
  const state = useSyncExternalStore(owner?.subscribe ?? noSubscribe, owner?.read ?? empty, empty)
  const [inboxId, setInboxId] = useState<string | null>(null)
  const [selected, setSelected] = useState<{ owner: typeof owner; id: string; surface: LeafSurface } | null>(null)
  const valid = canonicalId(id), inboxOpen = inboxId === id
  useEffect(() => {
    if (owner && valid) { void owner.openInvitation(id, surface); setSelected({ owner, id, surface }) }
  }, [owner, id, valid, surface])
  useEffect(() => { if (owner && inboxOpen) void owner.openInbox('INCOMING') }, [owner, inboxOpen])
  if (!owner || !valid) return <MeetingUnavailable />
  if (inboxOpen) return <div className="venue-meeting">
    <header className="venue-meeting-header"><strong>Invitations</strong>
      <button type="button" onClick={() => setInboxId(null)}>Back to invitation</button></header>
    <InvitationInbox owner={owner} state={state}
      onDirection={direction => { void owner.openInbox(direction) }}
      onRefresh={() => { void owner.refreshInbox() }} />
  </div>
  if (selected?.owner !== owner || selected.id !== id || selected.surface !== surface) return <p role="status">Reading linked invitation…</p>
  const close = () => navigate(surface === 'conversation' ? `/meet/invitations/${id}` : '/meet')
  const openInbox = () => setInboxId(id)
  if (state.context && state.context.invitation?.id !== id) return <div className="venue-meeting"><p role="status">Another meeting action is still unresolved.</p>
    <MeetingLeaf owner={owner} onClose={close} onOpenInbox={openInbox} /><button type="button" onClick={() => { void owner.openInvitation(id, surface) }}>Read linked invitation</button></div>
  return <MeetingLeaf owner={owner} onClose={close} onOpenInbox={openInbox} />
}
export function VenueInvitationPanel() { return <InvitationRoute surface="invitation" /> }
export function VenueConversationPanel() { return <InvitationRoute surface="conversation" /> }
