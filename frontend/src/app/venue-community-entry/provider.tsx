import { createContext, useContext, useLayoutEffect, useState, useSyncExternalStore, type ReactNode } from 'react'
import type { MeetingConsumerPort, MeetingLeafSnapshot } from '../../features/paradise-city-map/ports'
import { commandUnresolved, emptyMeetingLeaf } from '../../features/venue-invitations/model'
import { createMeetingLeafOwner, type MeetingLeafOwner } from './owner'
import type { MeetingLeafOptions } from './ports'
import { MeetingLeaf } from './view'

const Context = createContext<MeetingLeafOwner | null>(null)
/** Bind to the ONE primary consumer type; do not publish a second capability family. */
export function meetingConsumer(owner: MeetingLeafOwner): MeetingConsumerPort {
  let previous = owner.read(), snapshot: MeetingLeafSnapshot | null = null
  const read = () => {
    const current = owner.read()
    if (previous !== current || snapshot === null) {
      previous = current
      snapshot = current.phase === 'withdrawn' || current.phase === 'idle' ? null
        : Object.freeze({ active: current.active, locked: commandUnresolved(current.command), content: <MeetingLeaf owner={owner} /> })
    }
    return snapshot
  }
  return { read, subscribe: owner.subscribe, open: owner.open, close: owner.close }
}
/** Root mounts once inside canonical Auth, outside route switching. Never construct on a route. */
export function VenueCommunityFeatures({ children, auth, release, newCommandKey, bindConsumer }: MeetingLeafOptions & {
  readonly children: ReactNode
  readonly bindConsumer?: (consumer: MeetingConsumerPort | null) => void
}) {
  const [bound, setBound] = useState<{ auth: typeof auth; release: typeof release; newCommandKey: typeof newCommandKey; owner: MeetingLeafOwner } | null>(null)
  useLayoutEffect(() => {
    const owner = createMeetingLeafOwner({ auth, release, newCommandKey })
    setBound({ auth, release, newCommandKey, owner })
    bindConsumer?.(meetingConsumer(owner))
    return () => { owner.dispose(); bindConsumer?.(null) }
  }, [auth, release, newCommandKey, bindConsumer])
  const owner = bound?.auth === auth && bound.release === release && bound.newCommandKey === newCommandKey
    && release && auth?.privacy.isCurrent(release.scope) && !release.signal.aborted ? bound.owner : null
  return <Context value={owner}>{children}</Context>
}
export const useMeetingLeafOwner = () => useContext(Context)
const noSubscribe = () => () => {}, empty = () => emptyMeetingLeaf
export function VenueCommunityOutlet() {
  const owner = useMeetingLeafOwner()
  useSyncExternalStore(owner?.subscribe ?? noSubscribe, owner?.read ?? empty, empty)
  return owner?.read().active ? <MeetingLeaf owner={owner} /> : null
}
