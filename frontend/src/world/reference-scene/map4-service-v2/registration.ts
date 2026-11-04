// Deterministic registration of corrected ART 97d9d3f40e0d17ab536ac696bdb2140093225f6a.
import resources from './resources.json'
import { compositionVersion } from './semantics'
import type { CompositionRegistration } from './contract'

export const compositionRegistration = { ...resources, version: compositionVersion,
  presentationHash: 'sha256:22e74b17f9753102e2f8a1df844681447fd82a0fde1b4f5ee68f9fd1f213ddc1' } as CompositionRegistration
