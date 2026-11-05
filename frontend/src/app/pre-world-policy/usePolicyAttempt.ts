import { useEffect, useState, useSyncExternalStore } from 'react'
import type { PrivacyScope } from '../../shared/query/privacy'
import type { ActiveScope } from '../ports'
import { createPreWorldPolicy, type PreWorldPolicyController } from './controller'
import type { PolicyApplicationRelease, PolicyView, PreWorldPolicyPort } from './model'

const idle: PolicyView = Object.freeze({ kind: 'idle' })
const noSubscribe = () => () => {}
const readIdle = () => idle

/** Allocate only inside useScopedLoad's live dispatch, so StrictMode's discarded effect
 * cannot dispose a controller reused by the next effect. No render-time side effects. */
export function usePolicyAttempt(options: {
  source: object
  port: PreWorldPolicyPort | undefined
  scope: ActiveScope
  privacyScope: PrivacyScope | null
  attempt: number
  isCurrent: () => boolean
}) {
  const { port, scope, privacyScope, attempt, source } = options
  const key = `${scope.userId}:${scope.eventId}:${scope.generation}:${privacyScope?.policyGeneration ?? -1}:${attempt}`
  const [slot, setSlot] = useState<{ key: string; source: object; port: PreWorldPolicyPort | undefined; controller: PreWorldPolicyController } | null>(null)
  const controller = slot?.key === key && slot.port === port && slot.source === source ? slot.controller : null
  const view = useSyncExternalStore(controller?.subscribe ?? noSubscribe, controller?.read ?? readIdle)
  useEffect(() => {
    if (!controller || view.kind === 'consumed') return
    const check = () => { if (document.visibilityState !== 'hidden') controller.check() }
    document.addEventListener('visibilitychange', check)
    window.addEventListener('focus', check)
    window.addEventListener('pageshow', check)
    check()
    return () => {
      document.removeEventListener('visibilitychange', check)
      window.removeEventListener('focus', check)
      window.removeEventListener('pageshow', check)
    }
  }, [controller, view.kind])
  function start(signal: AbortSignal) {
    // The actual Root identity is captured exactly once, never taken from package.json,
    // a map/theme contract version, storage, or a caller's version query string.
    let release: PolicyApplicationRelease | null = null
    try { release = port?.release() ?? null } catch { /* Missing Root identity fails closed. */ }
    const captured = privacyScope
    const owner = createPreWorldPolicy({ port,
      binding: release && captured?.gate === 'open' && captured.userId === scope.userId
        && captured.eventId === scope.eventId && captured.generation === scope.generation
        ? { scope: Object.freeze({ ...captured, userId: captured.userId, eventId: captured.eventId }), release, attempt } : null,
      isCurrent: options.isCurrent })
    const stop = () => owner.dispose()
    signal.addEventListener('abort', stop, { once: true })
    if (signal.aborted) stop()
    setSlot({ key, source, port, controller: owner })
    return owner
  }
  return { view, start }
}
