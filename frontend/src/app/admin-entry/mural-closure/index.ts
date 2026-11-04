import type { SessionController } from '../../../features/auth/model/session'
import { createMuralControl, createMuralAdminApi, createAdminTransport } from '../../../features/admin/mural'
import { createEventClosure, createClosureApi } from '../../../features/admin/closure'
import { qualifiedContext, sameScope, type OperatorContextPort } from '../../../features/admin/mural/contract'
import type { PrivacyScope } from '../../../shared/query/privacy'

export type MuralClosureView = 'mural' | 'closure'
export interface MuralClosureOptions {
  readonly auth: Pick<SessionController, 'request' | 'privacy'>
  readonly scope: PrivacyScope
  /** Root's current server-qualified authority. This adapter never obtains or grants a role. */
  readonly context: OperatorContextPort
  readonly now?: () => number
  readonly newKey?: () => string
}

/** Retain above panel switches. The canonical shell owns disposal, not the panel. */
export function createMuralClosureEntry({ auth, scope, context, now, newKey }: MuralClosureOptions) {
  const expected = Object.freeze({ ...scope })
  if (!auth.privacy.isCurrent(expected) || !sameScope(expected, auth.privacy.capture())) throw new Error('Admin scope unavailable')
  let disposed = false, version = 0, wallStarted = false, closureStarted = false
  const listeners = new Set<() => void>()
  // Do not pass an unscoped/new transport around canonical Auth's request policy.
  const boundContext: OperatorContextPort = {
    read() {
      if (disposed || !auth.privacy.isCurrent(expected)) return null
      const current = qualifiedContext(auth.privacy, context)
      return current && sameScope(current.scope, expected) ? current : null
    },
    subscribe: listener => context.subscribe(listener),
  }
  const transport = createAdminTransport({ http: { request: input => auth.request(input) }, privacy: auth.privacy, context: boundContext,
    ...(now ? { now } : {}) })
  const keyOptions = newKey ? { newKey } : {}
  const wall = createMuralControl({ privacy: auth.privacy, context: boundContext, api: createMuralAdminApi(transport), ...keyOptions })
  const closure = createEventClosure({ privacy: auth.privacy, context: boundContext, api: createClosureApi(transport), ...keyOptions })
  const emit = () => { ++version; for (const listener of Array.from(listeners)) { try { listener() } catch { /* Independent views. */ } } }
  let stopContext = () => {}, stopPrivacy = () => {}
  const dispose = () => {
    if (disposed) return
    disposed = true
    stopContext(); stopPrivacy(); wall.dispose(); closure.dispose(); transport.dispose(); emit(); listeners.clear()
  }
  try {
    stopContext = context.subscribe(emit)
    stopPrivacy = auth.privacy.registerCleanup(dispose)
  } catch (error) { dispose(); throw error }
  return Object.freeze({
    wall, closure,
    isCurrent: () => boundContext.read() !== null,
    isDisposed: () => disposed,
    read: () => version,
    subscribe(listener: () => void) { if (disposed) return () => {}; listeners.add(listener); return () => { listeners.delete(listener) } },
    async start(view: MuralClosureView) {
      if (disposed) return
      if (view === 'mural' && !wallStarted) { wallStarted = true; await wall.start() }
      if (view === 'closure' && !closureStarted) { closureStarted = true; closure.start() }
    },
    dispose,
  })
}
export type MuralClosureEntry = ReturnType<typeof createMuralClosureEntry>
