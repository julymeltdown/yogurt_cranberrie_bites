import assert from 'node:assert/strict'
import test from 'node:test'
import { bindDefaultInputs, localInputDefine, parseDefaultConfiguration } from './configuration.mjs'

const now = Date.parse('2026-11-05T03:00:00+09:00'), rev = 'a'.repeat(40), binary = 'b'.repeat(40)
const event = '12345678-1234-1234-1234-123456789abc', owner = 'unit-only-private-control-value'
const origin = 'http://127.0.0.1:5173'
function fixture() {
  const input = { version: 'mcr-default-app/v2', servingConfig: '/private/unit-only/serving.json', runtimePacket: '/private/unit-only/runtime.json',
    servedRoot: '/private/unit-only/application', harnessRevision: rev, publishedRevision: rev, cases: ['feed-read'], timeoutMs: 30_000 }
  const serving = { version: 'mcr-in03-serving/v1', allocation: 'UNIT_DEFAULT_APP', origin, gatewayOrigin: 'http://127.0.0.1:63320',
    availabilityRevision: rev, frontendRevision: rev, runtimePacket: input.runtimePacket, activeUntil: '2026-11-04T18:10:00Z' }
  const roles = ['requester', 'recipient', 'restricted'], principals = Object.fromEntries(roles.map((role, i) => [role,
    { userId: `12345678-1234-1234-1234-12345678900${i}`, email: `${role}@example.test`, password: 'unit-only-never-a-runtime-principal' }]))
  const expiresAt = '2026-11-04T18:05:00Z'
  const packet = { browser_origin: origin, gateway_url: serving.gatewayOrigin, event_id: event, principals,
    runtime: { private: true, controlOwner: owner, origin, eventId: event, expiresAt },
    authProducer: { version: 'mcr-local-real-auth/v1', kind: 'CANONICAL_ENROLLMENT_GATEWAY_PRIVATE_HTTP_MAIL', sourceRevision: rev, binaryRevision: binary,
      tool: 'session_deletion_runtime/enrollment.py', operatorPolicyProductionApproval: false, mailOwner: owner, controlOwner: owner, origin, eventId: event,
      issuedAt: '2026-11-04T17:59:00Z', expiresAt, custody: 'PRIVATE_MODE_0600',
      actors: Object.fromEntries(roles.map(role => [role, { userId: principals[role].userId, email: principals[role].email, eventId: event,
        method: 'GATEWAY_ENROLLMENT_COOKIE_HTTP_MAIL_VERIFY_LOGIN', signupStatus: 202, verifyStatus: 200, loginStatus: 200, bootstrapStatus: 200 }])) },
    defaultApp: { applicationRelease: { version: 1, applicationVersion: '0.1.0', buildId: 'c'.repeat(64), sourceRevision: rev },
      frontendRevision: rev, localInputRevision: rev, localInput: { revision: 'unit-only' } } }
  return { input, serving, packet }
}
test('admits only a bounded explicit default-app subset, never provider/WS/full-gate flags', () => {
  const { input } = fixture(); assert(Object.isFrozen(parseDefaultConfiguration(input)))
  for (const changes of [{ cases: ['full-IN03'] }, { cases: ['age-positive'] }, { cases: ['feed-comment'] },
    { cases: ['feed-read', 'feed-read'] }, { version: 'mcr-default-app/v1' }, { timeoutMs: 180_001 }, { servedRoot: 'relative' }, { fakeBaseline: true }, { publishedRevision: '' }]) {
    assert.throws(() => parseDefaultConfiguration({ ...input, ...changes }))
  }
})
test('Enrollment provenance rejects legacy status normalization, production approval and invented identity claims', () => {
  const { input, serving, packet } = fixture()
  for (const changes of [{ kind: 'CANONICAL_GATEWAY_PRIVATE_HTTP_MAIL' }, { tool: 'world_in01_prepare.py' },
    { operatorPolicyProductionApproval: true }, { issuedAt: '2026-11-04T19:00:00Z' }, { expiresAt: '2026-11-04T17:00:00Z' },
    { actors: { ...packet.authProducer.actors, requester: { ...packet.authProducer.actors.requester, signupStatus: 200 } } },
    { actors: { ...packet.authProducer.actors, requester: { ...packet.authProducer.actors.requester, verifiedAge: true } } }]) {
    assert.throws(() => bindDefaultInputs(input, serving, { ...packet, authProducer: { ...packet.authProducer, ...changes } }, now))
  }
  assert.throws(() => bindDefaultInputs(input, serving, { ...packet, principals: { ...packet.principals,
    requester: { ...packet.principals.requester, accessToken: 'unit-only-forbidden-field' } } }, now))
  assert.throws(() => bindDefaultInputs(input, serving, { ...packet, principals: { ...packet.principals, recipient: packet.principals.requester } }, now))
})
test('release and explicit social resources are required, without interpreting third-role labels as age proof', () => {
  const { input, serving, packet } = fixture(), selected = { ...input, cases: ['social-ui'] }
  assert.throws(() => bindDefaultInputs(selected, serving, packet, now))
  const resources = { in03Social: { introId: '12345678-1234-1234-1234-123456789aab', platform: 'INSTAGRAM',
    requesterHandle: 'unit_requester', recipientHandle: 'unit_recipient' } }
  const admitted = bindDefaultInputs(selected, serving, { ...packet, resources }, now)
  assert.equal(admitted.social, resources.in03Social)
  assert(!Object.hasOwn(admitted, 'ageQualified'))
  for (const changes of [{ requesterHandle: '@invalid' }, { recipientHandle: resources.in03Social.requesterHandle },
    { platform: 'FAKE' }, { introId: 'not-an-id' }, { actualAge: 'VERIFIED' }]) {
    assert.throws(() => bindDefaultInputs(selected, serving, { ...packet, resources: { in03Social: { ...resources.in03Social, ...changes } } }, now))
  }
  assert.throws(() => bindDefaultInputs({ ...selected, cases: ['social-ui', 'sharing-unavailable'] }, serving, { ...packet, resources }, now))
  for (const release of [undefined, { ...packet.defaultApp.applicationRelease, sourceRevision: binary },
    { ...packet.defaultApp.applicationRelease, buildId: '' }, { ...packet.defaultApp.applicationRelease, qualified: true }]) {
    assert.throws(() => bindDefaultInputs(input, serving, { ...packet, defaultApp: { ...packet.defaultApp, applicationRelease: release } }, now))
  }
})
test('binds canonical private origin/event/producer and actual producer-supplied local configuration', () => {
  const { input, serving, packet } = fixture(), result = bindDefaultInputs(parseDefaultConfiguration(input), serving, packet, now)
  assert.equal(result.cutoff, Date.parse('2026-11-04T18:05:00Z')); assert.equal(result.localInput, packet.defaultApp.localInput)
  for (const changes of [{ defaultApp: undefined }, { defaultApp: { ...packet.defaultApp, frontendRevision: binary } },
    { defaultApp: { ...packet.defaultApp, providerPositive: true } }, { event_id: 'foreign' }, { gateway_url: origin }, { authProducer: null }]) {
    assert.throws(() => bindDefaultInputs(input, serving, { ...packet, ...changes }, now))
  }
  assert.throws(() => bindDefaultInputs({ ...input, cases: ['feed-read', 'feed-comment'] }, serving, packet, now))
})
test('startup requires cleanup headroom while live guard preserves the original expiry without early renewal', () => {
  const { input, serving, packet } = fixture(), late = Date.parse('2026-11-04T18:04:50Z')
  assert.throws(() => bindDefaultInputs(input, serving, packet, late))
  assert.equal(bindDefaultInputs(input, serving, packet, late, false).cutoff, Date.parse(packet.runtime.expiresAt))
  assert.throws(() => bindDefaultInputs(input, serving, packet, Date.parse(packet.runtime.expiresAt), false))
})
test('public define contains exactly validated producer data, with no default movement or environment fallback', () => {
  let received
  const value = { revision: 'unit-only', reviewedBy: 'unit-only', movementPolicy: { speedPxPerSecond: 1, footWidthPx: 1, footHeightPx: 1, interactionRadiusPx: 1 }, zoom: 1 }
  assert.equal(JSON.parse(localInputDefine(value, input => { received = input; return input })), JSON.stringify(value))
  assert.equal(received, value)
  assert.throws(() => localInputDefine(value, () => null)); assert.throws(() => localInputDefine(value, undefined))
})
