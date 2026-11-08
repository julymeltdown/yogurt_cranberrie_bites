import { AvatarError, createReviewedAtlasLeases } from '../../entities/avatar/runtime'
import { createAvatarEditorController, createReviewedAvatarPreview } from '../../features/avatar'
import { createMeController } from '../../features/me'
import type { SessionController } from '../../features/auth/model/session'
import type { PrivacyScope } from '../../shared/query/privacy'
import type { ShellBootstrap } from '../ports'
import { thumbnailSource, type AccountAssetsPort } from './assets'
import type { WholeCharacterRuntime } from '../../entities/avatar/whole-character'
import { createWholeCharacterEditorController } from '../../features/avatar/whole-character/controller'
import type { ReviewedCharacterOwner } from '../character-entry/reviewed-registry'
import { createReviewedMotionEditorOwner } from '../../features/avatar/motion-adoption/production-owner'
import { admitProfileAvatar, isReviewedMotionRelease, type ProfileAvatarEditorOwner } from '../../features/profile-avatar'

export interface AccountOwnerOptions {
  readonly auth: SessionController
  readonly bootstrap: ShellBootstrap
  readonly scope: PrivacyScope
  readonly assets?: AccountAssetsPort | undefined
  readonly characters?: WholeCharacterRuntime | null
  readonly reviewed?: ReviewedCharacterOwner | null
}
/** Own controllers only. Neither creates a session, QueryClient, Phaser game nor world connection. */
export function createAccountOwner({ auth, bootstrap, scope, assets, characters = null, reviewed = null }: AccountOwnerOptions) {
  if (!auth.privacy.isCurrent(scope) || bootstrap.participant.userId !== scope.userId || bootstrap.participant.eventId !== scope.eventId
    || bootstrap.event.eventId !== scope.eventId) throw new AvatarError('unavailable')
  if (characters && (!characters.isCurrent() || characters.scope.userId !== scope.userId || characters.scope.eventId !== scope.eventId
    || characters.scope.generation !== scope.generation || characters.scope.policyGeneration !== scope.policyGeneration || characters.scope.gate !== scope.gate)) throw new AvatarError('unavailable')
  if (reviewed && (characters || !reviewed.isCurrent() || reviewed.scope.userId !== scope.userId || reviewed.scope.eventId !== scope.eventId
    || reviewed.scope.generation !== scope.generation || reviewed.scope.policyGeneration !== scope.policyGeneration || reviewed.scope.gate !== scope.gate)) throw new AvatarError('unavailable')
  let disposed = false
  const leases = createReviewedAtlasLeases({ isCurrent: () => !disposed && auth.privacy.isCurrent(scope) })
  const source = thumbnailSource(bootstrap, scope, assets, leases), me = createMeController(auth)
  const avatar = createAvatarEditorController({ auth,
    async createPreview(expected, signal) {
      if (disposed || signal.aborted || !auth.privacy.isCurrent(scope) || !auth.privacy.isCurrent(expected)) throw new AvatarError('cancelled')
      if (!source) throw new AvatarError('unavailable')
      return createReviewedAvatarPreview({ ...source.options, signal })
    },
    onCommitted(verifiedAvatar, expected) {
      if (!disposed && auth.privacy.isCurrent(scope) && auth.privacy.isCurrent(expected)) void me.refreshAfterAvatar(expected, verifiedAvatar.revision)
      // Authoritative world appearance arrives through existing server replication, never an optimistic local patch.
    },
  })
  const wholeAvatar = characters?.isCurrent() ? createWholeCharacterEditorController({ auth, scope, runtime: characters,
    onCommitted(verifiedAvatar, expected) {
      if (!disposed && auth.privacy.isCurrent(scope) && auth.privacy.isCurrent(expected)) void me.refreshAfterAvatar(expected, verifiedAvatar.revision)
    },
  }) : null
  const motionAvatar = reviewed ? createReviewedMotionEditorOwner({ auth, scope, authority: reviewed.authority, runtime: reviewed.runtime,
    onCommitted(verifiedAvatar, expected) {
      if (!disposed && auth.privacy.isCurrent(scope) && auth.privacy.isCurrent(expected)) void me.refreshAfterAvatar(expected, verifiedAvatar.revision)
    },
  }) : null
  // WP-08: the profile-owned avatar owner. It is admitted from the shell release
  // without an event gate, and it is disposed with the account.
  let profileAvatar: ProfileAvatarEditorOwner | null = null
  const avatarAdmission = new AbortController()
  if (isReviewedMotionRelease(bootstrap.document?.avatar_manifest)) {
    void admitProfileAvatar({ auth, scope, manifest: bootstrap.document?.avatar_manifest,
      canSave: bootstrap.capabilities?.mutations?.available === true,
      origin: globalThis.location?.origin ?? '', signal: avatarAdmission.signal,
    }).then(owner => {
      if (disposed || !auth.privacy.isCurrent(scope)) { owner?.dispose(); return }
      profileAvatar = owner
    }).catch(() => { profileAvatar = null })
  }
  let unregister = () => {}
  const dispose = () => { if (disposed) return; disposed = true; avatarAdmission.abort(); profileAvatar?.dispose(); unregister(); motionAvatar?.dispose(); wholeAvatar?.dispose(); avatar.dispose(); me.dispose(); leases.dispose() }
  unregister = auth.privacy.registerCleanup(dispose)
  return Object.freeze({ auth, bootstrap, scope, assets, characters, reviewed, me, avatar, wholeAvatar, motionAvatar, source,
    get profileAvatar() { return profileAvatar }, dispose, isDisposed: () => disposed })
}
export type AccountOwner = ReturnType<typeof createAccountOwner>
