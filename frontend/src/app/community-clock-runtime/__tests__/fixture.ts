import { eventId } from '../../community-authority/__tests__/fixtures'
import type { ClockRuntimeEnvironment } from '..'
import type { CompiledClockInput } from '../input'

/** SYNTHETIC UNIT INPUT. Never a production measurement or evidence artifact. */
export function syntheticInput(): CompiledClockInput {
  const binding = { candidate: 'a'.repeat(40), environmentId: 'SYNTHETIC_ONLY', eventId,
    origin: 'https://community.test', bootstrapContractSha256: 'b'.repeat(64),
    concertBinarySha256: 'c'.repeat(64), serverClockSource: 'd'.repeat(40) }
  return { current: { ...binding }, measurement: {
    kind: 'MCR_C03_CLOCK_MEASUREMENT_V1', classification: 'MEASURED_RUNTIME', evidenceSha256: 'e'.repeat(64),
    binding, browser: { userAgent: 'SYNTHETIC_UNIT_BROWSER', platform: 'SYNTHETIC', crossOriginIsolated: false },
    samples: [
      { requestStartedMs: 0, responseReceivedMs: 100, serverTime: '2026-11-02T23:59:40Z' },
      { requestStartedMs: 10000, responseReceivedMs: 10100, serverTime: '2026-11-02T23:59:50Z' },
      { requestStartedMs: 20000, responseReceivedMs: 20100, serverTime: '2026-11-03T00:00:00Z' },
    ], observationsMs: [...Array.from({ length: 21 }, (_, i) => i * 1000), 20100], uninterrupted: true,
  } }
}
export function environment(input = syntheticInput()) {
  let nowMs = 100, origin = 1, interrupted = () => {}, currentOrigin = input.current.origin
  const browser = { ...input.measurement.browser }
  const value: ClockRuntimeEnvironment = { eventId, origin: () => currentOrigin, browser: () => browser,
    observe: () => ({ nowMs, origin }), onInterruption(fn) { interrupted = fn; return () => { interrupted = () => {} } } }
  return { value, browser, setTime(ms: number) { nowMs = ms }, setOrigin(n: number) { origin = n },
    setLocation(s: string) { currentOrigin = s }, interrupt() { interrupted() } }
}
