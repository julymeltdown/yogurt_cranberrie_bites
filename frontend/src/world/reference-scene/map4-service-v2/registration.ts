// Exact published roof/street ART da33467cd084ecba071244a1939b6217f4b0de02 (canonical efa9d956).
import resources from './resources.json'
import { compositionVersion } from './semantics'
import type { CompositionRegistration } from './contract'

export const compositionRegistration = { ...resources, version: compositionVersion,
  presentationHash: 'sha256:348f1969deeaca05c16087f7ae5c7060f49c3c8fbd934ffc7aef195c937f2d24' } as CompositionRegistration
