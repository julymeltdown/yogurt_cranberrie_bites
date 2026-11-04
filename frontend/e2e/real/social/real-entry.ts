import { createSessionController } from '../../../src/features/auth/model/session'
import { createSocialOwner, sameScope } from '../../../src/app/social-composition/owner'
import { canonicalSharingRequests } from '../../../src/app/social-composition/sharing'
import type { SocialContext } from '../../../src/app/social-composition/ports'
import type { IN03AvailabilityPort } from '../../../src/app/in03-entry/availability'
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
  let disposed = false, binding = false, pendingPort: IN03AvailabilityPort | null = null
  const lifetime = new AbortController()
  const dispose = () => { if (disposed) return; disposed = true; lifetime.abort(); http.dispose(); pendingPort?.dispose(); social?.dispose(); auth.dispose() }
  async function bindSocial(availability: (context: SocialContext) => IN03AvailabilityPort | null): Promise<BoundSocial> {
    if (disposed || social || binding) throw new Error('IN03 identity already bound or disposed')
    const scope = auth.privacy.capture()
    if (!sameScope(scope, initialScope)) throw new Error('NOT_RUN: identity changed before social binding')
    const context = { auth, scope, bootstrap }
    // Borrow the production producer; C03 capabilities do not imply service readiness.
    const port = availability(context)
    if (!port || typeof port.prepare !== 'function') throw new Error('NOT_RUN: asynchronous production availability is unavailable')
    binding = true; pendingPort = port
    let owner: ReturnType<typeof createSocialOwner> | null = null
    let sharing: ReturnType<typeof createSocialSharingController> | null = null
    try {
      await port.prepare(lifetime.signal)
      const lease = port.read()
      if (disposed || !auth.privacy.isCurrent(scope) || !lease || lease.bootstrap !== bootstrap || !sameScope(lease.scope, scope)) throw new Error('NOT_RUN: availability is not bound to this bootstrap and privacy scope')
      const createdOwner = createSocialOwner({ ...context, availability: port, requests: canonicalSharingRequests, navigate: () => {} })
      owner = createdOwner
      const access = { read: () => ({ available: createdOwner.available('requests'), readOnly: bootstrap.event.phase === 'READ_ONLY' || !bootstrap.capabilities.mutations.available }), subscribe: port.subscribe.bind(port) }
      const createdSharing = createSocialSharingController({ auth, access, sensitiveRead: auth.requestSensitive })
      sharing = createdSharing; createdSharing.start()
      social = { owner: createdOwner, sharing: createdSharing, dispose() { createdSharing.dispose(); createdOwner.dispose(); port.dispose() } }
      return social
    } catch (error) { sharing?.dispose(); owner?.dispose(); port.dispose(); throw error }
    finally { pendingPort = null; binding = false }
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
    return { auth, get bootstrap() { return bootstrap }, get scope() { return initialScope }, bindSocial, http: http.read,
      holdNextProfileRead: http.holdNextProfileRead,
      async reopenSocial(availability: (context: SocialContext) => IN03AvailabilityPort | null) {
        if (disposed || binding) throw new Error('NOT_RUN: canonical actor cannot rebind')
        social?.dispose(); social = null
        const captured = auth.privacy.capture(), session = auth.session.read()
        if (!auth.privacy.isCurrent(captured) || session.userId !== input.userId || session.eventId !== input.eventId) throw new Error('NOT_RUN: canonical actor changed before rebind')
        const next = await auth.loadBootstrap({ generation: session.generation, userId: input.userId, eventId: input.eventId }, lifetime.signal)
        if (disposed || !auth.privacy.isCurrent(captured)) throw new Error('NOT_RUN: stale bootstrap rebind')
        bootstrap = next; initialScope = captured
        return bindSocial(availability)
      },
      async bootstrapPacket(source: SourcePins) {
        if (disposed || !auth.privacy.isCurrent(initialScope)) throw new Error('NOT_RUN: stale bootstrap evidence lease')
        return captureBootstrap(bootstrap, initialScope, http.read(), source)
      }, dispose }
  } catch (error) { dispose(); throw error }
}

/** Compatibility entry for the retained controller preparation harness. */
export async function openRealSocial(input: { eventId: string; userId: string; email: string; password: string },
  availability: (context: SocialContext) => IN03AvailabilityPort | null) {
  const identity = await openRealIdentity(input)
  try { return { ...await identity.bindSocial(availability), auth: identity.auth, http: identity.http, dispose: identity.dispose } }
  catch (error) { identity.dispose(); throw error }
}
