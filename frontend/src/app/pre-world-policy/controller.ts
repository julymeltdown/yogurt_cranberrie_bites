import type { PolicyAttempt, PolicyClock, PolicyView, PreWorldDecision, PreWorldPolicyPort } from './model'

const browserClock: PolicyClock = {
  now: () => performance.now(),
  set: (callback, ms) => setTimeout(callback, ms),
  clear: timer => clearTimeout(timer as ReturnType<typeof setTimeout>),
}
const version = (value: string) => !/[^0-9.]/.test(value) && /^(0|[1-9]\d{0,4})\.(0|[1-9]\d{0,4})\.(0|[1-9]\d{0,4})$/.test(value)
  && value.split('.').every(part => Number(part) <= 65535)

/** One controller per captured bootstrap attempt. No cache, World, socket or retry loop. */
export function createPreWorldPolicy(options: {
  port: PreWorldPolicyPort | undefined
  binding: PolicyAttempt | null
  isCurrent: () => boolean
  clock?: PolicyClock
  timeoutMs?: number
}) {
  const clock = options.clock ?? browserClock
  const binding = options.binding && Object.freeze({
    ...options.binding, scope: Object.freeze({ ...options.binding.scope }),
    release: Object.freeze({ ...options.binding.release }),
  })
  let view: PolicyView = Object.freeze({ kind: 'idle' })
  let flight: Promise<boolean> | undefined
  const lifetime = new AbortController()
  let timer: unknown
  let detach = () => {}
  let deadline = 0
  let disposed = false
  const expiredView: PolicyView = Object.freeze({ kind: 'unavailable', reason: 'expired' })
  const changedView: PolicyView = Object.freeze({ kind: 'unavailable', reason: 'changed' })
  const unavailableView: PolicyView = Object.freeze({ kind: 'unavailable', reason: 'unavailable' })
  const listeners = new Set<() => void>()
  const current = () => !disposed && Boolean(binding && binding.scope.gate === 'open'
    && binding.release.version === 1 && version(binding.release.applicationVersion)
    && binding.release.buildId.length === 64 && /^[0-9a-f]{64}$/.test(binding.release.buildId)
    && binding.release.sourceRevision.length === 40 && /^[0-9a-f]{40}$/.test(binding.release.sourceRevision)
    && Number.isSafeInteger(binding.attempt) && binding.attempt >= 0) && options.isCurrent()
  const publish = (next: PolicyView) => {
    view = Object.freeze(next)
    for (const listener of Array.from(listeners)) listener()
  }
  const clearTimer = () => { if (timer !== undefined) clock.clear(timer); timer = undefined }
  const invalidate = (reason: 'expired' | 'changed' | 'unavailable') => {
    clearTimer(); deadline = 0
    lifetime?.abort(); detach()
    if (!disposed && view.kind !== 'consumed') publish({ kind: 'unavailable', reason })
  }
  const fresh = () => current() && !lifetime?.signal.aborted && clock.now() < deadline
  function accept(decision: PreWorldDecision, started: number): boolean {
    const ttl = decision.validUntilMs - decision.serverTimeMs
    // Defense in depth for injected ports; strict wire validation belongs to the adapter.
    if (!Number.isSafeInteger(decision.eventRevision) || decision.eventRevision < 1
      || (decision.policyRevision !== null && (!Number.isSafeInteger(decision.policyRevision) || decision.policyRevision < 1))
      || !Number.isFinite(decision.serverTimeMs) || !Number.isFinite(decision.validUntilMs)
      || ttl <= 0 || ttl > 30_000 || !Number.isFinite(started)
      || (decision.minimumClientVersion !== null && !version(decision.minimumClientVersion))
      || (decision.kind !== 'unavailable' && (decision.policyRevision === null || decision.minimumClientVersion === null))
      || !['normal', 'maintenance', 'version', 'unavailable'].includes(decision.kind)) {
      invalidate('unavailable'); return false
    }
    deadline = started + ttl
    if (!fresh()) { invalidate(current() ? 'expired' : 'changed'); return false }
    clearTimer()
    timer = clock.set(() => invalidate(current() ? 'expired' : 'changed'), deadline - clock.now())
    switch (decision.kind) {
      case 'normal': publish({ kind: 'normal' }); return true
      case 'maintenance': publish({ kind: 'maintenance' }); return false
      case 'version': publish({ kind: 'version', minimumClientVersion: decision.minimumClientVersion! }); return false
      case 'unavailable': publish({ kind: 'unavailable', reason: 'unavailable' }); return false
    }
  }
  function prepare(signal: AbortSignal): Promise<boolean> {
    if (flight) return flight
    if (disposed || !binding || !options.port || !current() || signal.aborted) {
      invalidate('changed'); return Promise.resolve(false)
    }
    const port = options.port
    const captured = binding
    const owned = lifetime
    const started = clock.now()
    publish({ kind: 'checking' })
    const aborted = new Promise<false>(resolve => {
      const stop = () => resolve(false)
      owned.signal.addEventListener('abort', stop, { once: true })
      const cancel = () => invalidate('changed')
      signal.addEventListener('abort', cancel, { once: true })
      detach = () => {
        owned.signal.removeEventListener('abort', stop)
        signal.removeEventListener('abort', cancel)
      }
    })
    const timeout = Math.min(8_000, Math.max(1, options.timeoutMs ?? 8_000))
    deadline = started + timeout
    timer = clock.set(() => invalidate('unavailable'), timeout)
    flight = Promise.race([
      Promise.resolve().then(() => {
        if (owned.signal.aborted) return false
        if (!current()) { invalidate('changed'); return false }
        return port.read(captured, owned.signal).then(decision => {
          if (owned.signal.aborted) return false
          if (!current()) { invalidate('changed'); return false }
          return accept(decision, started)
        })
      }), aborted,
    ]).catch(error => { invalidate('unavailable'); throw error })
    return flight
  }
  return {
    signal: lifetime.signal,
    read: () => {
      // Pure snapshots also fence delayed background timers at the next render.
      if (view.kind === 'consumed' || view.kind === 'idle') return view
      if (!current()) return changedView
      if (deadline > 0 && clock.now() >= deadline) return view.kind === 'checking' ? unavailableView : expiredView
      return view
    },
    subscribe(listener: () => void) { listeners.add(listener); return () => { listeners.delete(listener) } },
    prepare,
    /** Foreground/BFCache checks only withdraw stale authority; they never fetch. */
    check() {
      if (view.kind === 'consumed' || view.kind === 'idle') return
      if (!current()) invalidate('changed')
      else if (deadline > 0 && clock.now() >= deadline) invalidate(view.kind === 'checking' ? 'unavailable' : 'expired')
    },
    /** Check immediately before bootstrap dispatch and again before committing its result. */
    isFresh() {
      if (view.kind !== 'normal') return false
      if (!fresh()) { invalidate(current() ? 'expired' : 'changed'); return false }
      return true
    },
    /** One-way consumption: existing World/ticket authority owns subsequent continuity. */
    consume() {
      if (view.kind !== 'normal' || !fresh()) { invalidate(current() ? 'expired' : 'changed'); return false }
      clearTimer(); detach(); publish({ kind: 'consumed' }); return true
    },
    dispose() {
      if (disposed) return
      disposed = true; clearTimer(); lifetime?.abort(); detach(); listeners.clear()
      deadline = 0; view = Object.freeze({ kind: 'unavailable', reason: 'changed' })
    },
  }
}
export type PreWorldPolicyController = ReturnType<typeof createPreWorldPolicy>
