import type { ShellBootstrap } from '../ports'
import type { MeetingClockPort } from '../meeting-entry/ports'
import type { SessionController } from '../../features/auth/model/session'
import { canonicalUuid, decodeContract, object } from '../../features/auth/model/contract'
import { placeSchema, sourceLink, type Place } from '../../features/places/model/contract'
import type { PrivacyScope } from '../../shared/query/privacy'
import type { AuthorityTimeline, PerformanceObservation } from './timeline'
export { createAuthorityTimeline, createBrowserAuthorityTimeline } from './timeline'
export type { AuthorityTimeline, PerformanceObservation, TimelineOptions } from './timeline'

export interface ClockQualification {
  /** Maximum retained request/anchor age. Requalification requires another canonical response. */
  readonly maxAgeMs: number
  /** Explicit combined timestamp/clock-resolution uncertainty, not an inferred browser accuracy claim. */
  readonly uncertaintyMs: number
  /** Qualified upper ratio of elapsed server time to observed performance duration (>= 1). */
  readonly maxElapsedRate: number
}
export interface CommunityAuthorityOptions {
  readonly bootstrap: ShellBootstrap
  readonly scope: PrivacyScope
  readonly privacy: Pick<SessionController['privacy'], 'isCurrent' | 'registerCleanup'>
  readonly signal: AbortSignal
  readonly timeline: AuthorityTimeline
  readonly requestStarted: PerformanceObservation
  readonly responseReceived: PerformanceObservation
  readonly qualification: ClockQualification
}
/** Original canonical fields retained. `id` is exactly place_id for existing authoring consumers. */
export type PublicPlaceChoice = Readonly<Place & { readonly id: string }>
export interface PublicPlaceProjection {
  readonly scope: PrivacyScope
  readonly source: Readonly<{ mapId: string; mapRevision: number; contentHash: string }>
  readonly items: readonly PublicPlaceChoice[]
}
export interface ServerClockInterval {
  readonly scope: PrivacyScope
  readonly lowerMs: number
  readonly upperMs: number
  readonly roundTripMs: number
}
const frozen = (value: unknown, seen = new Set<object>()): boolean => {
  if (value === null || typeof value !== 'object') return true
  if (seen.has(value)) return true
  seen.add(value)
  return Object.isFrozen(value) && Object.values(value).every(child => frozen(child, seen))
}
const validScope = (scope: PrivacyScope) => canonicalUuid(scope.userId) && canonicalUuid(scope.eventId)
  && Number.isSafeInteger(scope.generation) && scope.generation >= 0 && Number.isSafeInteger(scope.policyGeneration)
  && scope.policyGeneration >= 0 && scope.gate === 'open'

/** Qualified same-scope projections only; this leaf issues no request or physical-arrival permission. */
export function createCommunityAuthority(options: CommunityAuthorityOptions) {
  const scope: PrivacyScope = Object.freeze({ userId: options.scope.userId, eventId: options.scope.eventId,
    generation: options.scope.generation, policyGeneration: options.scope.policyGeneration, gate: options.scope.gate })
  const policy = Object.freeze({ ...options.qualification })
  const listeners = new Set<() => void>(), stops: (() => void)[] = []
  let active = true, timer: ReturnType<typeof setTimeout> | undefined
  let serverMs = Number.NaN, timestampQuantumMs = 0
  let projection: PublicPlaceProjection | null = null, choices: PublicPlaceProjection | null = null
  function dispose() {
    if (!active) return
    active = false; projection = null; choices = null; clearTimeout(timer)
    options.signal.removeEventListener('abort', dispose)
    for (const stop of stops.splice(0).reverse()) { try { stop() } catch { /* Release every borrowed subscription. */ } }
    for (const fn of Array.from(listeners)) { try { fn() } catch { /* Notify every consumer on withdrawal. */ } }
    listeners.clear()
  }
  function live() {
    try { return active && !options.signal.aborted && options.privacy.isCurrent(scope) }
    catch { return false }
  }
  function readInterval(): ServerClockInterval | null {
    if (!live()) { dispose(); return null }
    const timing = options.timeline.measure(options.requestStarted, options.responseReceived)
    if (!timing || !live() || timing.sinceStartMs >= policy.maxAgeMs) { dispose(); return null }
    // Generation occurred somewhere inside the FULL request interval, never at receipt alone.
    const uncertainty = timestampQuantumMs + policy.uncertaintyMs
    const upperMs = Math.ceil(serverMs + uncertainty + timing.sinceStartMs * policy.maxElapsedRate)
    const lowerMs = Math.max(0, Math.floor(serverMs - uncertainty + timing.sinceReceiptMs / policy.maxElapsedRate))
    if (!Number.isSafeInteger(upperMs) || !Number.isSafeInteger(lowerMs) || lowerMs > upperMs) { dispose(); return null }
    return Object.freeze({ scope, lowerMs, upperMs, roundTripMs: timing.roundTripMs })
  }
  function subscribe(listener: () => void) {
    if (!active) return () => {}
    const entry = () => listener(); listeners.add(entry); return () => { listeners.delete(entry) }
  }
  try {
    if (!validScope(scope) || !live() || !Number.isFinite(policy.maxAgeMs) || policy.maxAgeMs < 1 || policy.maxAgeMs > 2_147_483_647
      || !Number.isFinite(policy.uncertaintyMs) || policy.uncertaintyMs < 0 || policy.uncertaintyMs > 60_000
      || !Number.isFinite(policy.maxElapsedRate) || policy.maxElapsedRate < 1 || policy.maxElapsedRate > 2) throw Error('Unqualified input')
    const bootstrap = options.bootstrap
    if (!Object.isFrozen(bootstrap) || !bootstrap.document || !frozen(bootstrap.document)
      || bootstrap.contractVersion !== '0.1.0' || bootstrap.event.eventId !== scope.eventId
      || bootstrap.participant.eventId !== scope.eventId || bootstrap.participant.userId !== scope.userId) throw Error('Unbound bootstrap')
    const document = decodeContract('Bootstrap', bootstrap.document), event = object(document.event), participant = object(document.participant)
    if (event.event_id !== scope.eventId || participant.event_id !== scope.eventId || participant.user_id !== scope.userId) throw Error('Unbound document')
    const timestamp = document.server_time
    if (typeof timestamp !== 'string' || !/^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:\.\d+)?Z$/.test(timestamp)) throw Error('Unknown server time')
    serverMs = Date.parse(timestamp)
    if (!Number.isSafeInteger(serverMs) || serverMs < 0 || new Date(serverMs).toISOString().slice(0, 19) !== timestamp.slice(0, 19)) throw Error('Invalid server time')
    timestampQuantumMs = 10 ** (3 - Math.min(3, timestamp.match(/\.(\d+)Z$/)?.[1]?.length ?? 0))
    const map = object(document.map_manifest)
    if (!Array.isArray(map.places)) throw Error('Missing canonical places')
    const items = map.places.map(value => {
      const p = placeSchema.parse(value)
      return Object.freeze({ ...p, id: p.place_id, provenance: Object.freeze({ ...p.provenance }) })
    })
    if (new Set(items.map(p => p.id)).size !== items.length) throw Error('Duplicate canonical places')
    const source = Object.freeze({ mapId: String(map.map_id), mapRevision: Number(map.map_revision), contentHash: String(map.content_hash) })
    projection = Object.freeze({ scope, source, items: Object.freeze(items) })
    // Label-only downstream forms must never offer a virtual/unreviewed destination as a physical meeting place.
    choices = Object.freeze({ scope, source, items: Object.freeze(items.filter(p => p.place_kind === 'PHYSICAL'
      && p.verification_state === 'VERIFIED' && p.closure_state === 'OPEN' && p.provenance.review_state === 'REVIEWED'
      && sourceLink(p.provenance.source_uri) !== null)) })
    const register = (work: () => () => void) => { const stop = work(); if (active) stops.push(stop); else stop() }
    register(() => options.privacy.registerCleanup(dispose))
    register(() => options.timeline.subscribe(dispose))
    if (live()) options.signal.addEventListener('abort', dispose, { once: true })
    const timing = options.timeline.measure(options.requestStarted, options.responseReceived)
    if (!readInterval() || !timing) dispose()
    else timer = setTimeout(dispose, Math.max(1, policy.maxAgeMs - timing.sinceStartMs))
  } catch { dispose() }
  const clock: MeetingClockPort = Object.freeze({ read: () => { const i = readInterval(); return i ? Object.freeze({ scope, nowMs: i.upperMs }) : null }, subscribe })
  return Object.freeze({ clock, readInterval,
    /** All actual public map facts, including virtual concepts. Grants no physical permission or per-place revision. */
    publicPlaces: Object.freeze({ read: () => readInterval() ? projection : null, subscribe }),
    /** Conservative reviewed PHYSICAL choices compatible with CommunityEntryRelease.places. */
    places: Object.freeze({ read: () => readInterval() ? choices : null, subscribe }),
    dispose,
  })
}
export type CommunityAuthority = ReturnType<typeof createCommunityAuthority>
