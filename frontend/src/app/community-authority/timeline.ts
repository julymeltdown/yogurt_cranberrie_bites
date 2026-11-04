/** Process-local performance observations. Copies and observations from another timeline are unusable. */
export interface PerformanceObservation { readonly nowMs: number; readonly origin: number }
export interface AuthorityTimeline {
  capture(): PerformanceObservation | null
  measure(start: PerformanceObservation, receipt: PerformanceObservation): {
    readonly sinceStartMs: number; readonly sinceReceiptMs: number; readonly roundTripMs: number
  } | null
  subscribe(listener: () => void): () => void
  dispose(): void
}
export interface TimelineOptions {
  /** Same monotonic performance clock for the entire request and subsequent reads. */
  readonly observe: () => PerformanceObservation | null
  readonly maxObservationGapMs: number
  /** Lifecycle interruption withdraws the timeline permanently; a new request needs a new timeline. */
  readonly onInterruption: (invalidate: () => void) => () => void
}
const finite = (n: number) => Number.isFinite(n) && n >= 0 && n <= Number.MAX_SAFE_INTEGER

/** No wall-clock, persistence, Auth, request, or runtime ownership. */
export function createAuthorityTimeline(options: TimelineOptions): AuthorityTimeline {
  let active = true, previous: PerformanceObservation | null = null
  let stop = () => {}, timer: ReturnType<typeof setInterval> | undefined
  const observations = new WeakSet<PerformanceObservation>(), listeners = new Set<() => void>()
  function dispose() {
    if (!active) return
    active = false; clearInterval(timer)
    try { stop() } catch { /* Withdrawal cannot depend on an environment listener. */ }
    for (const fn of Array.from(listeners)) { try { fn() } catch { /* Notify every borrower. */ } }
    listeners.clear()
  }
  function capture(): PerformanceObservation | null {
    if (!active) return null
    try {
      const value = options.observe()
      if (!active) return null
      if (!value || !finite(value.nowMs) || !finite(value.origin) || (previous && (value.origin !== previous.origin
        || value.nowMs < previous.nowMs || value.nowMs - previous.nowMs > options.maxObservationGapMs))) {
        dispose(); return null
      }
      previous = Object.freeze({ nowMs: value.nowMs, origin: value.origin })
      observations.add(previous)
      return previous
    } catch { dispose(); return null }
  }
  if (!Number.isFinite(options.maxObservationGapMs) || options.maxObservationGapMs < 2
    || options.maxObservationGapMs > 2_147_483_647) dispose()
  else {
    try {
      const cleanup = options.onInterruption(dispose)
      if (active) stop = cleanup; else cleanup()
      if (capture()) timer = setInterval(capture, Math.min(1000, Math.floor(options.maxObservationGapMs / 2)))
    } catch { dispose() }
  }
  return Object.freeze({ capture,
    measure(start: PerformanceObservation, receipt: PerformanceObservation) {
      if (!observations.has(start) || !observations.has(receipt) || start.nowMs > receipt.nowMs) return null
      const now = capture()
      if (!now || receipt.nowMs > now.nowMs) return null
      return Object.freeze({ sinceStartMs: now.nowMs - start.nowMs, sinceReceiptMs: now.nowMs - receipt.nowMs,
        roundTripMs: receipt.nowMs - start.nowMs })
    },
    subscribe(listener: () => void) {
      if (!active) return () => {}
      const entry = () => listener(); listeners.add(entry); return () => { listeners.delete(entry) }
    },
    dispose,
  })
}

/** Create BEFORE canonical request dispatch. Hidden/frozen/BFCache contexts must reacquire a fresh anchor. */
export function createBrowserAuthorityTimeline(maxObservationGapMs: number, signal: AbortSignal): AuthorityTimeline {
  return createAuthorityTimeline({ maxObservationGapMs,
    observe: () => typeof window === 'undefined' || document.visibilityState !== 'visible' ? null
      : { nowMs: window.performance.now(), origin: window.performance.timeOrigin },
    onInterruption(invalidate) {
      if (signal.aborted) { invalidate(); return () => {} }
      signal.addEventListener('abort', invalidate, { once: true })
      if (typeof window === 'undefined') return () => signal.removeEventListener('abort', invalidate)
      const visibility = () => { if (document.visibilityState !== 'visible') invalidate() }
      const restored = (event: PageTransitionEvent) => { if (event.persisted) invalidate() }
      document.addEventListener('visibilitychange', visibility)
      document.addEventListener('freeze', invalidate); document.addEventListener('resume', invalidate)
      window.addEventListener('pagehide', invalidate); window.addEventListener('pageshow', restored)
      return () => {
        signal.removeEventListener('abort', invalidate)
        document.removeEventListener('visibilitychange', visibility)
        document.removeEventListener('freeze', invalidate); document.removeEventListener('resume', invalidate)
        window.removeEventListener('pagehide', invalidate); window.removeEventListener('pageshow', restored)
      }
    },
  })
}
