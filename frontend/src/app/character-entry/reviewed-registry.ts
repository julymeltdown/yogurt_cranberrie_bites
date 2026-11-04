import type { PrivacyScope } from '../../shared/query/privacy'
import type { ShellBootstrap } from '../ports'
import { createReviewedProductionAuthority, type ReviewedProductionAuthority, type PublicReviewedConfig, type ReviewedProductionAuth } from '../../entities/avatar/runtime/motion-adoption/reviewed-production'
import { decodeReviewedMotionManifest, type ReviewedMotionPin } from '../../entities/avatar/runtime/motion-adoption/reviewed-manifest'
import { createMotionRuntime, type MotionRuntime } from '../../entities/avatar/runtime/motion-adoption/runtime'

const same = (a: PrivacyScope, b: PrivacyScope) => a.generation === b.generation && a.userId === b.userId
  && a.eventId === b.eventId && a.policyGeneration === b.policyGeneration && a.gate === b.gate
const key = (pin: ReviewedMotionPin) => JSON.stringify([pin.manifest_id, pin.manifest_version, pin.content_hash])
const record = (value: unknown): value is Record<string, unknown> => value !== null && typeof value === 'object' && !Array.isArray(value)
export interface ReviewedCharacterOwner {
  readonly kind: 'reviewed-motion'
  readonly scope: PrivacyScope
  readonly authority: ReviewedProductionAuthority
  readonly runtime: MotionRuntime<PublicReviewedConfig>
  isCurrent(): boolean
  dispose(): void
}
/** Registry lifetime owns admission and artwork. Views borrow; their abort signals never cancel admission. */
export function createReviewedCharacterRegistry(auth: ReviewedProductionAuth, origin: string) {
  let closed = false
  type Entry = { scope: PrivacyScope; key: string; controller: AbortController;
    owner: ReviewedCharacterOwner | null; flight: Promise<ReviewedCharacterOwner | null> }
  let entry: Entry | null = null
  const current = (scope: PrivacyScope) => !closed && auth.session.isCurrent(scope) && auth.privacy.isCurrent(scope)
    && same(scope, auth.privacy.capture())
  function clear() {
    const previous = entry; entry = null
    previous?.controller.abort()
    previous?.owner?.dispose()
  }
  const unregister = auth.privacy.registerCleanup(clear)
  return {
    read(scope: PrivacyScope, pin: ReviewedMotionPin): ReviewedCharacterOwner | null {
      return current(scope) && entry && same(entry.scope, scope) && entry.key === key(pin)
        && entry.owner?.isCurrent() ? entry.owner : null
    },
    admit(scope: PrivacyScope, bootstrap: ShellBootstrap): Promise<ReviewedCharacterOwner | null> {
      const manifest = bootstrap.document?.avatar_manifest
      if (!current(scope) || scope.gate !== 'open' || bootstrap.event.eventId !== scope.eventId
        || bootstrap.participant.eventId !== scope.eventId || bootstrap.participant.userId !== scope.userId
        || !record(manifest) || manifest.contract_version !== '2.0.0' || manifest.representation !== 'reviewed_selected_source_motion'
        || typeof manifest.manifest_id !== 'string' || !Number.isSafeInteger(manifest.manifest_version)
        || typeof manifest.content_hash !== 'string') { clear(); return Promise.resolve(null) }
      const pin = Object.freeze({ manifest_id: manifest.manifest_id, manifest_version: Number(manifest.manifest_version), content_hash: manifest.content_hash })
      const identity = key(pin)
      if (entry && same(entry.scope, scope) && entry.key === identity && !entry.controller.signal.aborted
        && (!entry.owner || entry.owner.isCurrent())) return entry.flight
      clear()
      const owned: Entry = { scope: Object.freeze({ ...scope }), key: identity, controller: new AbortController(), owner: null,
        flight: Promise.resolve(null) }
      entry = owned
      const live = () => entry === owned && current(owned.scope) && !owned.controller.signal.aborted
      owned.flight = (async () => {
        let authority: ReviewedProductionAuthority | null = null
        let runtime: MotionRuntime<PublicReviewedConfig> | null = null
        try {
          // Reject an unreviewed shell document before any protected request.
          await decodeReviewedMotionManifest(manifest, pin)
          if (!live()) return null
          authority = await createReviewedProductionAuthority({ auth, scope: owned.scope, pin, signal: owned.controller.signal })
          if (!live()) { authority.dispose(); return null }
          runtime = createMotionRuntime({ authority, scope: owned.scope, privacy: auth.privacy, origin })
          const shared = runtime, grant = authority
          let disposed = false
          const owner: ReviewedCharacterOwner = Object.freeze({ kind: 'reviewed-motion' as const, scope: owned.scope,
            authority: grant, runtime: shared, isCurrent: () => !disposed && live() && grant.isCurrent() && shared.isCurrent(),
            dispose() { if (disposed) return; disposed = true; shared.dispose(); grant.dispose() },
          })
          if (!owner.isCurrent()) { owner.dispose(); return null }
          owned.owner = owner
          return owner
        } catch {
          runtime?.dispose(); authority?.dispose()
          if (entry === owned) { entry = null; owned.controller.abort() }
          return null
        }
      })()
      return owned.flight
    },
    clear,
    dispose() { if (closed) return; closed = true; unregister(); clear() },
  }
}
