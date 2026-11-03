import { createContext, useContext, useEffect, useState, useSyncExternalStore, type ReactNode } from 'react'
import { useParams } from 'react-router-dom'
import { MeetingBeaconDetail, MeetingBeaconForm } from '../../features/meet-beacon'
import { createMeetingEntry, unavailableMeetingEntry, type MeetingEntry } from './bridge'
import type { MeetingEntryAuth, MeetingEntryRelease } from './ports'
import { canonicalId } from '../routes'

const Context = createContext<MeetingEntry | null>(null)
const noSubscribe = () => () => {}, empty = () => unavailableMeetingEntry
/** Mount once inside the authorized shell, outside panels. Construction in an effect obeys StrictMode disposal. */
export function MeetingEntryFeatures({ auth, release, children }: {
  readonly auth: MeetingEntryAuth | null; readonly release: MeetingEntryRelease | null; readonly children: ReactNode
}) {
  const [owner, setOwner] = useState<{ auth: MeetingEntryAuth | null; release: MeetingEntryRelease | null; entry: MeetingEntry } | null>(null)
  useEffect(() => {
    const entry = createMeetingEntry({ auth, release }); setOwner({ auth, release, entry })
    return () => entry.dispose()
  }, [auth, release])
  const current = owner?.auth === auth && owner.release === release ? owner.entry : null
  return <Context value={current}>{children}</Context>
}
/** Read-only current feature owner for separately owned rally/pickup composition. */
export function useMeetingEntry() {
  const entry = useContext(Context)
  return useSyncExternalStore(entry?.subscribe ?? noSubscribe, entry?.read ?? empty, empty)
}
export function MeetingCheckInPanel() {
  const { controller } = useMeetingEntry()
  const [selected, setSelected] = useState<typeof controller>(null)
  useEffect(() => { if (controller) { controller.select(null); setSelected(controller) } }, [controller])
  return controller && selected === controller ? <MeetingBeaconForm controller={controller} /> : <MeetingUnavailable />
}
export function MeetingBeaconPanel({ beaconId: supplied, ownerActions = false }: { readonly beaconId?: string; readonly ownerActions?: boolean }) {
  const params = useParams(), id = supplied ?? params.beaconId ?? '', { controller } = useMeetingEntry()
  const valid = canonicalId(id)
  // Never render a previous target while the passive selection effect catches up.
  const [selection, setSelection] = useState<{ controller: typeof controller; id: string } | null>(null)
  useEffect(() => { if (controller) void controller.select(valid ? id : null); setSelection({ controller, id }) }, [controller, id, valid])
  if (!controller || !valid) return <MeetingUnavailable />
  if (selection?.controller !== controller || selection.id !== id) return <p role="status">Reading current meeting…</p>
  return <LinkedMeeting controller={controller} id={id} ownerActions={ownerActions} />
}
function LinkedMeeting({ controller, id, ownerActions }: { controller: NonNullable<ReturnType<typeof useMeetingEntry>['controller']>; id: string; ownerActions: boolean }) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read)
  if (state.beacon && state.beacon.beacon_id !== id) return <div className="meeting-beacon"><p role="status">Meeting information changed while an action was in progress.</p>
    <button type="button" onClick={() => { void controller.select(id) }}>Read linked meeting</button></div>
  return <MeetingBeaconDetail controller={controller} ownerActions={ownerActions} />
}
function MeetingUnavailable() {
  return <p role="status">Real-world meeting guidance is unavailable until current place, audience, and expiry information is verified.</p>
}
