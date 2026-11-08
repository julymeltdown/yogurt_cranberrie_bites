import type { SessionController } from '../../features/auth/model/session'
import type { PrivacyScope } from '../../shared/query/privacy'
import type { AvatarConfig } from '../avatar/whole-admission'
import { requireMotion } from './art'
import { decodeReviewedMotionManifest, reviewedMotionAuthorityOperations, type ReviewedMotionPin } from './reviewed-manifest'
import type { MotionAuthorityOperations, ProfileAvatarScope } from './runtime'

export type PublicReviewedConfig = Extract<AvatarConfig, { contract_version: '2.0.0' }>
export type ReviewedProductionAuth = Pick<SessionController, 'session' | 'privacy' | 'request'>
const minted = new WeakMap<object, ReviewedProductionAuth>()
const proof = Symbol('authenticated reviewed motion authority')
export interface ReviewedProductionLifetime {
  readonly qualification: 'REVIEWED_PRODUCTION'
  readonly [proof]: true
  readonly scope: ProfileAvatarScope
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

/**
 * WP-08 T-704: read the reviewed motion manifest through the one canonical Auth
 * owner, scoped to the *profile*.
 *
 * The concert version fetched `/api/concert/events/{eventId}/bootstrap` to learn
 * the manifest and whether mutations were available. A profile avatar has no
 * event, so this version takes the already pinned manifest document from the
 * caller (the profile shell's own release read) and never issues an
 * event-scoped request. `canSave` is the canonical session's ability to write,
 * not an event capability.
 */
export async function createReviewedProductionAuthority(options: {
  readonly auth: ReviewedProductionAuth
  readonly scope: ProfileAvatarScope
  readonly pin: ReviewedMotionPin
  /** The pinned reviewed manifest document, already read by the profile shell. */
  readonly manifest: unknown
  /** Whether the canonical session may mutate; the profile routes decide the rest. */
  readonly canSave: boolean
  readonly signal: AbortSignal
}): Promise<ReviewedProductionAuthority> {
  const { auth, signal } = options, scope = Object.freeze({ ...options.scope }), pin = Object.freeze({ ...options.pin })
  requireMotion(typeof scope.userId === 'string' && scope.userId.length > 0 && scope.gate === 'open')
  const lifetime = new AbortController(), listeners = new Set<() => void>()
  let disposed = false, unregister = () => {}
  const current = () => !disposed && !lifetime.signal.aborted && auth.session.isCurrent(scope as PrivacyScope) && auth.privacy.isCurrent(scope as PrivacyScope)
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
    // The canonical HTTP decoder is synchronous; keep async bundle hashing inside this fenced owner.
    const stage = await decodeReviewedMotionManifest(options.manifest, pin)
    requireMotion(current() && !signal.aborted)
    const operations = reviewedMotionAuthorityOperations(stage)
    const authority: ReviewedProductionAuthority = Object.freeze({ ...operations,
      qualification: 'REVIEWED_PRODUCTION' as const, [proof]: true as const, scope, signal: lifetime.signal,
      canSave: options.canSave === true,
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
