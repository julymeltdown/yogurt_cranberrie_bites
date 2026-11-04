import { createWholeCharacterRuntime } from '../../entities/avatar/whole-character'
import type { SessionController } from '../../features/auth/model/session'
import { createCharacterRuntimeRegistry } from './registry'
import { createReviewedCharacterRegistry } from './reviewed-registry'
import type { PrivacyScope } from '../../shared/query/privacy'
import type { ShellBootstrap } from '../ports'

export function createRootCharacterRegistry(auth: SessionController, origin?: string) {
  const reviewed = createReviewedCharacterRegistry(auth, origin ?? location.origin)
  const whole = createCharacterRuntimeRegistry({ auth, create: ({ scope, manifest, pin }) => createWholeCharacterRuntime({
    privacy: { ...auth.privacy, capture: () => scope }, manifest, pin, origin: origin ?? location.origin,
  }) })
  return {
    get(scope: PrivacyScope, bootstrap: ShellBootstrap) { reviewed.clear(); return whole.get(scope, bootstrap) },
    read: whole.read,
    admitReviewed(scope: PrivacyScope, bootstrap: ShellBootstrap) { whole.clear(); return reviewed.admit(scope, bootstrap) },
    readReviewed: reviewed.read,
    dispose() { reviewed.dispose(); whole.dispose() },
  }
}
export type RootCharacterRegistry = ReturnType<typeof createRootCharacterRegistry>
