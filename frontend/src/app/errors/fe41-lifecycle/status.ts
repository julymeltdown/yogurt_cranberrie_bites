import type { WorldEntryRequest } from '../../world-entry/authority'
import type { WorldCompositionState } from '../../../world/composition'

/** Local lifecycle projection, not a server maintenance or application-version decision. */
export interface WorldLifecycleStatus {
  readonly release: WorldEntryRequest
  readonly kind: 'loading' | 'ready' | 'unavailable' | 'version'
}
export interface WorldLifecycleReader {
  read(): WorldLifecycleStatus | null
  subscribe(listener: () => void): () => void
}
export function projectWorldLifecycle(release: WorldEntryRequest, state: Pick<WorldCompositionState, 'phase' | 'problem'>): WorldLifecycleStatus {
  const kind = state.phase === 'unavailable' ? state.problem === 'map-version' ? 'version' : 'unavailable'
    : state.phase === 'disposed' ? 'unavailable' : state.phase === 'ready' ? 'ready' : 'loading'
  return Object.freeze({ release, kind })
}
