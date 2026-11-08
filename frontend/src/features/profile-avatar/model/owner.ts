import type { PrivacyScope } from '../../../shared/query/privacy'
import { createProfileAvatarPersistence } from '../api/persistence'
import type { MotionRuntime } from '../../../entities/emo-avatar/runtime'
import { requireMotion } from '../../../entities/emo-avatar/art'
import {
  assertReviewedProductionAuthority,
  type PublicReviewedConfig,
  type ReviewedProductionAuth,
  type ReviewedProductionAuthority,
} from '../../../entities/emo-avatar/reviewed-production'
import { createProfileAvatarEditorController, type MotionAvatarState } from '../model/controller'

/** Two scopes belong to the same account when the account and generation match. No event is compared. */
const sameAccount = (left: PrivacyScope, right: PrivacyScope) => left.userId === right.userId
  && left.generation === right.generation && left.policyGeneration === right.policyGeneration && left.gate === right.gate

/**
 * WP-08 T-704/T-705: the account owns the profile avatar draft/controller.
 *
 * Views borrow it; the registry owns the shared runtime and authority. The
 * owner is bound to one account, and it is disposed the moment that account is
 * no longer current — so a switched account can never start, save, or render
 * the previous account's editor.
 */
export function createProfileAvatarEditorOwner(options: {
  readonly auth: ReviewedProductionAuth
  readonly scope: PrivacyScope
  readonly authority: ReviewedProductionAuthority
  readonly runtime: MotionRuntime<PublicReviewedConfig>
  readonly onCommitted?: (avatar: MotionAvatarState<PublicReviewedConfig>, scope: PrivacyScope) => void
}) {
  const { auth, authority, runtime } = options, scope = Object.freeze({ ...options.scope })
  assertReviewedProductionAuthority(authority, auth)
  requireMotion(runtime.authority === authority && sameAccount(scope, authority.scope as PrivacyScope) && sameAccount(scope, runtime.scope as PrivacyScope)
    && runtime.isCurrent() && auth.session.isCurrent(scope) && auth.privacy.isCurrent(scope))
  const persistence = createProfileAvatarPersistence({ auth, authority })
  let disposed = false
  const current = () => !disposed && authority.isCurrent() && runtime.isCurrent()
    && auth.session.isCurrent(scope) && auth.privacy.isCurrent(scope)
  const controller = Object.freeze({ ...createProfileAvatarEditorController({ runtime, persistence, privacy: auth.privacy,
    onCommitted(avatar, committedScope) {
      if (current() && sameAccount(scope, committedScope)) options.onCommitted?.(avatar, scope)
    },
  }), lifetime: 'owner' as const })
  let unregister = () => {}
  function dispose() {
    if (disposed) return
    disposed = true; unregister(); controller.dispose(); persistence.dispose()
  }
  unregister = authority.onInvalidate(dispose)
  if (!current()) dispose()
  return Object.freeze({ kind: 'profile-avatar-editor-owner' as const, scope, controller, isCurrent: current,
    start() { if (current()) controller.start(); else dispose() }, dispose,
  })
}
export type ProfileAvatarEditorOwner = ReturnType<typeof createProfileAvatarEditorOwner>
