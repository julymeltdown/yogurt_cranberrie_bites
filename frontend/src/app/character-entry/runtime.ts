import { createWholeCharacterRuntime } from '../../entities/avatar/whole-character'
import type { SessionController } from '../../features/auth/model/session'
import { createCharacterRuntimeRegistry } from './registry'

export function createRootCharacterRegistry(auth: SessionController, origin?: string) {
  return createCharacterRuntimeRegistry({ auth, create: ({ manifest, pin }) => createWholeCharacterRuntime({
    privacy: auth.privacy, manifest, pin, origin: origin ?? location.origin,
  }) })
}
export type RootCharacterRegistry = ReturnType<typeof createRootCharacterRegistry>
