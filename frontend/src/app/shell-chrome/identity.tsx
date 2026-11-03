import { useSyncExternalStore } from 'react'
import { ShellHeader } from './index'
import { useAccountOwner } from '../account-entry/provider'
import { decodeParticipant, type Participant } from '../../features/me/model/contract'
import { AvatarThumbnail } from '../../features/me/ui/AvatarThumbnail'
import type { ShellBootstrap } from '../ports'
const none = () => null, noop = () => () => {}
/** Reads already authorized data; no fetch, extra profile controller or art fallback. */
export function SessionShellHeader({ bootstrap }: { bootstrap: ShellBootstrap }) {
  const owner = useAccountOwner()
  const state = useSyncExternalStore(owner?.me.subscribe ?? noop, owner?.me.read ?? none)
  let participant: Participant | null = state?.participant ?? null
  if (!participant && owner && owner.auth.privacy.isCurrent(owner.scope)) {
    try { participant = decodeParticipant(bootstrap.document?.participant, owner.scope) } catch { /* No identity invented when this projection is unavailable. */ }
  }
  const avatar = participant && owner?.source ? <AvatarThumbnail auth={owner.auth} config={participant.profile.avatar.config} source={owner.source} /> : undefined
  return <ShellHeader eventTitle={bootstrap.event.title} {...(participant ? { displayName: participant.profile.display_name } : {})} {...(avatar ? { avatar } : {})} />
}
