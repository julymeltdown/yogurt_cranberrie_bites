import { createContext, useContext, useLayoutEffect, useState, useSyncExternalStore, type ReactNode } from 'react'
import { useParams } from 'react-router-dom'
import type { SessionController } from '../../features/auth/model/session'
import { PlaceDetail, PlacesOverview, type PlacesController } from '../../features/places'
import { createPlacesEntry, type PlacesEntry, type PlacesEntryPort } from './owner'

const Context = createContext<PlacesController | null>(null)
const empty = () => null, noop = () => () => {}
/** Mount once around the shell's stable lifetime, not separately per panel. */
export function PlacesEntryProvider({ auth, source, children }: {
  readonly auth: SessionController | null; readonly source: PlacesEntryPort | null; readonly children: ReactNode
}) {
  const release = useSyncExternalStore(source ? listener => source.subscribe(listener) : noop, source ? () => source.read() : empty, empty)
  const [owner, setOwner] = useState<PlacesEntry | null>(null)
  useLayoutEffect(() => {
    if (!auth || !source || !release) return
    let entry: PlacesEntry
    try { entry = createPlacesEntry({ auth, source, release }) } catch { return }
    setOwner(entry)
    return entry.dispose
  }, [auth, source, release])
  const current = owner?.auth === auth && owner.source === source && owner.release === release ? owner : null
  const controller = useSyncExternalStore(current?.subscribe ?? noop, current?.read ?? empty, empty)
  return <Context value={controller}>{children}</Context>
}
export function usePlacesEntry() { return useContext(Context) }
export function PlacesOverviewRoute() {
  const controller = usePlacesEntry()
  return controller ? <PlacesOverview controller={controller} /> : <PlacesUnavailable />
}
export function PlaceDetailRoute() {
  const controller = usePlacesEntry(), { placeId = '' } = useParams()
  return controller ? <PlaceDetail controller={controller} placeId={placeId} /> : <PlacesUnavailable />
}
function PlacesUnavailable() {
  return <p role="status">Places are unavailable until your current event map is verified. Return to the world to check access.</p>
}
