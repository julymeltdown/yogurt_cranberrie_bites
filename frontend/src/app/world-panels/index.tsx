import { createContext, useContext, useMemo, useSyncExternalStore, type ReactNode } from 'react'
import type { PrivacyScope } from '../../shared/query/privacy'
import type { WorldPresentationPort, WorldPresentationRelease } from '../../world/composition'
import type { PlacesEntryPort, PlacesEntryRelease } from '../places-entry'
import { PlacesEntryProvider, PlacesOverviewRoute, PlaceDetailRoute } from '../places-entry'
import { MuralEntryProvider, MuralWallPanel } from '../mural-entry'
import { EraControls } from '../shell-chrome'
import './presentation.css'
import { useWorldPanelOwnership } from './ownership'
import { usePorts } from '../access'
const Context = createContext<WorldPresentationRelease | null>(null)
const empty = () => null, noop = () => () => {}
/** Pure projection: no asset, controller, camera, scene, socket or privacy authority is created here. */
export function createPlacesPresentationSource(source: WorldPresentationPort): PlacesEntryPort {
  let previous: WorldPresentationRelease | null = null, release: PlacesEntryRelease | null = null
  return { subscribe: source.subscribe, read() {
    const current = source.read()
    if (!current || current.request.signal.aborted) { previous = null; release = null; return null }
    if (current !== previous) {
      previous = current
      release = Object.freeze({ scope: current.request.privacyScope, signal: current.request.signal,
        ...(current.overview ? { overview: { read: () => source.read() === current ? current.overview!.read() : null, subscribe: current.overview.subscribe } } : {}),
        map: { read: () => source.read() === current ? current.map : null, subscribe: source.subscribe },
        camera: { read: () => source.read() === current ? current.camera : null } })
    }
    return release
  } }
}
/** Stable shell lifetime keeps uncertain mural commands across navigation and release withdrawal. */
export function WorldPanels({ source, scope, children }: { source: WorldPresentationPort | null; scope: PrivacyScope | null; children: ReactNode }) {
  const { auth } = usePorts(), ownership = useWorldPanelOwnership()
  const current = useSyncExternalStore(source?.subscribe ?? noop, source?.read ?? empty, empty)
  const mural = useSyncExternalStore(current?.mural?.subscribe ?? noop, current?.mural?.read ?? empty, empty)
  const places = useMemo(() => source ? createPlacesPresentationSource(source) : null, [source])
  const wall = ownership.mural ? <MuralEntryProvider auth={auth ?? null} scope={scope} release={mural}>{children}</MuralEntryProvider> : children
  return <Context value={current}>{ownership.places ? <PlacesEntryProvider auth={auth ?? null} source={places}>{wall}</PlacesEntryProvider> : wall}</Context>
}
export function ProductionEraControls() { const release = useContext(Context); return <EraControls controller={release?.theme ?? null} /> }
export function ProductionThemePanel() { return <><ProductionEraControls /><p>Era preferences are saved on this device for this account and event. Server preference synchronization is unavailable.</p></> }
export function ProductionMapPanel() { return <div className="mcr-root-map"><PlacesOverviewRoute /></div> }
export { PlaceDetailRoute as ProductionPlacePanel, MuralWallPanel as ProductionWallPanel }
