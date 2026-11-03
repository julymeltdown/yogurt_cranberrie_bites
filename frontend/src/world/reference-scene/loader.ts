import { loadMap, type LoadedMap } from '../map/resources'
import { validateManifests } from '../map/manifest'
import { requireReferenceScene, validateReferenceMetadata } from './model'

/** Optional exact-release loader hook. Root must first admit coherent pins; this grants no bootstrap authority. */
export async function loadReferenceScene(input: Parameters<typeof loadMap>[0], signal: AbortSignal): Promise<LoadedMap> {
  const metadata = await validateManifests(input.mapText, input.themeText, input.pin)
  validateReferenceMetadata(metadata) // Reject stale/cross-release candidates before any resource or decode.
  const loaded = await loadMap(input, signal)
  try { requireReferenceScene(loaded); if (signal.aborted) throw Error('Reference scene cancelled'); return loaded }
  catch (error) { loaded.dispose(); throw error }
}
