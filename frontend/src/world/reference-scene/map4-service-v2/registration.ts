// Deterministic registration of density ART 5d2d0d34536fa8bf9314c67e5a3b8390e9baf553 (canonical 7acfac3a).
import resources from './resources.json'
import { compositionVersion } from './semantics'
import type { CompositionRegistration } from './contract'

export const compositionRegistration = { ...resources, version: compositionVersion,
  presentationHash: 'sha256:6efb8ec1098233533e8bf7013d9e4925e4afa6af515b506412dee87c33327683' } as CompositionRegistration
