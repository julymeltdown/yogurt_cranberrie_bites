import { createContext, useContext, useEffect, useState, type ReactNode } from 'react'
import { ParadiseCityMap } from '../../features/paradise-city-map'
import type { PersonView, VenueMapFocusPort } from '../../features/paradise-city-map/ports'
import { createVenueConsumer, type VenueConsumerInput } from './consumer'
import { useReviewedCharacterOwner } from '../character-entry/provider'
import { ReviewedCharacterView } from '../character-entry/ReviewedCharacterView'

const Context = createContext<ReturnType<typeof createVenueConsumer> | null>(null)
/** Root mounts once around its stable Outlet, borrowing SAME Auth/privacy/bootstrap binding. */
export function ParadiseCityVenueProvider({ input, children, bindFocus }: { readonly input: VenueConsumerInput | null; readonly children: ReactNode; readonly bindFocus?: (port: VenueMapFocusPort | null) => void }) {
  const [bound, setBound] = useState<{ input: VenueConsumerInput; consumer: ReturnType<typeof createVenueConsumer> } | null>(null)
  useEffect(() => {
    if (!input) return
    const consumer = createVenueConsumer(input); setBound({ input, consumer })
    return () => consumer.dispose()
  }, [input])
  const current = bound?.input === input && input && !input.signal.aborted && input.auth.privacy.isCurrent(input.scope) ? bound.consumer : null
  useEffect(() => {
    if (!bindFocus || !current) return
    bindFocus({ focus: current.controller.focus }); return () => bindFocus(null)
  }, [current, bindFocus])
  return <Context value={current}>{children}</Context>
}
/** Existing /world/map mount remains Root-owned. */
export function ParadiseCityMapPanel({ renderPortrait }: {
  readonly renderPortrait?: (person: Pick<PersonView, 'displayName' | 'avatar'>, size: 64 | 128) => ReactNode
}) {
  const current = useContext(Context), owner = useReviewedCharacterOwner()
  const portrait = renderPortrait ?? ((person, size) => owner?.isCurrent()
    ? <ReviewedCharacterView owner={owner} config={person.avatar} label={`${person.displayName}'s character`} size={size} />
    : <span role="img" aria-label={`${person.displayName}'s character unavailable`} className="pc-portrait-fallback">Character unavailable</span>)
  return current ? <ParadiseCityMap controller={current.controller} renderPortrait={portrait} />
    : <p role="status">The venue map is unavailable until current access and siteplan inputs are supplied.</p>
}
