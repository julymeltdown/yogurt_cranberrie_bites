// TEST ONLY: installed by the native runner's Vite transform, never imported by the product.
import { QueryObserver } from '@tanstack/react-query'
import type { SessionController } from '../../../features/auth/model/session'
import type { ShellPorts } from '../../ports'
import type { createAppRouter } from '../../router'
import type { PrivateQueryKey } from '../../../shared/query/keys'

export function installQualificationBridge({ auth, ports, router, dispose }: {
  auth: SessionController; ports: ShellPorts; router: ReturnType<typeof createAppRouter>; dispose(): void
}) {
  const results: Record<string, { ok: boolean; kind?: string; status?: number; uncertain?: boolean }> = {}
  let observer: QueryObserver<string, Error, string, string, PrivateQueryKey> | undefined, stop: (() => void) | undefined, unregister: (() => void) | undefined
  let signal: AbortSignal | undefined, scope: ReturnType<typeof auth.privacy.capture> | undefined
  let draft = '', subscriptions = 0
  function record(name: string, work: Promise<unknown>) {
    void work.then(() => { results[name] = { ok: true } }, (error: unknown) => {
      const safe = error as { kind?: string; status?: number; uncertain?: boolean }
      results[name] = { ok: false, ...(safe.kind ? { kind: safe.kind } : {}),
        ...(safe.status ? { status: safe.status } : {}), ...(safe.uncertain ? { uncertain: true } : {}) }
    })
  }
  const api = {
    async login() { return auth.login('fixture@example.test', 'fixture-password') },
    prime() {
      stop?.(); observer?.destroy(); unregister?.()
      signal = auth.session.read().signal; scope = auth.privacy.capture(); draft = 'private fixture draft'; subscriptions = 1
      unregister = auth.privacy.registerCleanup(() => { draft = ''; subscriptions = 0 })
      observer = new QueryObserver(auth.queryClient, auth.privacy.options({ resource: 'participant', read: () => 'private fixture value' }))
      stop = observer.subscribe(() => undefined)
    },
    read(label: string, mutation = false) {
      const event = auth.session.read().eventId
      record(label, auth.request({ path: `/api/concert/events/${event}/qualification/${label}`, method: mutation ? 'PATCH' : 'GET',
        access: 'protected', errorFamily: 'core', decode: value => value, ...(mutation ? { body: { json: { enabled: true } } } : {}) }))
    },
    logout() { record('logout', auth.logout().then(ok => { if (!ok) throw { kind: 'unconfirmed' } })) },
    retryCapabilities() { auth.retryCapabilities() },
    state() {
      const session = auth.session.read()
      return { auth: auth.read(), generation: session.generation, tokenVersion: session.tokenVersion,
        authenticated: session.userId !== null, credentialPresent: session.accessToken !== null,
        canonicalPorts: ports.auth === auth && ports.session === auth.session,
        private: { queries: auth.queryClient.getQueryCache().getAll().length,
          observed: observer?.getCurrentResult().data !== undefined, draft: draft !== '', subscriptions,
          oldSignalAborted: signal?.aborted ?? false, oldScopeCurrent: scope ? auth.privacy.isCurrent(scope) : false },
        results: structuredClone(results), route: router.state.location.pathname }
    },
    dispose() { stop?.(); observer?.destroy(); unregister?.(); dispose() },
  }
  Object.assign(globalThis, { __PRODUCTION_REFRESH__: api })
}
