// Deterministic registration of corrected ART 94c47a8b3219efc76a4be9646014cfeaad2f1957.
import resources from './resources.json'
import { compositionVersion } from './semantics'
import type { CompositionRegistration } from './contract'

export const compositionRegistration = { ...resources, version: compositionVersion,
  presentationHash: 'sha256:4922af8c4761014a2dd672ec52610ab9abd394be0bcc0b29e7780421167600ca' } as CompositionRegistration
