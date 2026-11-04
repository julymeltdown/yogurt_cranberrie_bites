// Deterministic registration of corrected ART af298e3246096c4b7a3ae5c7e439f5f33988a6be.
import resources from './resources.json'
import { compositionVersion } from './semantics'
import type { CompositionRegistration } from './contract'

export const compositionRegistration = { ...resources, version: compositionVersion,
  presentationHash: 'sha256:d8437dda63dd5f0cb554bcfcc1c267782db6630fab39c541291bb3c368d154b6' } as CompositionRegistration
