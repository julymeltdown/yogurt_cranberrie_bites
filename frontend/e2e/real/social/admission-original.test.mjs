import assert from 'node:assert/strict'
import { chmodSync, mkdtempSync, rmSync, symlinkSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { tmpdir } from 'node:os'
import test from 'node:test'
import { admitLocalAuthProducer, admitOriginal, readRunConfig } from './admission.mjs'

const uuid = n => `9ab386e2-a82a-41e4-829d-${String(n).padStart(12, '0')}`
function localInput() {
  const now = Date.now(), expiresAt = new Date(now + 120_000).toISOString(), eventId = uuid(10)
  const principals = Object.fromEntries(['requester', 'recipient', 'restricted'].map((role, index) => [role,
    { userId: uuid(index + 1), email: `in03-${role}@example.test`, password: 'private-unit-canary' }]))
  const owner = 'private-unit-control-owner', origin = 'http://127.0.0.1:43127'
  return { mode: 'real', origin, eventId, qualifiedRefresh: false, availabilityModule: '/src/app/social-production.ts', availabilityRevision: 'a'.repeat(40),
    source: { frontendRevision: 'a'.repeat(40), backendRevision: 'b'.repeat(40), contractRevision: 'c'.repeat(40), authProducerRevision: 'd'.repeat(40), authBinaryRevision: 'e'.repeat(40) },
    principals, resources: { introId: uuid(20), postId: uuid(21), expiredRequestId: uuid(22) }, denials: { unrelated: [403], expired: [410], revoked: [403] },
    runtime: { controlOwner: owner, origin, eventId, expiresAt, private: true },
    authProducer: { version: 'mcr-local-real-auth/v1', kind: 'CANONICAL_GATEWAY_PRIVATE_HTTP_MAIL', sourceRevision: 'd'.repeat(40), binaryRevision: 'e'.repeat(40),
      tool: 'world_in01_prepare.py', mailOwner: owner, controlOwner: owner, origin, eventId, issuedAt: new Date(now - 1000).toISOString(), expiresAt, custody: 'PRIVATE_MODE_0600',
      actors: Object.fromEntries(Object.entries(principals).map(([role, principal]) => [role, { userId: principal.userId, email: principal.email, eventId,
        method: 'GATEWAY_SIGNUP_HTTP_MAIL_VERIFY_LOGIN', signupStatus: 200, verifyStatus: 200, loginStatus: 200, bootstrapStatus: 200 }])) } }
}

test('versioned canonical local mail inputs pass preflight only and remain unqualified for IN03', () => {
  const input = localInput(), result = admitOriginal(input)
  assert.equal(admitLocalAuthProducer(input), true)
  assert.equal(result.status, 'INPUTS_PRESENT')
  assert.equal(result.IN03, 'NOT_RUN')
  assert.ok(result.unavailable.length > 0)
  for (const secret of ['private-unit-canary', 'private-unit-control-owner', 'in03-requester@example.test']) assert.ok(!JSON.stringify(result).includes(secret))
})

test('look-real email, SMTP claims and fabricated principal markers cannot replace canonical provenance', () => {
  for (const mutate of [
    input => { delete input.authProducer },
    input => { input.authProducer.kind = 'CANONICAL_GATEWAY_LOCAL_SMTP' },
    input => { input.authProducer.actors.requester.method = 'SEEDED_JWT' },
    input => { input.authProducer.issuer = 'test-issuer' },
    input => { input.principals.requester.email = 'fan@real-looking-domain.com' },
    input => { input.principals.requester.email = input.authProducer.actors.requester.email = 'synthetic@example.test' },
    input => { input.principals.requester.accessToken = 'private-token-canary' },
    input => { input.principals.requester.jwt = 'seeded-token-canary' },
  ]) {
    const input = localInput(); mutate(input)
    assert.equal(admitOriginal(input).status, 'NOT_RUN')
  }
})

test('foreign actors, events, origins, source pins, custody and expired runtime are rejected', () => {
  const mutations = [
    input => { input.authProducer.actors.requester.userId = uuid(99) },
    input => { input.authProducer.actors.requester.eventId = uuid(99) },
    input => { input.authProducer.actors.requester.email = 'other@example.test' },
    input => { input.authProducer.eventId = uuid(99) },
    input => { input.authProducer.origin = 'http://127.0.0.1:43128' },
    input => { input.authProducer.sourceRevision = 'f'.repeat(40) },
    input => { input.authProducer.binaryRevision = 'f'.repeat(40) },
    input => { input.authProducer.mailOwner = 'foreign-private-owner' },
    input => { input.runtime.controlOwner = 'foreign-private-owner' },
    input => { input.runtime.private = false },
    input => { input.authProducer.custody = 'PUBLIC' },
    input => { input.authProducer.expiresAt = new Date(Date.now() - 1000).toISOString() },
    input => { input.authProducer.issuedAt = new Date(Date.now() + 1000).toISOString() },
    input => { input.authProducer.actors.recipient = { ...input.authProducer.actors.requester } },
  ]
  for (const mutate of mutations) {
    const input = localInput(); mutate(input)
    assert.equal(admitLocalAuthProducer(input), false)
    assert.equal(admitOriginal(input).status, 'NOT_RUN')
  }
})

test('non-200 signup, verification, login or bootstrap statuses never qualify actor preflight', () => {
  for (const status of [201, 401, 403, 503, '200']) for (const key of ['signupStatus', 'verifyStatus', 'loginStatus', 'bootstrapStatus']) {
    const input = localInput(); input.authProducer.actors.requester[key] = status
    assert.equal(admitLocalAuthProducer(input), false)
  }
})

test('canonical Python UTC precision is accepted and no independent 900-second renewal gate is invented', () => {
  const input = localInput(), now = Date.now()
  input.authProducer.issuedAt = new Date(now - 1000).toISOString().replace('Z', '123+00:00')
  input.authProducer.expiresAt = input.runtime.expiresAt = new Date(now + 1_800_000).toISOString().replace('Z', '+00:00')
  assert.equal(admitLocalAuthProducer(input, now), true)
  input.authProducer.expiresAt = input.runtime.expiresAt = new Date(now - 1).toISOString()
  assert.equal(admitLocalAuthProducer(input, now), false)
})

test('private configuration rejects public permissions and symlink custody', () => {
  const directory = mkdtempSync(join(tmpdir(), 'in03-private-'))
  try {
    const path = join(directory, 'private.json'), linked = join(directory, 'linked.json')
    writeFileSync(path, JSON.stringify(localInput()), { mode: 0o600 })
    assert.equal(readRunConfig(path).mode, 'real')
    symlinkSync(path, linked)
    assert.throws(() => readRunConfig(linked), { message: 'NOT_RUN: private real configuration unreadable' })
    chmodSync(path, 0o644)
    assert.throws(() => readRunConfig(path), { message: 'NOT_RUN: private real configuration unreadable' })
  } finally { rmSync(directory, { recursive: true }) }
})

test('missing original inputs remain NOT_RUN without requiring an unrelated approved request', () => {
  const result = admitOriginal(undefined)
  assert.equal(result.status, 'NOT_RUN')
  assert.equal(result.IN03, 'NOT_RUN')
  assert.ok(result.missing.some(value => value.input === 'server-expired request identity'))
  assert.ok(!result.missing.some(value => value.input === 'admitted approvedRequestId'))
})

test('dependency outage cannot be admitted as authorization-denial evidence; diagnostics stay redacted', () => {
  const privateValue = 'unit-redaction-canary'
  const result = admitOriginal({ principals: { requester: { email: privateValue, password: privateValue } },
    denials: { unrelated: [503], expired: [200], revoked: [] } })
  assert.equal(result.status, 'NOT_RUN')
  for (const kind of ['unrelated', 'expired', 'revoked'])
    assert.ok(result.missing.some(value => value.input === `contract-qualified ${kind} denial statuses`))
  assert.ok(!JSON.stringify(result).includes(privateValue))
})

test('malformed private JSON cannot expose its content in the thrown diagnostic', () => {
  const directory = mkdtempSync(join(tmpdir(), 'in03-admission-'))
  try {
    const path = join(directory, 'private.json')
    writeFileSync(path, '{"password":"unit-redaction-canary",', { mode: 0o600 })
    assert.throws(() => readRunConfig(path), { message: 'NOT_RUN: private real configuration unreadable' })
  } finally { rmSync(directory, { recursive: true }) }
})
