import type { PrivacyScope } from '../../../shared/query/privacy'
import type { MotionRuntime } from '../../../entities/avatar/runtime/motion-adoption/runtime'
import { requireMotion } from '../../../entities/avatar/runtime/motion-adoption/art'
import { assertReviewedProductionAuthority, type PublicReviewedConfig, type ReviewedProductionAuth,
  type ReviewedProductionAuthority } from '../../../entities/avatar/runtime/motion-adoption/reviewed-production'
import { createMotionEditorController, type MotionAvatarState } from './controller'
import { createReviewedMotionPersistence } from './production-persistence'

const sameScope = (left: PrivacyScope, right: PrivacyScope) => left.userId === right.userId && left.eventId === right.eventId
  && left.generation === right.generation && left.policyGeneration === right.policyGeneration && left.gate === right.gate

/** Account owns this draft/controller. Views borrow it; the registry owns the shared runtime/authority. */
export function createReviewedMotionEditorOwner(options: {
  readonly auth: ReviewedProductionAuth
  readonly scope: PrivacyScope
  readonly authority: ReviewedProductionAuthority
  readonly runtime: MotionRuntime<PublicReviewedConfig>
  readonly onCommitted?: (avatar: MotionAvatarState<PublicReviewedConfig>, scope: PrivacyScope) => void
}) {
  const { auth, authority, runtime } = options, scope = Object.freeze({ ...options.scope })
  assertReviewedProductionAuthority(authority, auth)
  requireMotion(runtime.authority === authority && sameScope(scope, authority.scope) && sameScope(scope, runtime.scope)
    && runtime.isCurrent() && auth.session.isCurrent(scope) && auth.privacy.isCurrent(scope))
  const persistence = createReviewedMotionPersistence({ auth, authority })
  let disposed = false
  const current = () => !disposed && authority.isCurrent() && runtime.isCurrent()
    && auth.session.isCurrent(scope) && auth.privacy.isCurrent(scope)
  const controller = Object.freeze({ ...createMotionEditorController({ runtime, persistence, privacy: auth.privacy,
    onCommitted(avatar, committedScope) {
      if (current() && sameScope(scope, committedScope)) options.onCommitted?.(avatar, scope)
    },
  }), lifetime: 'owner' as const })
  let unregister = () => {}
  function dispose() {
    if (disposed) return
    disposed = true; unregister(); controller.dispose(); persistence.dispose()
  }
  unregister = authority.onInvalidate(dispose)
  if (!current()) dispose()
  return Object.freeze({ kind: 'reviewed-motion-editor-owner' as const, scope, controller, isCurrent: current,
    start() { if (current()) controller.start(); else dispose() }, dispose,
  })
}
export type ReviewedMotionEditorOwner = ReturnType<typeof createReviewedMotionEditorOwner>
