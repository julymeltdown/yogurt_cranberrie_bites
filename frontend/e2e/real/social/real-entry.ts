import { createSessionController } from '../../../src/features/auth/model/session'
import { createSocialOwner, sameScope } from '../../../src/app/social-composition/owner'
import { canonicalSharingRequests } from '../../../src/app/social-composition/sharing'
import type { SocialContext, SocialAvailabilityPort } from '../../../src/app/social-composition/ports'
import { createSocialSharingController } from '../../../src/features/social-sharing/model/controller'
import { captureBootstrap, observePublicHTTP, type SourcePins } from './observations'

type BoundSocial = { owner: ReturnType<typeof createSocialOwner>; sharing: ReturnType<typeof createSocialSharingController>; dispose(): void }

/** start() launches reads; awaiting an overlapping guarded load() does not await that flight. */
export function waitForController(phase: () => string, subscribe: (listener: () => void) => () => void): Promise<void> {
  return new Promise((resolve, reject) => {
    let stop = () => {}
    const timer = setTimeout(() => { stop(); reject(new Error('NOT_RUN: social controller did not settle')) }, 18000)
    const check = () => {
      if (['idle', 'loading'].includes(phase())) return
      clearTimeout(timer); stop(); resolve()
    }
    stop = subscribe(check); check()
  })
}

/** One canonical identity. Never mount the default App beside this runner. */
export async function openRealIdentity(input: { eventId: string; userId: string; email: string; password: string }) {
  const http = observePublicHTTP()
  const auth = createSessionController({ eventId: input.eventId, qualifiedRefresh: false, fetch: http.fetch })
  let social: BoundSocial | null = null
  let disposed = false
  const dispose = () => { if (disposed) return; disposed = true; social?.dispose(); auth.dispose() }
  function bindSocial(availability: (context: SocialContext) => SocialAvailabilityPort | null): BoundSocial {
    if (disposed || social) throw new Error('IN03 identity already bound or disposed')
    const scope = auth.privacy.capture()
    if (!sameScope(scope, initialScope)) throw new Error('NOT_RUN: identity changed before social binding')
    const context = { auth, scope, bootstrap }
    // Borrow the production producer; C03 capabilities do not imply service readiness.
    const port = availability(context), lease = port?.read()
    if (!port || !lease || lease.bootstrap !== bootstrap || !sameScope(lease.scope, scope)) throw new Error('NOT_RUN: availability is not bound to this bootstrap and privacy scope')
    const owner = createSocialOwner({ ...context, availability: port, requests: canonicalSharingRequests, navigate: () => {} })
    const access = { read: () => ({ available: owner.available('requests'), readOnly: bootstrap.event.phase === 'READ_ONLY' || !bootstrap.capabilities.mutations.available }), subscribe: port.subscribe.bind(port) }
    const sharing = createSocialSharingController({ auth, access, sensitiveRead: auth.requestSensitive })
    sharing.start()
    social = { owner, sharing, dispose() { sharing.dispose(); owner.dispose() } }
    return social
  }
  let bootstrap: Awaited<ReturnType<typeof auth.loadBootstrap>>
  let initialScope: ReturnType<typeof auth.privacy.capture>
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
    bootstrap = await auth.loadBootstrap({ generation: session.generation, userId: input.userId, eventId: input.eventId }, new AbortController().signal)
    initialScope = auth.privacy.capture()
    return { auth, bootstrap, scope: initialScope, bindSocial, http: http.read,
      async bootstrapPacket(source: SourcePins) {
        if (disposed || !auth.privacy.isCurrent(initialScope)) throw new Error('NOT_RUN: stale bootstrap evidence lease')
        return captureBootstrap(bootstrap, initialScope, http.read(), source)
      }, dispose }
  } catch (error) { dispose(); throw error }
}

/** Compatibility entry for the retained controller preparation harness. */
export async function openRealSocial(input: { eventId: string; userId: string; email: string; password: string },
  availability: (context: SocialContext) => SocialAvailabilityPort | null) {
  const identity = await openRealIdentity(input)
  try { return { ...identity.bindSocial(availability), auth: identity.auth, http: identity.http, dispose: identity.dispose } }
  catch (error) { identity.dispose(); throw error }
}
