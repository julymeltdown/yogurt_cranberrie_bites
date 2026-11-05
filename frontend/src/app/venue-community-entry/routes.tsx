import { useEffect, useState } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import type { LeafSurface } from '../../features/venue-invitations/model'
import { canonicalId } from '../routes'
import { useMeetingLeafOwner } from './provider'
import { MeetingLeaf, MeetingUnavailable } from './view'

function InvitationRoute({ surface }: { readonly surface: LeafSurface }) {
  const owner = useMeetingLeafOwner(), id = useParams().invitationId ?? ''
  const navigate = useNavigate()
  const [selected, setSelected] = useState<{ owner: typeof owner; id: string; surface: LeafSurface } | null>(null)
  const valid = canonicalId(id)
  useEffect(() => {
    if (owner && valid) { void owner.openInvitation(id, surface); setSelected({ owner, id, surface }) }
  }, [owner, id, valid, surface])
  if (!owner || !valid) return <MeetingUnavailable />
  if (selected?.owner !== owner || selected.id !== id || selected.surface !== surface) return <p role="status">Reading linked invitation…</p>
  const state = owner.read()
  const close = () => navigate(surface === 'conversation' ? `/meet/invitations/${id}` : '/meet')
  if (state.context && state.context.invitation?.id !== id) return <div className="venue-meeting"><p role="status">Another meeting action is still unresolved.</p>
    <MeetingLeaf owner={owner} onClose={close} /><button type="button" onClick={() => { void owner.openInvitation(id, surface) }}>Read linked invitation</button></div>
  return <MeetingLeaf owner={owner} onClose={close} />
}
export function VenueInvitationPanel() { return <InvitationRoute surface="invitation" /> }
export function VenueConversationPanel() { return <InvitationRoute surface="conversation" /> }
