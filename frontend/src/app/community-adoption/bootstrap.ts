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
  let timeline: AuthorityTimeline | null = null, stopPrivacy = () => {}, stopAuthority = () => {}
  const captured = scope ? Object.freeze({ ...scope }) : null
  function isCurrent() { return active && !signal.aborted && !!captured && !!privacy && privacy.isCurrent(captured) }
  function dispose() {
    if (!active) return
    active = false; signal.removeEventListener('abort', dispose)
    for (const stop of [stopPrivacy, stopAuthority, () => authority?.dispose(), () => timeline?.dispose()]) {
      try { stop() } catch { /* Release every owned lifetime even if a borrower fails. */ }
    }
  }
  signal.addEventListener('abort', dispose, { once: true })
  try {
    if (!isCurrent()) dispose()
    else {
      const stop = privacy!.registerCleanup(dispose)
      if (active) stopPrivacy = stop; else stop()
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
  } catch { timeline?.dispose(); timeline = null }
  const requestStarted = timeline?.capture() ?? null
  return Object.freeze({ dispose,
    complete(source: ShellBootstrap): CommunityBootstrapBinding {
      // Capture only once, after decoded receipt of this original request.
      const responseReceived = !completed ? timeline?.capture() ?? null : null
      if (completed) throw Error('Bootstrap timing already consumed')
      completed = true
      if (isCurrent() && timeline && requestStarted && responseReceived && qualification) {
        authority = createCommunityAuthority({ bootstrap: source, scope: captured!, privacy: privacy!, signal,
          timeline, requestStarted, responseReceived, qualification })
        if (!authority.clock.read()) { authority.dispose(); authority = null; timeline.dispose() }
        else stopAuthority = authority.clock.subscribe(dispose)
      } else timeline?.dispose()
      return Object.freeze({ source, scope: captured, signal, isCurrent,
        read: () => isCurrent() ? authority : null, dispose })
    },
  })
}
