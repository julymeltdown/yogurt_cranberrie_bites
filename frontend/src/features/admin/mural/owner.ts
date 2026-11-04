import type { OperatorContext, OperatorContextPort, PrivacyPort } from './contract'
import { qualifiedContext, sameContext, sameScope } from './contract'
export interface ReadLease { readonly context: OperatorContext; readonly signal: AbortSignal; readonly serial: number }
/** One feature lifetime; shell retains it through panel navigation. No persistence or background mutation. */
export function createAdminOwner(privacy: PrivacyPort, context: OperatorContextPort, changed: (security: boolean) => void) {
  const scope = Object.freeze({ ...privacy.capture() })
  let ended = false, started = false, serial = 0, active: AbortController | null = null, observed: OperatorContext | null = null
  let stopContext = () => {}, stopPrivacy = () => {}
  const capture = () => {
    const next = qualifiedContext(privacy, context)
    return !ended && privacy.isCurrent(scope) && next && sameScope(next.scope, scope) ? next : null
  }
  const cancel = () => { ++serial; active?.abort(); active = null }
  const update = () => {
    const next = capture()
    if ((!next && observed) || next && (!observed || !sameContext(next, observed))) {
      const security = !next || !observed || next.roleRevision !== observed.roleRevision || !sameScope(next.scope, observed.scope)
      observed = next; cancel(); changed(security)
    }
  }
  return {
    capture,
    begin(): ReadLease | null {
      const value = capture(); if (!value || active) return null
      active = new AbortController(); return { context: value, signal: active.signal, serial: ++serial }
    },
    valid(lease: ReadLease) { const value = capture(); return !lease.signal.aborted && serial === lease.serial && !!value && sameContext(value, lease.context) },
    finish(lease: ReadLease) { if (lease.serial === serial) active = null },
    cancel,
    start() {
      if (ended || started) return
      started = true; observed = capture()
      stopContext = context.subscribe(update)
      stopPrivacy = privacy.registerCleanup(() => { if (ended) return; observed = null; cancel(); ended = true; changed(true); stopContext() })
    },
    dispose() { if (ended) { stopPrivacy(); return }; ended = true; cancel(); stopContext(); stopPrivacy() },
  }
}
