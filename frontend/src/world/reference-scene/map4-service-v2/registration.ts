// Deterministic registration of corrected ART 8fcd1287496aedbde12ef0d2e426855f8f8ec4d6.
import resources from './resources.json'
import { compositionVersion } from './semantics'
import type { CompositionRegistration } from './contract'

export const compositionRegistration = { ...resources, version: compositionVersion,
  presentationHash: 'sha256:9fcddca91030aaff0f3c3efaa1018fdeddb4708c56c9eea80024deb0bee9f522' } as CompositionRegistration
