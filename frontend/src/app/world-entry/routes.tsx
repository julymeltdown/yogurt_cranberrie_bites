import { useCallback, createContext, useContext, useEffect, useState, type ReactNode } from 'react'
import { WorldRooms, type ReplicationReader } from '../../features/world-rooms'
import { WorldNearby } from '../../features/world-nearby'
import type { NearbyPerson } from '../../features/world-nearby/projection'
import { useReviewedCharacterOwner } from '../character-entry/provider'
import { ReviewedCharacterView } from '../character-entry/ReviewedCharacterView'
import { createWorldAccessBridge, type WorldAccessBridge } from './bridge'
import type { EntryAuth, WorldEntryRequest } from './authority'

const AccessContext = createContext<WorldAccessBridge | null>(null)
/** Mount once inside the authorized shell, outside route panels. No transport/runtime ownership. */
export function WorldAccessFeatures({ auth, replication, release, children }: {
  auth: EntryAuth; replication: ReplicationReader; release: WorldEntryRequest; children: ReactNode
}) {
  const [owner, setOwner] = useState<{ auth: EntryAuth; replication: ReplicationReader; release: WorldEntryRequest; bridge: WorldAccessBridge } | null>(null)
  useEffect(() => {
    let bridge: WorldAccessBridge
    try { bridge = createWorldAccessBridge({ auth, replication, release }) } catch { return }
    setOwner({ auth, replication, release, bridge })
    return () => bridge.dispose()
  }, [auth, replication, release])
  const current = owner?.auth === auth && owner.replication === replication && owner.release === release ? owner.bridge : null
  return <AccessContext value={current}>{children}</AccessContext>
}
export function WorldRoomsPanel() {
  const bridge = useContext(AccessContext)
  return bridge ? <WorldRooms controller={bridge.controller} /> : <p role="status">Room access is unavailable until your current world access is verified.</p>
}
export function WorldNearbyPanel() {
  const bridge = useContext(AccessContext), owner = useReviewedCharacterOwner()
  const renderAvatar = useCallback((person: NearbyPerson) => owner?.isCurrent() && person.avatar?.config
    ? <ReviewedCharacterView owner={owner} config={person.avatar.config} label={`${person.displayName}'s avatar`} size={64} />
    : <span role="img" aria-label={`${person.displayName}'s avatar unavailable`}>Character unavailable</span>, [owner])
  return bridge ? <WorldNearby controller={bridge.controller} renderAvatar={renderAvatar} /> : <p role="status">Nearby people are unavailable until your current world access is verified.</p>
}
