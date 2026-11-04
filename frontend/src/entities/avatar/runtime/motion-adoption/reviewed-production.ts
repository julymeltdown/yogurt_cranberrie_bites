import type { SessionController } from '../../../../features/auth/model/session'
import { decodeBootstrap } from '../../../../features/auth/model/bootstrap'
import { canonicalUuid, decodeContract, object } from '../../../../features/auth/model/contract'
import type { PrivacyScope } from '../../../../shared/query/privacy'
import type { AvatarConfig } from '../../whole-admission'
import { validateConfig } from '../../whole-admission'
import { requireMotion } from './art'
import { decodeReviewedMotionManifest, reviewedMotionAuthorityOperations, type ReviewedMotionPin } from './reviewed-manifest'
import type { MotionAuthorityOperations } from './runtime'

export type PublicReviewedConfig = Extract<AvatarConfig, { contract_version: '2.0.0' }>
export type ReviewedProductionAuth = Pick<SessionController, 'session' | 'privacy' | 'request'>
const minted = new WeakMap<object, ReviewedProductionAuth>()
const proof = Symbol('authenticated reviewed motion authority')
export interface ReviewedProductionLifetime {
  readonly qualification: 'REVIEWED_PRODUCTION'
  readonly [proof]: true
  readonly scope: PrivacyScope
  readonly signal: AbortSignal
  readonly canSave: boolean
  isCurrent(): boolean
  onInvalidate(listener: () => void): () => void
  dispose(): void
}
export type ReviewedProductionAuthority = MotionAuthorityOperations<PublicReviewedConfig> & ReviewedProductionLifetime

/** A stage, copied object or qualification string cannot manufacture a public grant. */
export function isReviewedProductionAuthority(value: unknown): value is ReviewedProductionAuthority {
  return typeof value === 'object' && value !== null && minted.has(value)
}
export function assertReviewedProductionAuthority(value: unknown, auth?: ReviewedProductionAuth): asserts value is ReviewedProductionAuthority {
  requireMotion(isReviewedProductionAuthority(value) && value.isCurrent() && (!auth || minted.get(value) === auth))
}

/** Read the actual C03 public endpoint through the one canonical Auth owner.
 * This does not activate a server release or convert a legacy appearance.
 */
export async function createReviewedProductionAuthority(options: {
  readonly auth: ReviewedProductionAuth
  readonly scope: PrivacyScope
  readonly pin: ReviewedMotionPin
  readonly signal: AbortSignal
}): Promise<ReviewedProductionAuthority> {
  const { auth, signal } = options, scope = Object.freeze({ ...options.scope }), pin = Object.freeze({ ...options.pin })
  requireMotion(canonicalUuid(scope.userId) && canonicalUuid(scope.eventId) && scope.gate === 'open')
  const activeScope = { generation: scope.generation, userId: scope.userId, eventId: scope.eventId }
  const lifetime = new AbortController(), listeners = new Set<() => void>()
  let disposed = false, unregister = () => {}
  const current = () => !disposed && !lifetime.signal.aborted && auth.session.isCurrent(scope) && auth.privacy.isCurrent(scope)
  function dispose() {
    if (disposed) return
    disposed = true; lifetime.abort(); unregister(); signal.removeEventListener('abort', dispose)
    for (const listener of Array.from(listeners)) { try { listener() } catch { /* Withdraw every consumer. */ } }
    listeners.clear()
  }
  signal.addEventListener('abort', dispose, { once: true })
  try {
    requireMotion(current() && !signal.aborted)
    unregister = auth.privacy.registerCleanup(dispose)
    const response = await auth.privacy.run(scope, ({ signal }) => auth.request({
      path: `/api/concert/events/${scope.eventId}/bootstrap`, method: 'GET', access: 'protected', errorFamily: 'core', signal,
      decode: value => structuredClone(decodeContract('Bootstrap', value)),
    }), lifetime.signal)
    requireMotion(current() && !signal.aborted && response.scope.generation === scope.generation
      && response.scope.userId === scope.userId && response.scope.eventId === scope.eventId)
    // The canonical HTTP decoder is synchronous; keep async bundle hashing inside this fenced owner.
    const bootstrap = await decodeBootstrap(response.data, activeScope)
    requireMotion(current() && !signal.aborted)
    const document = object(bootstrap.document)
    const stage = await decodeReviewedMotionManifest(document.avatar_manifest, pin)
    const avatar = object(document.avatar)
    validateConfig(avatar.config, stage.manifest)
    requireMotion(current() && !signal.aborted)
    const operations = reviewedMotionAuthorityOperations(stage)
    const authority: ReviewedProductionAuthority = Object.freeze({ ...operations,
      qualification: 'REVIEWED_PRODUCTION' as const, [proof]: true as const, scope, signal: lifetime.signal,
      canSave: object(object(document.capabilities).mutations).available === true,
      isCurrent: current,
      onInvalidate(listener: () => void) {
        if (!current()) { listener(); return () => {} }
        listeners.add(listener); return () => { listeners.delete(listener) }
      }, dispose,
    })
    minted.set(authority, auth)
    return authority
  } catch (error) { dispose(); throw error }
}
