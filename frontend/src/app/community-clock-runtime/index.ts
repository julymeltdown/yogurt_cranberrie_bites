import type { CommunityClockProfile } from '../community-adoption/bootstrap'
import { createAuthorityTimeline, type ClockQualification, type PerformanceObservation } from '../community-authority'
import type { ShellBootstrap } from '../ports'
import { decodeCompiledClockInput, deriveClockLimits, sameClockBrowser, secondsTimestamp, type ClockBrowser } from './input'

export interface ClockProfileLifetime {
  isCurrent(): boolean
  subscribe(listener: () => void): () => void
  /** SAME decoded canonical response; never fetch or relabel a cached body. */
  qualify(source: ShellBootstrap): ClockQualification | null
}
export interface RuntimeClockProfile extends CommunityClockProfile { readonly runtime: ClockProfileLifetime }
export interface ClockRuntimeEnvironment {
  readonly origin: () => string
  readonly eventId: string | null
  readonly browser: () => ClockBrowser
  readonly observe: () => PerformanceObservation | null
  readonly onInterruption: (withdraw: () => void) => () => void
}

/** raw is compiled, catalogued Root build input, NOT a query, fetched object,
 * localStorage, Bootstrap property or role/config grant. Evidence SHA identifies
 * the captured measurement; release input custody supplies its provenance. */
export function createCommunityClockRuntime(raw: unknown, environment: ClockRuntimeEnvironment) {
  const listeners = new Set<() => void>()
  let active = false, profile: RuntimeClockProfile | undefined
  let timeline: ReturnType<typeof createAuthorityTimeline> | undefined
  let stop = () => {}, timer: ReturnType<typeof setTimeout> | undefined
  function dispose() {
    if (!active) return
    active = false; clearTimeout(timer)
    for (const release of [stop, () => timeline?.dispose()]) {
      try { release() } catch { /* Release all lifetimes even if an adapter fails. */ }
    }
    for (const listener of Array.from(listeners)) { try { listener() } catch { /* Withdraw every borrower. */ } }
    listeners.clear()
  }
  try {
    const input = decodeCompiledClockInput(raw), { measurement, current } = input
    const limits = deriveClockLimits(measurement)
    const supported = () => environment.origin() === current.origin && environment.eventId === current.eventId
      && sameClockBrowser(environment.browser(), measurement.browser)
    if (!supported()) return Object.freeze({ profile, dispose })
    active = true
    timeline = createAuthorityTimeline({ observe: () => supported() ? environment.observe() : null,
      maxObservationGapMs: limits.maxObservationGapMs, onInterruption: environment.onInterruption })
    stop = timeline.subscribe(dispose)
    const started = timeline.capture()
    const isCurrent = () => {
      if (!active) return false
      const at = timeline?.capture()
      if (!started || !at || at.nowMs - started.nowMs >= limits.maxAgeMs) { dispose(); return false }
      return active
    }
    if (!isCurrent()) { dispose(); return Object.freeze({ profile, dispose }) }
    // A finite measured window is not a permanent platform guarantee. Neither a
    // new canonical request nor a document restore extends this instance's life.
    timer = setTimeout(dispose, limits.maxAgeMs)
    const lifetime: ClockProfileLifetime = Object.freeze({ isCurrent,
      subscribe(listener: () => void) {
        if (!isCurrent()) { listener(); return () => {} }
        listeners.add(listener); return () => { listeners.delete(listener) }
      },
      qualify(source: ShellBootstrap) {
        if (!isCurrent()) return null
        try {
          if (source.event.eventId !== current.eventId || !source.document) return null
          const server = secondsTimestamp(source.document.server_time)
          const lastMeasured = secondsTimestamp(measurement.samples.at(-1)!.serverTime)
          if (server < lastMeasured) return null
          // Existing authority separately includes one-second wire precision.
          // Cap TOTAL age from request start so the interval's upper bound never
          // crosses the actual qualification's server-time expiry.
          const age = Math.min(limits.maxAgeMs, Math.floor((limits.serverValidUntilMs - server - 1000 - limits.uncertaintyMs) / limits.maxElapsedRate))
          if (age < 1) return null
          return Object.freeze({ maxAgeMs: age, uncertaintyMs: limits.uncertaintyMs, maxElapsedRate: limits.maxElapsedRate })
        } catch { return null }
      },
    })
    profile = Object.freeze({ classification: 'QUALIFIED_RUNTIME', evidence: 'sha256:' + measurement.evidenceSha256,
      maxAgeMs: limits.maxAgeMs, uncertaintyMs: limits.uncertaintyMs,
      maxElapsedRate: limits.maxElapsedRate, maxObservationGapMs: limits.maxObservationGapMs, runtime: lifetime })
  } catch { dispose() }
  return Object.freeze({ profile, dispose })
}

/** Root/50's separate assembly MUST include this exact input in a new buildId.
 * No input is configured in source, and no guessed values are defaulted here. */
export function createProductionCommunityClock(eventId: string | null) {
  return createCommunityClockRuntime(import.meta.env.VITE_MCR_COMMUNITY_CLOCK_INPUT, {
    eventId,
    origin: () => window.location.origin,
    browser: () => ({ userAgent: navigator.userAgent, platform: navigator.platform, crossOriginIsolated: window.crossOriginIsolated === true }),
    observe: () => typeof window === 'undefined' || document.visibilityState !== 'visible' ? null
      : { nowMs: window.performance.now(), origin: window.performance.timeOrigin },
    onInterruption(withdraw) {
      if (typeof window === 'undefined') { withdraw(); return () => {} }
      const visibility = () => { if (document.visibilityState !== 'visible') withdraw() }
      const restored = (event: PageTransitionEvent) => { if (event.persisted) withdraw() }
      document.addEventListener('visibilitychange', visibility)
      document.addEventListener('freeze', withdraw); document.addEventListener('resume', withdraw)
      window.addEventListener('pagehide', withdraw); window.addEventListener('pageshow', restored)
      return () => {
        document.removeEventListener('visibilitychange', visibility)
        document.removeEventListener('freeze', withdraw); document.removeEventListener('resume', withdraw)
        window.removeEventListener('pagehide', withdraw); window.removeEventListener('pageshow', restored)
      }
    },
  })
}
