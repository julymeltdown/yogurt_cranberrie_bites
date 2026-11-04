import type { SessionController } from '../../../features/auth/model/session'
import type { HttpClient } from '../../../shared/api/http'
import { createMuralToolsHttp } from '../../../features/mural/sync'
import { createWorldMuralAuthority } from '../../../world/mural'
import type { MuralQualificationFactory } from '../../../world/mural-entry-lifecycle/source'
import { createMuralAdmissionSource, matchesMuralPlacement } from './authority'
import { validateMuralProductionConfiguration } from './configuration'
import { subscribeMuralEligibilityRefresh } from './refresh'

/** Reuses canonical HTTP and the incumbent scene; configuration alone grants no server capability. */
export function createProductionMuralFactory({ auth, configuration }: {
  auth: SessionController; configuration: unknown
}): MuralQualificationFactory | null {
  const placement = validateMuralProductionConfiguration(configuration)
  if (!placement) return null
  return context => {
    if (!matchesMuralPlacement(placement, context) || context.signal.aborted || !auth.privacy.isCurrent(context.scope)) return null
    const scope = Object.freeze({ ...context.scope }), request = auth.request.bind(auth)
    const http: HttpClient = {
      request(input) {
        if (context.signal.aborted || !auth.privacy.isCurrent(scope)) return Promise.reject(new Error('Mural release unavailable'))
        const signal = AbortSignal.any([context.signal, ...(input.signal ? [input.signal] : [])])
        return auth.privacy.run(scope, async operation => {
          const response = await request({ ...input, signal: operation.signal })
          if (signal.aborted || !auth.privacy.isCurrent(scope)) throw new Error('Mural release changed')
          return response
        }, signal)
      },
      refreshSession: async () => { throw new Error('Use canonical Auth refresh ownership') },
    }
    // GET load requires a live matching World binding, never writable=true.
    const tools = createMuralToolsHttp({ http, privacy: auth.privacy, authority: createWorldMuralAuthority({ privacy: auth.privacy, world: context.world }) })
    const source = createMuralAdmissionSource({ privacy: auth.privacy, context, configuration: placement, loadTools: tools.load })
    let ended = false, stopRefresh = () => {}, stopPrivacy = () => {}
    const dispose = () => {
      if (ended) return
      ended = true; source.dispose(); stopRefresh(); stopPrivacy(); context.signal.removeEventListener('abort', dispose)
    }
    try {
      context.signal.addEventListener('abort', dispose, { once: true })
      stopRefresh = subscribeMuralEligibilityRefresh(auth, scope, source.invalidate)
      stopPrivacy = auth.privacy.registerCleanup(dispose)
      if (ended || context.signal.aborted || !auth.privacy.isCurrent(scope)) { stopRefresh(); stopPrivacy(); dispose() }
    } catch { dispose() }
    return Object.freeze({ read: source.read, subscribe: source.subscribe })
  }
}
