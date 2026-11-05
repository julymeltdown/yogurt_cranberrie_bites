import type { SessionController } from '../../features/auth/model/session'
import { capturedApplicationRelease } from '../../shared/release/application'
import { createPolicyTransport } from './transport'

/** Borrows the shell's canonical Auth and captures Root metadata per attempt. */
export function createDefaultPreWorldPolicy(auth: SessionController) {
  return createPolicyTransport(auth, capturedApplicationRelease)
}
