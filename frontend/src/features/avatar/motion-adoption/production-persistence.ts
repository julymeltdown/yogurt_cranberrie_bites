import { assertReviewedProductionAuthority, type PublicReviewedConfig, type ReviewedProductionAuth,
  type ReviewedProductionAuthority } from '../../../entities/avatar/runtime/motion-adoption/reviewed-production'
import { decodeAvatarState } from '../../../entities/avatar/whole-admission'
import { equal } from '../../auth/model/contract'
import { decodeParticipant } from '../../me/model/contract'
import { requireMotion } from '../../../entities/avatar/runtime/motion-adoption/art'
import type { MotionAvatarState, MotionPersistence } from './controller'

/** Borrow the canonical Auth and admitted runtime authority. No token, HTTP client or private cache is created. */
export function createReviewedMotionPersistence({ auth, authority }: {
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
    const response = await auth.privacy.run(scope, ({ signal: active }) => auth.request({
      path: `/api/concert/events/${scope.eventId}/me${submitted ? '/avatar' : ''}`,
      method: submitted ? 'PUT' : 'GET', access: 'protected', errorFamily: 'core', signal: active,
      ...(submitted ? { body: { json: { expected_revision: expectedRevision!, config: submitted } } } : {}),
      decode: value => {
        if (submitted) {
          const saved = decode(value)
          requireMotion(saved.revision === expectedRevision! + 1 && equal(saved.config, submitted))
          return saved
        }
        return decode(decodeParticipant(value, scope).profile.avatar)
      },
    }), combined)
    current(combined)
    requireMotion(response.scope.generation === scope.generation && response.scope.userId === scope.userId && response.scope.eventId === scope.eventId)
    return accept(decode(response.data))
  }
  return Object.freeze({ read: (signal: AbortSignal) => request(signal),
    save: (revision: number, config: PublicReviewedConfig, signal: AbortSignal) => request(signal, config, revision),
    dispose() { dispose(); remove() },
  })
}
