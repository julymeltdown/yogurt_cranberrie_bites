import { AvatarError, createReviewedAtlasLeases } from '../../entities/avatar/runtime'
import { createAvatarEditorController, createReviewedAvatarPreview } from '../../features/avatar'
import { createMeController } from '../../features/me'
import type { SessionController } from '../../features/auth/model/session'
import type { PrivacyScope } from '../../shared/query/privacy'
import type { ShellBootstrap } from '../ports'
import { thumbnailSource, type AccountAssetsPort } from './assets'
import type { WholeCharacterRuntime } from '../../entities/avatar/whole-character'
import { createWholeCharacterEditorController } from '../../features/avatar/whole-character/controller'

export interface AccountOwnerOptions {
  readonly auth: SessionController
  readonly bootstrap: ShellBootstrap
  readonly scope: PrivacyScope
  readonly assets?: AccountAssetsPort | undefined
  readonly characters?: WholeCharacterRuntime | null
}
/** Own controllers only. Neither creates a session, QueryClient, Phaser game nor world connection. */
export function createAccountOwner({ auth, bootstrap, scope, assets, characters = null }: AccountOwnerOptions) {
  if (!auth.privacy.isCurrent(scope) || bootstrap.participant.userId !== scope.userId || bootstrap.participant.eventId !== scope.eventId
    || bootstrap.event.eventId !== scope.eventId) throw new AvatarError('unavailable')
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
  let unregister = () => {}
  const dispose = () => { if (disposed) return; disposed = true; unregister(); wholeAvatar?.dispose(); avatar.dispose(); me.dispose(); leases.dispose() }
  unregister = auth.privacy.registerCleanup(dispose)
  return Object.freeze({ auth, bootstrap, scope, assets, characters, me, avatar, wholeAvatar, source, dispose, isDisposed: () => disposed })
}
export type AccountOwner = ReturnType<typeof createAccountOwner>
