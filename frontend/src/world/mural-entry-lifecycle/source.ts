import type { PrivacyPort } from '../../features/mural/editor/contract'
import type { MuralEntryRelease } from '../../app/mural-entry/ports'
import type { WorldReplicationRelease } from '../composition'
import type { PreparedWorldRelease } from '../composition/release'
import type { IncumbentMuralScene } from '../runtime'
import { createMuralEntryLifecycle, type MuralEntryQualification } from './index'

export interface MuralQualificationSource {
  read(): MuralEntryQualification | null
  subscribe(listener: () => void): () => void
}
export interface MuralQualificationContext {
  readonly scope: MuralEntryRelease['scope']
  readonly signal: AbortSignal
  readonly scene: IncumbentMuralScene
  readonly world: Pick<WorldReplicationRelease, 'read' | 'subscribe'>
  readonly release: PreparedWorldRelease
}
/** A supplied in-process qualification source, not a server authorization implementation.
 * Omit unless independent anchor/admission/policy inputs are available.
 */
export type MuralQualificationFactory = (context: MuralQualificationContext) => MuralQualificationSource | null
export interface MuralEntryReleaseSource {
  read(): MuralEntryRelease | null
  subscribe(listener: () => void): () => void
}

/** Connect a borrowed explicit source to the lifecycle; never retry a withdrawn qualification. */
export function attachMuralEntrySource({ privacy, context, factory }: {
  readonly privacy: PrivacyPort
  readonly context: MuralQualificationContext
  readonly factory: MuralQualificationFactory
}): MuralEntryReleaseSource & { dispose(): void } {
  const world = Object.freeze({ read: context.world.read.bind(context.world), subscribe: context.world.subscribe.bind(context.world) })
  const lifecycle = createMuralEntryLifecycle({ privacy, world })
  let stopped = false, stopSource = () => {}, stopPrivacy = () => {}
  const dispose = () => {
    if (stopped) return
    stopped = true; lifecycle.dispose()
    try { stopSource() } catch { /* Independent source cleanup. */ }
    try { stopPrivacy() } catch { /* Independent privacy cleanup. */ }
    context.signal.removeEventListener('abort', dispose)
  }
  try {
    context.signal.addEventListener('abort', dispose, { once: true })
    stopPrivacy = privacy.registerCleanup(dispose)
    if (context.signal.aborted || !privacy.isCurrent(context.scope)) dispose()
    if (!stopped) {
      const source = factory(Object.freeze({ ...context, world, scope: Object.freeze({ ...context.scope }) }))
      if (source) {
        const read = source.read.bind(source)
        const changed = () => {
          if (stopped) return
          try { lifecycle.bind(read()) } catch { dispose() }
        }
        stopSource = source.subscribe(changed)
        if (stopped) stopSource(); else changed()
      }
    }
  } catch { dispose() }
  return Object.freeze({ read: lifecycle.read, subscribe: lifecycle.subscribe, dispose })
}
