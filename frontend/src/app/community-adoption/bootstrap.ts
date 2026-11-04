import type { ShellBootstrap } from '../ports'
import type { PrivacyScope } from '../../shared/query/privacy'
import type { SessionController } from '../../features/auth/model/session'
import { createBrowserAuthorityTimeline, createCommunityAuthority, type AuthorityTimeline,
  type ClockQualification, type CommunityAuthority } from '../community-authority'

/** No default exists. TEST_ONLY profiles are rejected in production builds. */
export interface CommunityClockProfile extends ClockQualification {
  readonly maxObservationGapMs: number
  readonly classification: 'QUALIFIED_RUNTIME' | 'TEST_ONLY'
  readonly evidence: string
  readonly runtime?: {
    isCurrent(): boolean
    subscribe(withdraw: () => void): () => void
    qualify(source: ShellBootstrap): ClockQualification | null
  }
}
export interface CommunityBootstrapBinding {
  readonly source: ShellBootstrap
  readonly scope: PrivacyScope | null
  readonly signal: AbortSignal
  isCurrent(): boolean
  read(): CommunityAuthority | null
  dispose(): void
}
type Privacy = Pick<SessionController['privacy'], 'isCurrent' | 'registerCleanup'>
/** Called immediately before the SAME canonical Bootstrap load, never a second loader. */
export function beginCommunityBootstrap({ privacy, scope, signal, profile }: {
  readonly privacy: Privacy | undefined; readonly scope: PrivacyScope | null; readonly signal: AbortSignal
  readonly profile: CommunityClockProfile | undefined
}, makeTimeline: (gap: number, signal: AbortSignal) => AuthorityTimeline = createBrowserAuthorityTimeline) {
  let active = true, completed = false, authority: CommunityAuthority | null = null
  let timeline: AuthorityTimeline | null = null, stopPrivacy = () => {}, stopAuthority = () => {}, stopRuntime = () => {}
  const captured = scope ? Object.freeze({ ...scope }) : null
  function isCurrent() {
    if (!active) return false
    try {
      if (!signal.aborted && captured && privacy?.isCurrent(captured) && (!profile?.runtime || profile.runtime.isCurrent())) return active
    } catch { /* A failed lifetime callback withdraws the retained binding. */ }
    dispose(); return false
  }
  function dispose() {
    if (!active) return
    active = false; signal.removeEventListener('abort', dispose)
    for (const stop of [stopPrivacy, stopRuntime, stopAuthority, () => authority?.dispose(), () => timeline?.dispose()]) {
      try { stop() } catch { /* Release every owned lifetime even if a borrower fails. */ }
    }
  }
  signal.addEventListener('abort', dispose, { once: true })
  try {
    if (!isCurrent()) dispose()
    else {
      const stop = privacy!.registerCleanup(dispose)
      if (active) stopPrivacy = stop; else stop()
      if (isCurrent() && profile?.runtime) {
        const stop = profile.runtime.subscribe(dispose)
        if (active) stopRuntime = stop; else stop()
      }
    }
  } catch { dispose() }
  let qualification: ClockQualification | null = null
  try {
    if (isCurrent() && profile && (profile.classification === 'QUALIFIED_RUNTIME'
      || profile.classification === 'TEST_ONLY' && !import.meta.env.PROD)
      && typeof profile.evidence === 'string' && profile.evidence.trim().length > 0) {
      qualification = Object.freeze({ maxAgeMs: profile.maxAgeMs, uncertaintyMs: profile.uncertaintyMs, maxElapsedRate: profile.maxElapsedRate })
      timeline = makeTimeline(profile.maxObservationGapMs, signal)
    }
  } catch { dispose(); timeline = null }
  const requestStarted = timeline?.capture() ?? null
  return Object.freeze({ dispose,
    complete(source: ShellBootstrap): CommunityBootstrapBinding {
      // Capture only once, after decoded receipt of this original request.
      const responseReceived = !completed ? timeline?.capture() ?? null : null
      if (completed) throw Error('Bootstrap timing already consumed')
      completed = true
      try {
        if (qualification && isCurrent() && profile?.runtime) qualification = profile.runtime.qualify(source)
      } catch { dispose(); qualification = null }
      if (isCurrent() && timeline && requestStarted && responseReceived && qualification) {
        authority = createCommunityAuthority({ bootstrap: source, scope: captured!, privacy: privacy!, signal,
          timeline, requestStarted, responseReceived, qualification })
        if (!authority.clock.read()) { authority.dispose(); authority = null; timeline.dispose() }
        else {
          const stop = authority.clock.subscribe(dispose)
          if (active) stopAuthority = stop; else stop()
        }
      } else dispose()
      return Object.freeze({ source, scope: captured, signal, isCurrent,
        read: () => isCurrent() ? authority : null, dispose })
    },
  })
}
