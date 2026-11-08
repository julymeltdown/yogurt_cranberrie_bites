import { decodeAvatarState } from '../../../entities/avatar/whole-admission'
import { requireMotion } from '../../../entities/emo-avatar/art'
import {
  assertReviewedProductionAuthority,
  type PublicReviewedConfig,
  type ReviewedProductionAuth,
  type ReviewedProductionAuthority,
} from '../../../entities/emo-avatar/reviewed-production'
import type { PrivacyScope } from '../../../shared/query/privacy'
import { equal } from '../../auth/model/contract'
import type { MotionAvatarState, MotionPersistence } from '../model/controller'

/** The one profile avatar path. There is no event segment and no participant. */
export const PROFILE_AVATAR_PATH = '/api/me/avatar'

/**
 * WP-08 T-704: the profile avatar persistence adapter.
 *
 * This is the concert `createReviewedMotionPersistence` re-pointed:
 *
 * * `GET /api/me/avatar` reads the owner's avatar; `PUT /api/me/avatar` writes
 *   it. The concert path `/api/concert/events/{eventId}/me/avatar` is not used,
 *   no event id is fabricated, and there is no test-only authority in
 *   production.
 * * The response-consistency rules are kept: a save must return exactly the
 *   next revision and the submitted config, and a read must never go backwards.
 * * Every request is bound to the profile scope, so an account switch aborts the
 *   flight through the canonical privacy owner rather than resolving into the
 *   new account.
 */
export function createProfileAvatarPersistence({ auth, authority }: {
  readonly auth: ReviewedProductionAuth
  readonly authority: ReviewedProductionAuthority
}): MotionPersistence<PublicReviewedConfig> & { dispose(): void } {
  assertReviewedProductionAuthority(authority, auth)
  const scope = authority.scope, lifetime = new AbortController()
  let disposed = false, observed: MotionAvatarState<PublicReviewedConfig> | null = null
  const dispose = () => { if (disposed) return; disposed = true; observed = null; lifetime.abort() }
  const remove = authority.onInvalidate(dispose)
  const current = (signal: AbortSignal) => {
    requireMotion(!disposed && !signal.aborted)
    assertReviewedProductionAuthority(authority, auth)
  }
  function decode(value: unknown): MotionAvatarState<PublicReviewedConfig> {
    const avatar = decodeAvatarState(value)
    return Object.freeze({ revision: avatar.revision, config: authority.decodeConfig(avatar.config) })
  }
  function accept(avatar: MotionAvatarState<PublicReviewedConfig>) {
    requireMotion(!observed || avatar.revision > observed.revision || avatar.revision === observed.revision && equal(avatar.config, observed.config))
    observed = avatar; return avatar
  }
  async function request(signal: AbortSignal, config?: PublicReviewedConfig, expectedRevision?: number) {
    current(signal)
    const combined = AbortSignal.any([signal, lifetime.signal, authority.signal])
    const submitted = config === undefined ? undefined : authority.decodeConfig(config)
    if (submitted) requireMotion(authority.canSave && Number.isSafeInteger(expectedRevision) && expectedRevision! >= 1 && expectedRevision! < Number.MAX_SAFE_INTEGER
      && (!observed || expectedRevision === observed.revision))
    const response = await auth.privacy.run(scope as PrivacyScope, ({ signal: active }) => auth.request({
      path: PROFILE_AVATAR_PATH,
      method: submitted ? 'PUT' : 'GET', access: 'protected', errorFamily: 'core', signal: active,
      ...(submitted ? { body: { json: { expected_revision: expectedRevision!, config: submitted } } } : {}),
      decode: value => {
        if (submitted) {
          const saved = decode(value)
          requireMotion(saved.revision === expectedRevision! + 1 && equal(saved.config, submitted))
          return saved
        }
        return decode(value)
      },
    }), combined)
    current(combined)
    // The response must belong to the same account and generation this owner was
    // created for. An event is not part of the comparison because the avatar has
    // none.
    requireMotion(response.scope.generation === scope.generation && response.scope.userId === scope.userId)
    return accept(decode(response.data))
  }
  return Object.freeze({ read: (signal: AbortSignal) => request(signal),
    save: (revision: number, config: PublicReviewedConfig, signal: AbortSignal) => request(signal, config, revision),
    dispose() { dispose(); remove() },
  })
}
