import { expect, it } from 'vitest'
import { policyVersionTuple, servicePolicyDecoder, servicePolicyPath, servicePolicySchema } from '../contract'
import { policyFixtureRelease, servicePolicyFixture } from './fixtures'
import type { PolicyAttempt } from '../model'

const binding: PolicyAttempt = { scope: { userId: '11111111-1111-4111-8111-111111111111', eventId: '22222222-2222-4222-8222-222222222222',
  generation: 2, policyGeneration: 3, gate: 'open' }, release: policyFixtureRelease, attempt: 1 }
const context = { status: 200, scope: binding.scope, path: servicePolicyPath(binding.scope.eventId) }

it('decodes every literal discriminant and does not equate status200 with availability', () => {
  const decoder = servicePolicyDecoder(binding, () => true)
  for (const [kind, expected] of [['NORMAL', 'normal'], ['MAINTENANCE', 'maintenance'], ['CLIENT_VERSION_REQUIRED', 'version'], ['UNAVAILABLE', 'unavailable']] as const) {
    expect(decoder(servicePolicyFixture(kind), context).kind).toBe(expected)
  }
})

it('rejects open objects, impossible revisions, missing fields, noncanonical versions and invalid timestamps', () => {
  const original = servicePolicyFixture()
  for (const patch of [{ extra: 'ignored' }, { kind: 'NORMAL_ISH' }, { policy_revision: null }, { minimum_client_version: null },
    { event_revision: 0 }, { event_revision: Number.MAX_SAFE_INTEGER + 1 }, { policy_revision: 0.5 },
    { minimum_client_version: '01.0.0' }, { minimum_client_version: '65536.0.0' }, { minimum_client_version: '1.2.3-beta' },
    { minimum_client_version: '１.2.3' }, { server_time: '2026-02-30T00:00:00Z' }, { server_time: '2026-11-04T00:00:00+00:00' },
    { valid_until: original.server_time }, { valid_until: new Date(Date.parse(original.server_time) + 30_001).toISOString() }]) {
    expect(servicePolicySchema.safeParse({ ...original, ...patch }).success).toBe(false)
  }
  const { valid_until: omitted, ...missing } = original
  expect(omitted).toBeTruthy(); expect(servicePolicySchema.safeParse(missing).success).toBe(false)
})

it('binds the HTTP status/path/Auth event and generation and withdraws a changed privacy attempt', () => {
  let current = true
  const decoder = servicePolicyDecoder(binding, () => current), wire = servicePolicyFixture()
  for (const wrong of [{ ...context, status: 503 }, { ...context, path: '/another/event' },
    { ...context, scope: { ...context.scope, generation: 3 } }, { ...context, scope: { ...context.scope, eventId: binding.scope.userId } },
    { ...context, scope: { ...context.scope, userId: binding.scope.eventId } }]) expect(() => decoder(wire, wrong)).toThrow()
  current = false; expect(() => decoder(wire, context)).toThrow()
})

it('compares numeric tuples and rejects a server decision inconsistent with its captured client version', () => {
  const newer = servicePolicyDecoder({ ...binding, release: { ...binding.release, applicationVersion: '0.10.0' } }, () => true)
  expect(newer(servicePolicyFixture('NORMAL', { minimum: '0.2.0' }), context).kind).toBe('normal')
  expect(() => newer(servicePolicyFixture('CLIENT_VERSION_REQUIRED', { minimum: '0.2.0' }), context)).toThrow()
  const older = servicePolicyDecoder(binding, () => true)
  expect(() => older(servicePolicyFixture('NORMAL', { minimum: '0.2.0' }), context)).toThrow()
  expect(policyVersionTuple('65535.65535.65535')).toEqual([65535, 65535, 65535])
  expect(policyVersionTuple('000.1.0')).toBeNull()
})

it('never extends a decision window when RFC3339 precision exceeds milliseconds', () => {
  const decoder = servicePolicyDecoder(binding, () => true)
  const result = decoder({ ...servicePolicyFixture(), server_time: '2026-11-04T00:00:00.999999Z',
    valid_until: '2026-11-04T00:00:05.000001Z' }, context)
  expect(result.validUntilMs - result.serverTimeMs).toBe(4_000)
})

it('rejects terminal line separators rather than accepting the permissive dollar anchor', () => {
  const decoder = servicePolicyDecoder(binding, () => true)
  for (const suffix of ['\n', '\r', '\r\n', '\u2028', '\u2029', ' ']) {
    const malformed = '0.1.0' + suffix
    expect(policyVersionTuple(malformed)).toBeNull()
    expect(() => decoder({ ...servicePolicyFixture(), minimum_client_version: malformed }, context)).toThrow()
  }
})
