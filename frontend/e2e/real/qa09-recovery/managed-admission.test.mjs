import test from 'node:test'
import assert from 'node:assert/strict'
import { contractPins, createManagedAdmissionGuard } from './managed-admission.mjs'
import { observeManagedRecoveryCase } from './journey.mjs'

const id = digit => `${digit.repeat(8)}-${digit.repeat(4)}-${digit.repeat(4)}-${digit.repeat(4)}-${digit.repeat(12)}`
function fixture() {
  const controller = new AbortController()
  const config = { version: 1, isolated: true, allowDestructiveFixtureEndState: true,
    releaseRevision: 'a'.repeat(40), environmentId: id('1'), eventId: id('2'), origin: 'http://127.0.0.1:4000',
    contractHashes: { domain: contractPins.domain, realtime: contractPins.realtime },
    members: [{ userId: id('3'), password: 'private fixture member A' }, { userId: id('4') }],
    operator: { userId: id('5'), password: 'private fixture operator' } }
  const proof = { isolated: true, legacyData: false, exclusiveEvent: true,
    candidate: config.releaseRevision, environmentId: config.environmentId, eventId: config.eventId,
    domainHash: contractPins.domain, realtimeHash: contractPins.realtime,
    expiresAt: new Date(Date.now() + 60_000).toISOString(),
    actors: [...config.members.map(({ userId }) => ({ userId, role: 'MEMBER' })), { userId: id('5'), role: 'OPERATOR' }],
    services: ['gateway', 'auth', 'profile', 'concert', 'world', 'postgres'].map(name =>
      ({ name, ready: true, candidate: config.releaseRevision })), privateDiagnostic: 'MUST NOT ESCAPE' }
  let calls = 0, url = config.origin + '/gifts/pickups/' + id('6')
  const runtime = { async inspect(actual) { calls++; assert.equal(actual, config); return proof } }
  const args = { config, runtime, page: { url: () => url }, signal: controller.signal, deadlineEpochMs: Date.now() + 120_000 }
  return { config, proof, runtime, controller, args, calls: () => calls, navigate: value => { url = value } }
}

test('common managed proof binds without mural-only fields and cannot accept QA09', async () => {
  const f = fixture(), guard = createManagedAdmissionGuard(f.args)
  const result = await guard()
  assert.deepEqual(result, { managedObservationBound: true, contractRevision: contractPins.revision,
    businessCaseQualified: false, wholeQa09Accepted: false })
  assert.equal(f.calls(), 1)
  for (const value of ['MUST NOT ESCAPE', 'private fixture', f.config.eventId, f.config.operator.userId, f.config.origin])
    assert(!JSON.stringify(result).includes(value))
})

test('old contract, nonisolated config and ambiguous principals refuse before inspect', () => {
  for (const change of [f => { f.config.contractHashes.domain = 'b'.repeat(64) },
    f => { f.config.contractHashes.realtime = 'c'.repeat(64) }, f => { f.config.isolated = false },
    f => { f.config.members[1].userId = f.config.members[0].userId },
    f => { f.config.operator.userId = id('0') }, f => { f.config.origin += '/path' }]) {
    const f = fixture(); change(f)
    assert.throws(() => createManagedAdmissionGuard(f.args), /^Error: QA09_MANAGED_ADMISSION_UNAVAILABLE$/)
    assert.equal(f.calls(), 0)
  }
})

test('foreign candidate/event, role confusion, duplicate or unready services fail closed', async () => {
  for (const change of [f => { f.proof.candidate = 'b'.repeat(40) }, f => { f.proof.eventId = id('7') },
    f => { f.proof.environmentId = id('8') }, f => { f.proof.domainHash = 'c'.repeat(64) },
    f => { f.proof.exclusiveEvent = false }, f => { f.proof.legacyData = true },
    f => { f.proof.actors[2].role = 'MEMBER' }, f => { f.proof.actors[1] = f.proof.actors[0] },
    f => { f.proof.services[1] = f.proof.services[0] }, f => { f.proof.services[3].ready = false },
    f => { f.proof.services[3].candidate = 'd'.repeat(40) }]) {
    const f = fixture(), guard = createManagedAdmissionGuard(f.args); change(f)
    await assert.rejects(guard, /^Error: QA09_MANAGED_ADMISSION_UNAVAILABLE$/)
    assert.equal(f.calls(), 1)
  }
})

test('page navigation, changed config and resource abort prevent subsequent inspect', async () => {
  for (const change of [f => f.navigate('https://foreign.invalid/'),
    f => { f.config.eventId = id('8') }, f => { f.config.operator.userId = id('9') },
    f => f.controller.abort(new Error('PRIVATE ABORT DETAIL'))]) {
    const f = fixture(), guard = createManagedAdmissionGuard(f.args)
    await guard(); change(f)
    await assert.rejects(guard, /^Error: QA09_MANAGED_ADMISSION_UNAVAILABLE$/)
    assert.equal(f.calls(), 1)
  }
})

test('expired, invalid and unbounded proof windows fail', async () => {
  for (const expiresAt of [new Date(Date.now() - 1000).toISOString(), 'not a date',
    new Date(Date.now() + 1_800_000).toISOString()]) {
    const f = fixture(), guard = createManagedAdmissionGuard(f.args); f.proof.expiresAt = expiresAt
    await assert.rejects(guard, /^Error: QA09_MANAGED_ADMISSION_UNAVAILABLE$/)
  }
  const f = fixture(); f.args.deadlineEpochMs = Date.now() - 1
  assert.throws(() => createManagedAdmissionGuard(f.args), /QA09_MANAGED_ADMISSION_UNAVAILABLE/)
})

test('a new proof cannot renew the first admitted observation deadline', async t => {
  const f = fixture(), now = Date.now()
  t.mock.method(Date, 'now', () => now)
  f.args.deadlineEpochMs = now + 120_000
  f.proof.expiresAt = new Date(now + 1000).toISOString()
  const guard = createManagedAdmissionGuard(f.args)
  await guard()
  t.mock.method(Date, 'now', () => now + 1001)
  f.proof.expiresAt = new Date(now + 90_000).toISOString()
  await assert.rejects(guard, /^Error: QA09_MANAGED_ADMISSION_UNAVAILABLE$/)
  assert.equal(f.calls(), 1)
})

test('abort or custody change while inspect is pending cannot commit its proof', async () => {
  for (const change of [f => f.controller.abort(), f => f.navigate('https://foreign.invalid/'),
    f => { f.config.releaseRevision = 'b'.repeat(40) }]) {
    const f = fixture(); let resolve
    f.runtime.inspect = () => new Promise(done => { resolve = done })
    const guard = createManagedAdmissionGuard(f.args), pending = guard()
    change(f); resolve(f.proof)
    await assert.rejects(pending, /^Error: QA09_MANAGED_ADMISSION_UNAVAILABLE$/)
  }
})

test('owner failure stays sanitized and is not retried', async () => {
  const f = fixture(); let calls = 0
  f.runtime.inspect = async () => { calls++; throw new Error('PRIVATE TOKEN password=secret') }
  const guard = createManagedAdmissionGuard(f.args)
  await assert.rejects(guard, error => error.message === 'QA09_MANAGED_ADMISSION_UNAVAILABLE'
    && !error.cause && !JSON.stringify(error).includes('secret'))
  assert.equal(calls, 1)
})

test('managed journey refuses a substituted profile before DOM, boundary or action', async () => {
  const f = fixture(); f.proof.candidate = 'b'.repeat(40)
  let boundaryCalls = 0, outcomeCalls = 0
  await assert.rejects(observeManagedRecoveryCase({ ...f.args, caseId: 'QA09-PENDING-PICKUP',
    observer: { snapshot() { throw new Error('DOM_AND_OBSERVER_MUST_NOT_RUN') } },
    waitBoundary() { boundaryCalls++ }, waitForOutcome() { outcomeCalls++ },
  }), error => error.message === 'QA09_FRONTEND_OBSERVATION_FAILED'
    && error.observation.before === null && error.observation.boundaryObserved === false)
  assert.equal(boundaryCalls, 0)
  assert.equal(outcomeCalls, 0)
  assert.equal(f.calls(), 1)
})

test('deletion and closure need lifecycle-aware observation, never the unchanged live-profile gate', async () => {
  for (const caseId of ['QA09-DELETION', 'QA09-PARTIAL-CLOSURE']) {
    const f = fixture()
    await assert.rejects(observeManagedRecoveryCase({ ...f.args, caseId }),
      /^Error: QA09_LIFECYCLE_OBSERVER_REQUIRED$/)
    assert.equal(f.calls(), 0)
  }
})
