import { createSceneryOverview } from '../../world/illustrated/overview'
import { createReferenceSceneOverview, referenceSceneIdentity } from '../../world/reference-scene'
import { validateReferenceMetadata } from '../../world/reference-scene/model'
import type { LoadedMap } from '../../world/map/resources'
import type { RuntimeOverview } from '../../world/runtime'

type Metadata = Pick<LoadedMap, 'map' | 'theme' | 'widthPx' | 'heightPx' | 'spawn'>
/** Any reference claim must qualify as the exact coherent release, never fall back to old pixels. */
export function isReferenceSceneCandidate(metadata: Pick<Metadata, 'map' | 'theme'>) {
  return metadata.map.map_id === referenceSceneIdentity.mapId || metadata.map.geometry_hash === referenceSceneIdentity.geometryHash
    || [metadata.theme.scenery?.ground.path, ...metadata.theme.scenery?.atlases.map(a => a.path) ?? []]
      .some(path => path?.startsWith('world/reference-scene-v2/runtime-v4/'))
}
/** Called after ordinary manifest validation and before resource or room HTTP. */
export function preflightReferenceScene(metadata: Metadata) {
  if (isReferenceSceneCandidate(metadata)) validateReferenceMetadata(metadata)
}

/** One runtime owner multiplexes its two projections, borrowing only the current loaded map. */
export function createRootSceneOverview(): RuntimeOverview {
  const incumbent = createSceneryOverview(), reference = createReferenceSceneOverview()
  let active: RuntimeOverview | null = null, disposed = false, generation = 0
  const listeners = new Set<() => void>()
  const notify = () => { for (const listener of Array.from(listeners)) { try { listener() } catch { /* Independent views. */ } } }
  const stops = [incumbent, reference].map(owner => owner.subscribe(() => { if (!disposed && active === owner) notify() }))
  return {
    read: () => !disposed ? active?.read() ?? null : null,
    subscribe(listener) { if (disposed) return () => {}; listeners.add(listener); return () => { listeners.delete(listener) } },
    replace(map) {
      if (disposed) return
      const ticket = ++generation, previous = active; active = null
      previous?.replace(null); notify()
      if (disposed || generation !== ticket || !map) return
      try {
        preflightReferenceScene(map)
        const next = isReferenceSceneCandidate(map) ? reference : incumbent
        active = next; next.replace(map)
      } catch { /* A malformed reference claim stays withdrawn; no incumbent fallback. */ }
    },
    dispose() {
      if (disposed) return
      disposed = true; generation++; active = null; notify(); listeners.clear()
      for (const stop of stops) { try { stop() } catch { /* Finish releasing both owners. */ } }
      for (const owner of [incumbent, reference]) { try { owner.dispose() } catch { /* Finish independent teardown. */ } }
    },
  }
}
