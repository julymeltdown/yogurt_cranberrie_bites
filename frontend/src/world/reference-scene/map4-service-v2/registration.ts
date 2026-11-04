// Deterministic registration of corrected ART 77a4d405f65f82dc14edda98d2447d5f0ed302b7.
import resources from './resources.json'
import { compositionVersion } from './semantics'
import type { CompositionRegistration } from './contract'

export const compositionRegistration = { ...resources, version: compositionVersion,
  presentationHash: 'sha256:f7a89dc6d176f2414f845139de39607ba75830a54e5e623f2de2cd6fc76bed17' } as CompositionRegistration
