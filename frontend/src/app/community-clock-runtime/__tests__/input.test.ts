import { describe, expect, it } from 'vitest'
import { decodeCompiledClockInput, deriveClockLimits } from '../input'
import { syntheticInput } from './fixture'

describe('closed compiled clock input (synthetic unit data only)', () => {
  it('derives conservative limits from full request intervals and actual seconds precision', () => {
    const input = decodeCompiledClockInput(JSON.stringify(syntheticInput())), limits = deriveClockLimits(input.measurement)
    expect(limits.maxAgeMs).toBe(20100)
    expect(limits.maxObservationGapMs).toBe(1000)
    expect(limits.uncertaintyMs).toBe(100)
    expect(limits.maxElapsedRate).toBeCloseTo(10100 / 8000)
    expect(limits.serverValidUntilMs).toBe(Date.parse('2026-11-03T00:00:00Z') + 20100)
    expect(Object.isFrozen(input.measurement.samples[0])).toBe(true)
    expect(Object.isFrozen(input.measurement.observationsMs)).toBe(true)
  })
  it.each([
    ['extra grant', (x: any) => { x.role = 'admin' }],
    ['test label', (x: any) => { x.measurement.classification = 'TEST_ONLY' }],
    ['bare label', (x: any) => { x.measurement.classification = 'QUALIFIED_RUNTIME' }],
    ['candidate', (x: any) => { x.current.candidate = 'f'.repeat(40) }],
    ['binary', (x: any) => { x.current.concertBinarySha256 = 'f'.repeat(64) }],
    ['environment', (x: any) => { x.current.environmentId = 'changed' }],
    ['contract', (x: any) => { x.current.bootstrapContractSha256 = 'f'.repeat(64) }],
    ['server source', (x: any) => { x.current.serverClockSource = 'f'.repeat(40) }],
    ['insecure origin', (x: any) => { x.current.origin = x.measurement.binding.origin = 'http://example.com' }],
    ['control character', (x: any) => { x.measurement.browser.platform = 'bad\nplatform' }],
    ['fractional producer', (x: any) => { x.measurement.samples[0].serverTime = '2026-11-02T23:59:40.000Z' }],
    ['invalid date', (x: any) => { x.measurement.samples[0].serverTime = '2026-02-30T00:00:00Z' }],
    ['backward observations', (x: any) => { x.measurement.observationsMs[2] = -1 }],
    ['uncovered receipt', (x: any) => { x.measurement.observationsMs.pop() }],
    ['interrupted capture', (x: any) => { x.measurement.uninterrupted = false }],
    ['overlapping requests', (x: any) => { x.measurement.samples[1].requestStartedMs = 99 }],
    ['negative RTT', (x: any) => { x.measurement.samples[0].responseReceivedMs = -1 }],
    ['unbounded rate', (x: any) => { x.measurement.samples[2].serverTime = '2026-11-03T00:10:00Z' }],
    ['too few samples', (x: any) => { x.measurement.samples.pop() }],
    ['no usable server duration', (x: any) => { x.measurement.samples.forEach((s: any) => { s.serverTime = x.measurement.samples[0].serverTime }) }],
  ])('rejects %s', (_name, mutate) => {
    const input = structuredClone(syntheticInput()); mutate(input)
    expect(() => decodeCompiledClockInput(JSON.stringify(input))).toThrow()
  })
  it.each([undefined, '', '{}', '{', 'x'.repeat(512 * 1024 + 1)])('rejects absent or malformed input', raw => {
    expect(() => decodeCompiledClockInput(raw)).toThrow()
  })
})
