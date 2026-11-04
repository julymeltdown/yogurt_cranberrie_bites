import { canonicalUuid } from '../../features/auth/model/contract'

export interface ClockBinding {
  readonly candidate: string
  readonly environmentId: string
  readonly eventId: string
  readonly origin: string
  readonly bootstrapContractSha256: string
  readonly concertBinarySha256: string
  readonly serverClockSource: string
}
export interface ClockBrowser {
  readonly userAgent: string
  readonly platform: string
  readonly crossOriginIsolated: boolean
}
export interface ClockSample {
  readonly requestStartedMs: number
  readonly responseReceivedMs: number
  readonly serverTime: string
}
export interface ClockMeasurement {
  readonly kind: 'MCR_C03_CLOCK_MEASUREMENT_V1'
  readonly classification: 'MEASURED_RUNTIME'
  readonly evidenceSha256: string
  readonly binding: ClockBinding
  readonly browser: ClockBrowser
  readonly samples: readonly ClockSample[]
  readonly observationsMs: readonly number[]
  readonly uninterrupted: true
}
/** Root supplies current independently from the captured measurement binding.
 * Both byte sequences belong in the NEW application build-input catalog. */
export interface CompiledClockInput {
  readonly current: ClockBinding
  readonly measurement: ClockMeasurement
}
export interface DerivedClockLimits {
  readonly maxAgeMs: number
  readonly uncertaintyMs: number
  readonly maxElapsedRate: number
  readonly maxObservationGapMs: number
  readonly serverValidUntilMs: number
}
const object = (v: unknown): v is Record<string, unknown> => !!v && typeof v === 'object' && !Array.isArray(v)
function need(v: unknown): asserts v { if (!v) throw Error('COMMUNITY_CLOCK_INPUT_UNQUALIFIED') }
function closed(v: unknown, keys: readonly string[]): asserts v is Record<string, unknown> {
  need(object(v) && Object.keys(v).sort().join(',') === [...keys].sort().join(','))
}
const hex = (v: unknown, length: number): v is string => typeof v === 'string' && new RegExp(`^[a-f0-9]{${length}}$`).test(v)
const finite = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v) && v >= 0 && v <= Number.MAX_SAFE_INTEGER
const text = (v: unknown, max: number): v is string => typeof v === 'string' && v.length > 0 && v.length <= max
  && Array.from(v).every(c => c.charCodeAt(0) >= 32 && c.charCodeAt(0) !== 127)
export function decodeClockBinding(value: unknown): ClockBinding {
  closed(value, ['candidate', 'environmentId', 'eventId', 'origin', 'bootstrapContractSha256', 'concertBinarySha256', 'serverClockSource'])
  need(hex(value.candidate, 40) && text(value.environmentId, 128) && canonicalUuid(value.eventId)
    && hex(value.bootstrapContractSha256, 64) && hex(value.concertBinarySha256, 64) && hex(value.serverClockSource, 40))
  need(text(value.origin, 256))
  const origin = new URL(value.origin)
  need(origin.origin === value.origin && (origin.protocol === 'https:' || origin.protocol === 'http:' && ['localhost', '127.0.0.1', '[::1]'].includes(origin.hostname)))
  return Object.freeze(value as unknown as ClockBinding)
}
export function decodeClockBrowser(value: unknown): ClockBrowser {
  closed(value, ['userAgent', 'platform', 'crossOriginIsolated'])
  need(text(value.userAgent, 512) && text(value.platform, 128) && typeof value.crossOriginIsolated === 'boolean')
  return Object.freeze(value as unknown as ClockBrowser)
}
export function sameClockBinding(a: ClockBinding, b: ClockBinding): boolean {
  return Object.keys(a).every(key => a[key as keyof ClockBinding] === b[key as keyof ClockBinding])
}
export function sameClockBrowser(a: ClockBrowser, b: ClockBrowser): boolean {
  return a.userAgent === b.userAgent && a.platform === b.platform && a.crossOriginIsolated === b.crossOriginIsolated
}
/** Current C03 source emits SecondsFormat::Secs. Reject a silently changed
 * producer precision rather than reusing a previously qualified model. */
export function secondsTimestamp(value: unknown): number {
  need(typeof value === 'string' && /^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ$/.test(value))
  const ms = Date.parse(value)
  need(Number.isSafeInteger(ms) && ms >= 0 && new Date(ms).toISOString() === value.slice(0, -1) + '.000Z')
  return ms
}
/** Bounds are conditional on this recorded environment and finite exercised
 * duration. No platform-wide drift/accuracy guarantee is inferred. */
export function deriveClockLimits(measurement: ClockMeasurement): DerivedClockLimits {
  const { samples, observationsMs } = measurement
  need(samples.length >= 3 && samples.length <= 64 && observationsMs.length >= 3 && observationsMs.length <= 4096)
  let previous = -1, gap = 0
  for (const at of observationsMs) {
    need(finite(at) && at >= previous)
    if (previous >= 0) gap = Math.max(gap, at - previous)
    previous = at
  }
  const first = samples[0]!, last = samples.at(-1)!
  need(observationsMs[0]! <= first.requestStartedMs && observationsMs.at(-1)! >= last.responseReceivedMs)
  let previousReceipt = -1, previousServer = -1, maxRtt = 0
  const server = samples.map(sample => {
    closed(sample, ['requestStartedMs', 'responseReceivedMs', 'serverTime'])
    need(finite(sample.requestStartedMs) && finite(sample.responseReceivedMs) && sample.requestStartedMs >= previousReceipt && sample.responseReceivedMs > sample.requestStartedMs)
    const time = secondsTimestamp(sample.serverTime)
    need(time >= previousServer)
    maxRtt = Math.max(maxRtt, sample.responseReceivedMs - sample.requestStartedMs)
    previousReceipt = sample.responseReceivedMs; previousServer = time
    return time
  })
  // Each timestamp's quantization is bounded by its actual one-second format.
  // Use FULL disjoint request intervals; no RTT midpoint or local calendar.
  let rate = 1, pairs = 0
  for (let i = 0; i < samples.length; i++) for (let j = i + 1; j < samples.length; j++) {
    const elapsed = server[j]! - server[i]!
    if (elapsed <= 2000) continue
    const minimum = samples[j]!.requestStartedMs - samples[i]!.responseReceivedMs
    const maximum = samples[j]!.responseReceivedMs - samples[i]!.requestStartedMs
    need(minimum > 0)
    const upper = (elapsed + 2000) / minimum, lower = (elapsed - 2000) / maximum
    need(lower > 0)
    rate = Math.max(rate, upper, 1 / lower); pairs++
  }
  const maxAgeMs = Math.floor(last.responseReceivedMs - first.requestStartedMs)
  const maxObservationGapMs = Math.ceil(gap), uncertaintyMs = Math.ceil(maxRtt)
  need(pairs > 0 && rate <= 2 && maxAgeMs >= 1 && maxAgeMs <= 2_147_483_647
    && maxObservationGapMs >= 2 && maxObservationGapMs < maxAgeMs / 2 && uncertaintyMs <= 60_000)
  const serverValidUntilMs = server.at(-1)! + maxAgeMs
  need(Number.isSafeInteger(serverValidUntilMs))
  return Object.freeze({ maxAgeMs, uncertaintyMs, maxElapsedRate: rate, maxObservationGapMs, serverValidUntilMs })
}
export function decodeCompiledClockInput(raw: unknown): CompiledClockInput {
  need(typeof raw === 'string' && raw.length <= 512 * 1024)
  const value: unknown = JSON.parse(raw)
  closed(value, ['current', 'measurement'])
  const current = decodeClockBinding(value.current), m = value.measurement
  closed(m, ['kind', 'classification', 'evidenceSha256', 'binding', 'browser', 'samples', 'observationsMs', 'uninterrupted'])
  need(m.kind === 'MCR_C03_CLOCK_MEASUREMENT_V1' && m.classification === 'MEASURED_RUNTIME' && m.uninterrupted === true && hex(m.evidenceSha256, 64)
    && Array.isArray(m.samples) && Array.isArray(m.observationsMs))
  const binding = decodeClockBinding(m.binding), browser = decodeClockBrowser(m.browser)
  need(sameClockBinding(current, binding))
  const measurement = Object.freeze({ ...m, binding, browser,
    samples: Object.freeze(m.samples.map(sample => Object.freeze({ ...sample }))), observationsMs: Object.freeze([...m.observationsMs]),
  }) as unknown as ClockMeasurement
  deriveClockLimits(measurement)
  return Object.freeze({ current, measurement })
}
