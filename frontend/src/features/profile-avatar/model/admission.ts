import type { SessionController } from '../../auth/model/session'
import type { PrivacyScope } from '../../../shared/query/privacy'
import { createMotionRuntime } from '../../../entities/emo-avatar/runtime'
import { decodeReviewedMotionManifest, type ReviewedMotionPin } from '../../../entities/emo-avatar/reviewed-manifest'
import { createReviewedProductionAuthority, type PublicReviewedConfig, type ReviewedProductionAuth } from '../../../entities/emo-avatar/reviewed-production'
import { createProfileAvatarEditorOwner, type ProfileAvatarEditorOwner } from '../model/owner'

const record = (value: unknown): value is Record<string, unknown> => value !== null && typeof value === 'object' && !Array.isArray(value)
const same = (a: PrivacyScope, b: PrivacyScope) => a.generation === b.generation && a.userId === b.userId
  && a.policyGeneration === b.policyGeneration && a.gate === b.gate

/**
 * WP-08 T-704: admit the profile avatar from the shell's own release document.
 *
 * The concert registry required `bootstrap.event.eventId === scope.eventId` and
 * `bootstrap.participant.eventId === scope.eventId` before it would admit a
 * reviewed character — an event gate, which is exactly what a profile avatar
 * must not have. This admission checks the *account* instead: the release must
 * belong to the authenticated account, the manifest must be the pinned reviewed
 * manifest, and the account scope must be current. There is no event in the
 * predicate, and `origin` is the only other input.
 */
export async function admitProfileAvatar(options: {
  readonly auth: ReviewedProductionAuth
  readonly scope: PrivacyScope
  readonly manifest: unknown
  readonly canSave: boolean
  readonly origin: string
  readonly signal: AbortSignal
}): Promise<ProfileAvatarEditorOwner | null> {
  const { auth, scope, signal } = options
  if (scope.gate !== 'open' || !auth.session.isCurrent(scope) || !auth.privacy.isCurrent(scope)) return null
  const manifest = options.manifest
  if (!record(manifest) || manifest.contract_version !== '2.0.0'
    || manifest.representation !== 'reviewed_selected_source_motion'
    || typeof manifest.manifest_id !== 'string' || !Number.isSafeInteger(manifest.manifest_version)
    || typeof manifest.content_hash !== 'string') return null
  const pin: ReviewedMotionPin = Object.freeze({
    manifest_id: manifest.manifest_id,
    manifest_version: Number(manifest.manifest_version),
    content_hash: manifest.content_hash,
  })
  let authority: Awaited<ReturnType<typeof createReviewedProductionAuthority>> | null = null
  let runtime: ReturnType<typeof createMotionRuntime<PublicReviewedConfig>> | null = null
  try {
    // Reject an unreviewed document before any protected request or art load.
    await decodeReviewedMotionManifest(manifest, pin)
    if (signal.aborted || !auth.privacy.isCurrent(scope)) return null
    authority = await createReviewedProductionAuthority({
      auth, scope, pin, manifest, canSave: options.canSave, signal,
    })
    if (signal.aborted || !auth.privacy.isCurrent(scope)) { authority.dispose(); return null }
    runtime = createMotionRuntime({ authority, scope, privacy: auth.privacy, origin: options.origin })
    const owner = createProfileAvatarEditorOwner({ auth, scope, authority, runtime })
    if (!owner.isCurrent()) { owner.dispose(); runtime.dispose(); authority.dispose(); return null }
    return owner
  } catch {
    runtime?.dispose(); authority?.dispose(); return null
  }
}

/** Whether a shell release document describes the reviewed motion character. */
export function isReviewedMotionRelease(manifest: unknown): boolean {
  return record(manifest) && manifest.contract_version === '2.0.0'
    && manifest.representation === 'reviewed_selected_source_motion'
}

export type { SessionController }
export { same as sameProfileScope }
