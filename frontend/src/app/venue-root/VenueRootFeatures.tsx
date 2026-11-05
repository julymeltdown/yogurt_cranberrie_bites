import { useLayoutEffect, useState, useSyncExternalStore, type ReactNode } from 'react'
import { ParadiseCityVenueProvider } from '../venue-entry/ParadiseCityMapPanel'
import { VenueCommunityFeatures } from '../venue-community-entry'
import { createVenueRootBinding, type VenueRootBinding } from './binding'
import type { VenueRootInput } from './input'

const emptySubscribe = () => () => {}, emptyRead = () => false
const noCommandKey = () => { throw Error('No current venue command source') }
function Providers({ binding, children }: { readonly binding: VenueRootBinding | null; readonly children: ReactNode }) {
  const current = useSyncExternalStore(binding?.subscribe ?? emptySubscribe, binding?.read ?? emptyRead, emptyRead)
  return <VenueCommunityFeatures auth={current && binding ? binding.input.auth : null}
    release={current && binding ? binding.release : null} newCommandKey={binding?.input.newCommandKey ?? noCommandKey}
    {...(binding ? { bindConsumer: binding.bindConsumer } : {})}>
    <ParadiseCityVenueProvider input={current && binding ? binding.mapInput : null} {...(binding ? { bindFocus: binding.bindFocus } : {})}>
      {children}
    </ParadiseCityVenueProvider>
  </VenueCommunityFeatures>
}
/** Mount under the existing selected-character/Auth providers, OUTSIDE Outlet.
 * borrowed is stable per original shell/image release, never keyed by route. */
export function VenueRootFeatures({ borrowed, children }: { readonly borrowed: VenueRootInput | null; readonly children: ReactNode }) {
  const [bound, setBound] = useState<{ input: VenueRootInput; binding: VenueRootBinding } | null>(null)
  useLayoutEffect(() => {
    if (!borrowed) return
    const binding = createVenueRootBinding(borrowed)
    setBound({ input: borrowed, binding })
    // Parent cleanup scrubs bridge references before descendant provider disposal.
    return () => binding.dispose()
  }, [borrowed])
  const binding = bound?.input === borrowed ? bound.binding : null
  return <Providers binding={binding}>{children}</Providers>
}
