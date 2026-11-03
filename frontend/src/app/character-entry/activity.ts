import { useSyncExternalStore } from 'react'
import type { ProfileActivityInput } from '../../features/me/ui/reference-profile/ProfileActivity'
import { useSocialOwner } from '../social-composition/provider'

const empty = () => null, noSubscribe = () => () => {}
/** Observe an existing authorized feed page only. Opening a profile never starts a feed read. */
export function useLoadedProfileActivity(subjectUserId: string): ProfileActivityInput | undefined {
  const owner = useSocialOwner()
  const feed = useSyncExternalStore(owner?.feed.subscribe ?? noSubscribe, owner?.feed.read ?? empty, empty)
  if (!owner || owner.isDisposed() || !owner.auth.privacy.isCurrent(owner.scope) || !owner.available('feed')
    || !subjectUserId || !feed?.available || feed.feed.phase !== 'ready' || !feed.feed.data) return undefined
  return { scope: owner.scope, subjectUserId, page: feed.feed.data }
}
