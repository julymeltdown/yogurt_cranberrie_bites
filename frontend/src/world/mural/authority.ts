import type { WorldReplicationRelease } from '../composition'
import type { PrivacyPort } from '../../features/mural/editor/contract'
import { bindingSchema } from '../../features/mural/sync/contract'
import type { MuralAuthorityPort } from '../../features/mural/sync/contract'
/** Existing FE16/FE17 release only. No admission, transport ownership, engine or socket creation. */
export function createWorldMuralAuthority({ privacy, world, writable = () => false }: {
  privacy: PrivacyPort; world: Pick<WorldReplicationRelease, 'read' | 'subscribe'>; writable?: () => boolean
}): MuralAuthorityPort {
  const scope = Object.freeze({ ...privacy.capture() })
  return {
    read() {
      if (!privacy.isCurrent(scope)) return null
      const view = world.read(), baseline = view?.baseline
      if (!view || !baseline || view.phase !== 'live' || !view.visible || view.scope.userId !== scope.userId
        || view.scope.eventId !== scope.eventId || view.scope.generation !== scope.generation || baseline.eventId !== scope.eventId) return null
      const result = bindingSchema.safeParse({ eventId: baseline.eventId, roomId: baseline.roomId, roomEpoch: baseline.roomEpoch, streamId: baseline.streamId })
      return result.success ? Object.freeze({ binding: Object.freeze(result.data), writable: writable() === true }) : null
    },
    subscribe(listener) {
      const stopWorld = world.subscribe(listener), stopPrivacy = privacy.registerCleanup(listener)
      return () => { try { stopWorld() } finally { stopPrivacy() } }
    },
  }
}
