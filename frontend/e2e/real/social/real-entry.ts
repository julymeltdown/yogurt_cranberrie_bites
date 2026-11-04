import { createSessionController } from '../../../src/features/auth/model/session'
import { createSocialOwner, sameScope } from '../../../src/app/social-composition/owner'
import { canonicalSharingRequests } from '../../../src/app/social-composition/sharing'
import type { SocialContext, SocialAvailabilityPort } from '../../../src/app/social-composition/ports'
import { createSocialSharingController } from '../../../src/features/social-sharing/model/controller'

/** Standalone controller harness. Does not mount default App, Phaser, World or a WebSocket. */
export async function openRealSocial(input: { eventId: string; userId: string; email: string; password: string },
  availability: (context: SocialContext) => SocialAvailabilityPort) {
  const auth = createSessionController({ eventId: input.eventId, qualifiedRefresh: false })
  try {
    auth.start()
    await new Promise<void>((resolve, reject) => {
      let stop = () => {}
      const timer = setTimeout(() => { stop(); reject(new Error('NOT_RUN: Auth capabilities unavailable')) }, 18000)
      const check = () => {
        if (auth.read().capabilities === 'loading') return
        clearTimeout(timer); stop()
        if (auth.read().capabilities === 'ready' && auth.read().emailEnabled) resolve()
        else reject(new Error('NOT_RUN: canonical cookie login capability unavailable'))
      }
      stop = auth.subscribe(check); check()
    })
    if (!await auth.login(input.email, input.password)) throw new Error('NOT_RUN: canonical cookie login unavailable')
    const session = auth.session.read()
    if (session.userId !== input.userId || session.eventId !== input.eventId) throw new Error('NOT_RUN: principal context mismatch')
    const bootstrap = await auth.loadBootstrap({ generation: session.generation, userId: input.userId, eventId: input.eventId }, new AbortController().signal)
    const scope = auth.privacy.capture()
    const context = { auth, scope, bootstrap }
    // The host must supply its actual reviewed producer. C03 capabilities do not imply service readiness.
    const port = availability(context), lease = port.read()
    if (!lease || lease.bootstrap !== bootstrap || !sameScope(lease.scope, scope)) throw new Error('NOT_RUN: availability is not bound to this bootstrap and privacy scope')
    const owner = createSocialOwner({ ...context, availability: port, requests: canonicalSharingRequests, navigate: () => {} })
    const access = { read: () => ({ available: owner.available('requests'), readOnly: bootstrap.event.phase === 'READ_ONLY' || !bootstrap.capabilities.mutations.available }), subscribe: port.subscribe.bind(port) }
    const sharing = createSocialSharingController({ auth, access, sensitiveRead: auth.requestSensitive })
    sharing.start()
    return { owner, sharing, dispose() { sharing.dispose(); owner.dispose(); auth.dispose() } }
  } catch (error) { auth.dispose(); throw error }
}
