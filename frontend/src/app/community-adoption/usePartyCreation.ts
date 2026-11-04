import { useEffect, useState, useSyncExternalStore } from 'react'
import type { PartyAuthoringController } from '../../features/party-authoring'
import { useCommunityEntry } from '../community-entry'
const noSubscribe = () => () => {}
/** Allocate through the actual bridge in an effect, never through a render side effect. */
export function usePartyCreation() {
  const { entry, state } = useCommunityEntry()
  const [selection, setSelection] = useState<{ entry: typeof entry; author: PartyAuthoringController | null } | null>(null)
  useEffect(() => {
    setSelection(entry && state.phase === 'available' ? { entry, author: entry.partyAuthor() } : null)
  }, [entry, state.phase, state.revision])
  const author = state.phase === 'available' && selection?.entry === entry ? selection.author : null
  const access = useSyncExternalStore(author?.subscribe ?? noSubscribe, () => author?.read().access ?? false, () => false)
  return access && !!author?.places().length
}
