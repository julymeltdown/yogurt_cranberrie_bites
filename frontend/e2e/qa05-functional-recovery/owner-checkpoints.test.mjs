// TEST ONLY: synthetic clocks, relay frames, owner observations and ticket hook.
// These fixtures never establish real credentials, I/O, membership or readiness.
import { test } from 'node:test'
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { createOwnerCheckpoints } from './owner-checkpoints.mjs'
import { createFrameFaultController } from './frame-faults.mjs'
import { createCheckpoints, recoveryCases } from './checkpoints.mjs'

const uuid = n => `00000000-0000-0000-0000-${String(n).padStart(12, '0')}`
const clone = value => structuredClone(value)
const counts = { actors: 2, playersPerStream: 2, gamesPerApp: 1, canvasesPerApp: 1, ghostSprites: 0 }
const privateMarker = 'PRIVATE-TICKET-HEADER-PID-MUST-NOT-LEAK'
const refused = promise => assert.rejects(promise, error => {
  assert.equal(error.message, 'QA05_OWNER_CHECKPOINT_UNAVAILABLE')
  assert(!String(error).includes(privateMarker)); return true
})
const idFor = (windowId, caseId) => createHash('sha256').update(`${windowId}:${caseId}`).digest('hex').slice(0, 32)

function fixture(options = {}) {
  let at = Date.parse('2026-11-04T21:00:00Z'), sequence = 0, timer
  const lifetime = new AbortController(), calls = { arms: 0, requests: 0, polls: 0, inspect: 0, probes: 0, sleeps: 0, sends: 0, pauses: 0, resumes: 0 }
  const binding = { candidate: 'a'.repeat(40), environmentId: uuid(1), eventId: uuid(2), windowId: uuid(3), expiresAt: new Date(at + 180000).toISOString() }
  const principalIds = [uuid(4), uuid(5)]
  const config = { releaseRevision: binding.candidate, environmentId: binding.environmentId, eventId: binding.eventId,
    windowId: binding.windowId, origin: 'http://127.0.0.1:12345', contractHashes: { domain: 'b'.repeat(64), realtime: 'c'.repeat(64) },
    members: principalIds.map(userId => ({ userId })) }
  const backendBinding = { candidate: binding.candidate, environmentId: binding.environmentId, eventId: binding.eventId,
    windowId: binding.windowId, origin: config.origin, domainHash: config.contractHashes.domain,
    realtimeHash: config.contractHashes.realtime, members: [...principalIds] }
  const room = uuid(6), epoch = uuid(7)
  const observed = { state: 'OBSERVED', binding: backendBinding,
    generation: { systemIdentifier: 'synthetic-cluster', postmasterStartedAt: 'synthetic-pg-start', concertPid: 123,
      concertStartedAt: 'synthetic-concert-start', executableSha256: 'd'.repeat(64), ownerEpochs: [{ roomId: room, ownerEpoch: epoch }] },
    rooms: [{ roomId: room, ownerEpoch: epoch, ownerProcessId: 'synthetic-owner', leaseUntil: 'synthetic-lease' }],
    memberships: principalIds.map(userId => ({ userId, roomId: room, ownerEpoch: epoch, disconnected: false })) }
  let record, current = observed
  const faults = createFrameFaultController({ now: () => at,
    send() { calls.sends++ }, pauseUpstream() { calls.pauses++ }, resumeUpstream() { calls.resumes++ }, close() {},
    setTimer(callback) { timer = callback; return 1 }, clearTimer() { timer = null } })
  const summary = { activePairs: 2, observedPrincipals: 2, acceptedConnections: 2, disposed: false }
  const relay = {
    summary: () => clone(summary),
    arm(principal, request) { assert.equal(principal, principalIds[0]); calls.arms++; return faults.arm(request) },
    proof(principal) {
      assert(principalIds.includes(principal))
      return { connected: true, receivedFrames: 1 + calls.sends, forwardedBytes: 100 + calls.sends * 3,
        fault: principal === principalIds[0] ? faults.readProof() : { state: 'IDLE' } }
    },
  }
  const ownerControl = {
    inspect() { calls.inspect++; return clone(current) },
    requestWorldRestart(received, { requestId, beforeGeneration }) {
      assert.deepEqual(received, config); assert.deepEqual(beforeGeneration, current.generation)
      assert.equal(requestId, idFor(binding.windowId, 'world-restart'))
      calls.requests++
      record = { binding: clone(backendBinding), requestId, action: 'WORLD_RESTART', state: 'QUEUED',
        before: clone(current), requestedAtEpoch: at / 1000 }
      // A real lease timestamp can renew without a different generation.
      record.before.rooms[0].leaseUntil = 'synthetic-renewed-lease'
      return clone(record)
    },
    checkpoint(received, requestId) { assert.deepEqual(received, config); assert.equal(requestId, record.requestId); calls.polls++; return clone(record) },
  }
  const probe = ({ config: received, requestId, signal }) => {
    assert.deepEqual(received, config); assert(!signal.aborted); calls.probes++
    return { version: 1, binding: clone(binding), requestId, action: 'CONSUMED_TICKET_REPLAY', connectionAttempts: 1,
      authenticateMessages: 1, snapshots: 0, closeCode: 4401, closed: true,
      membershipBefore: { connected: 2, duplicates: 0 }, membershipAfter: { connected: 2, duplicates: 0 } }
  }
  const parameters = { binding, config, principalIds, relay, ownerControl, guard() {}, signal: lifetime.signal,
    now: () => at, sleep: async ms => { calls.sleeps++; at += ms } }
  if (options.probe) parameters.probeConsumedTicket = probe
  const f = { binding, config, principalIds, observed, summary, calls, relay, ownerControl, parameters, lifetime, probe, faults,
    setCurrent(value) { current = value }, advance(ms) { at += ms }, now: () => at,
    request(caseId, step) { return { version: 1, binding: clone(binding), sequence: ++sequence,
      caseId, step, actor: 'actor-a', request: { ...recoveryCases[caseId] }, observation: { ...counts } } },
    completePacket(caseId, frames = 1) {
      for (let i = 0; i < frames; i++) faults.acceptServerFrame(Uint8Array.from([129, 1, 65 + i]), { isBinary: false, isDelta: true })
      if (caseId !== 'packet-gap') { at += recoveryCases[caseId].maximumMs - 10; const callback = timer; callback() }
    },
    completeRestart() {
      const after = clone(observed)
      after.generation.concertPid = 124; after.generation.concertStartedAt = 'synthetic-replacement-start'
      after.generation.ownerEpochs[0].ownerEpoch = uuid(8); after.rooms[0].ownerEpoch = uuid(8)
      after.rooms[0].ownerProcessId = 'synthetic-replacement-owner'; after.memberships = []
      at += 6000
      Object.assign(record, { state: 'COMPLETE', after, stoppedAtEpoch: record.requestedAtEpoch + .5,
        naturalOwnerLeaseWaitSeconds: 5.1, completedAtEpoch: at / 1000 })
      current = clone(after)
      return record
    },
    reconnect() {
      current.memberships = principalIds.map(userId => ({ userId, roomId: current.rooms[0].roomId, ownerEpoch: current.rooms[0].ownerEpoch, disconnected: false }))
      summary.acceptedConnections = 4
    },
    switchRoom() {
      current = clone(current); current.rooms[0].roomId = uuid(9)
      current.generation.ownerEpochs[0].roomId = uuid(9)
      for (const member of current.memberships) member.roomId = uuid(9)
      summary.acceptedConnections = 4
    },
  }
  return f
}
const start = f => { f.checkpoint = createOwnerCheckpoints(f.parameters); return f }
const phase = (f, caseId, step) => f.checkpoint(f.request(caseId, step))
async function foreground(f) {
  assert.equal((await phase(f, 'foreground', 'frontend-ready')).status, 'OBSERVED')
  f.summary.acceptedConnections = 3
  assert.equal((await phase(f, 'foreground', 'after-observation')).status, 'OBSERVED')
}
async function packet(f, caseId, frames = 1) {
  assert.equal((await phase(f, caseId, 'frontend-ready')).status, 'ARMED')
  f.completePacket(caseId, frames)
  return phase(f, caseId, 'after-observation')
}

test('all three packet translations require real utility effects and expose only fixed count proofs', async () => {
  const f = start(fixture())
  for (const caseId of ['packet-gap', 'packet-delay', 'slow-client']) {
    const result = await packet(f, caseId)
    assert.equal(result.status, 'APPLIED'); assert.equal(result.action, recoveryCases[caseId].action)
    assert.equal(result.proof.fault.state, 'COMPLETE'); assert.equal(result.proof.connected, 2)
    const serialized = JSON.stringify(result)
    for (const secret of [f.principalIds[0], 'concertPid', 'synthetic-owner', 'leaseUntil', 'systemIdentifier', 'PRIVATE']) assert(!serialized.includes(secret))
  }
  assert.equal(f.calls.arms, 3); assert.equal(f.calls.pauses, 1); assert.equal(f.calls.resumes, 1)
})
test('a completed real pause/resume with zero in-flight frames is an effect, not a no-delay claim', async () => {
  const f = start(fixture())
  await packet(f, 'packet-gap'); await packet(f, 'packet-delay')
  const result = await packet(f, 'slow-client', 0)
  assert.equal(result.proof.fault.heldFrames, 0); assert.equal(result.proof.fault.resumeCompleted, 1)
})
test('completion waits for bounded actual relay release, not an ARMED response', async () => {
  const f = fixture(), sleep = f.parameters.sleep
  f.parameters.sleep = async ms => { await sleep(ms); f.completePacket('packet-gap') }
  start(f); await phase(f, 'packet-gap', 'frontend-ready')
  assert.equal((await phase(f, 'packet-gap', 'after-observation')).status, 'APPLIED')
  assert.equal(f.calls.sleeps, 1); assert.equal(f.calls.arms, 1)
})
test('an armed gap without a dropped delta expires and cannot be rearmed', async () => {
  const f = start(fixture()); await phase(f, 'packet-gap', 'frontend-ready')
  await refused(phase(f, 'packet-gap', 'after-observation'))
  await refused(phase(f, 'packet-gap', 'frontend-ready')); assert.equal(f.calls.arms, 1)
  assert(f.calls.sleeps <= 40)
})
for (const [name, mutate] of [
  ['invented COMPLETE with no dropped frame', fault => { fault.droppedFrames = 0 }],
  ['overflowed frame bound', fault => { fault.heldFrames = 2 }],
  ['uncertain sink', fault => { fault.failure = privateMarker }],
  ['late completion', fault => { fault.completedAtMs = fault.deadlineMs + 1 }],
  ['foreign request', fault => { fault.requestId = 'foreign' }],
  ['wrong action', fault => { fault.action = 'pause-downstream' }],
]) test(`rejects ${name} instead of certifying packet effect`, async () => {
  const f = start(fixture()); await phase(f, 'packet-gap', 'frontend-ready'); f.completePacket('packet-gap')
  const proof = f.relay.proof
  f.relay.proof = principal => { const result = proof(principal); if (principal === f.principalIds[0]) mutate(result.fault); return result }
  await refused(phase(f, 'packet-gap', 'after-observation')); assert.equal(f.calls.arms, 1)
})
test('missing private ticket hook returns UNAVAILABLE after foreground without claiming an attempt', async () => {
  const f = start(fixture()); await foreground(f)
  const result = await phase(f, 'ticket-replay', 'frontend-ready')
  assert.equal(result.status, 'UNAVAILABLE'); assert.equal(result.proof.reason, 'QA05_CONSUMED_TICKET_PROBE_NOT_BOUND')
  assert.equal(f.calls.probes, 0); await refused(phase(f, 'ticket-replay', 'frontend-ready'))
})
test('the existing consumer accepts the exact owner response correlation and phase ABI', async () => {
  const f = start(fixture({ probe: true }))
  const consumer = createCheckpoints({ binding: f.binding, invoke: f.checkpoint, now: f.now })
  await consumer.checkpoint('foreground', 'frontend-ready', counts)
  f.summary.acceptedConnections = 3
  await consumer.checkpoint('foreground', 'after-observation', counts)
  await consumer.checkpoint('ticket-replay', 'frontend-ready', counts)
  await consumer.checkpoint('ticket-replay', 'after-observation', counts)
  assert.deepEqual(consumer.disposition(), { foreground: 'after-observation', 'ticket-replay': 'after-observation' })
})
test('TEST ONLY canonical replay hook result joins current memberships and stays within the four-connection group', async () => {
  const f = fixture({ probe: true }), probe = f.parameters.probeConsumedTicket
  f.parameters.probeConsumedTicket = input => { f.summary.acceptedConnections = 3; return probe(input) }
  start(f); await foreground(f)
  const result = await phase(f, 'ticket-replay', 'frontend-ready')
  assert.equal(result.status, 'ARMED'); assert.equal(result.proof.closeCode, 4401)
  assert.equal((await phase(f, 'ticket-replay', 'after-observation')).status, 'APPLIED')
  assert.equal(f.calls.probes, 1)
})
test('foreground reserves the fourth total connection for the one direct private replay probe', async () => {
  const f = start(fixture({ probe: true })); await foreground(f)
  f.summary.acceptedConnections = 4
  await refused(phase(f, 'ticket-replay', 'frontend-ready')); assert.equal(f.calls.probes, 0)
})
for (const [name, mutate] of [
  ['timeout close code', proof => { proof.closeCode = 4408 }],
  ['authorization close code', proof => { proof.closeCode = 4403 }],
  ['accepted snapshot', proof => { proof.snapshots = 1 }],
  ['unexecuted hook', proof => { proof.connectionAttempts = 0 }],
  ['extra connection', proof => { proof.connectionAttempts = 2 }],
  ['duplicate membership', proof => { proof.membershipAfter.duplicates = 1 }],
  ['foreign window', proof => { proof.binding.windowId = uuid(99) }],
  ['private extra field', proof => { proof.ticket = privateMarker }],
]) test(`replay rejects ${name} and never retries the probe`, async () => {
  const f = fixture({ probe: true }), probe = f.parameters.probeConsumedTicket
  f.parameters.probeConsumedTicket = input => { const proof = probe(input); mutate(proof); return proof }
  start(f); await foreground(f); await refused(phase(f, 'ticket-replay', 'frontend-ready'))
  await refused(phase(f, 'ticket-replay', 'frontend-ready')); assert.equal(f.calls.probes, 1)
})
test('World restart captures a real generation, accepts once, observes completion before reconnect, then fresh membership', async () => {
  const f = start(fixture())
  const armed = await phase(f, 'world-restart', 'frontend-ready')
  assert.equal(armed.status, 'ARMED'); assert.equal(f.calls.requests, 1); assert.equal(f.calls.polls, 0)
  const original = f.ownerControl.checkpoint
  f.ownerControl.checkpoint = (...args) => { if (f.calls.polls === 1) f.completeRestart(); return original(...args) }
  assert.equal((await phase(f, 'world-restart', 'producer-ready')).status, 'APPLIED')
  assert.equal(f.calls.polls, 2); f.reconnect()
  assert.equal((await phase(f, 'world-restart', 'after-observation')).status, 'APPLIED')
  assert.equal(f.calls.requests, 1)
})
for (const [name, mutate] of [
  ['Postgres alias', result => { result.action = 'POSTGRES_CONCERT_RESTART' }],
  ['same process', result => { result.after.generation.concertPid = result.before.generation.concertPid }],
  ['same epochs', result => { result.after.generation.ownerEpochs = clone(result.before.generation.ownerEpochs); result.after.rooms = clone(result.before.rooms) }],
  ['changed Postgres', result => { result.after.generation.postmasterStartedAt = 'restarted-pg' }],
  ['short lease wait', result => { result.naturalOwnerLeaseWaitSeconds = 5 }],
  ['overlong restart', result => { result.completedAtEpoch = result.requestedAtEpoch + 61 }],
]) test(`World restart refuses ${name} with no second mutation`, async () => {
  const f = start(fixture()); await phase(f, 'world-restart', 'frontend-ready')
  mutate(f.completeRestart()); await refused(phase(f, 'world-restart', 'producer-ready'))
  await refused(phase(f, 'world-restart', 'frontend-ready')); assert.equal(f.calls.requests, 1)
})
test('World restart still queued after the 60-second poll boundary is unavailable, not applied', async () => {
  const f = start(fixture()); await phase(f, 'world-restart', 'frontend-ready')
  await refused(phase(f, 'world-restart', 'producer-ready')); assert.equal(f.calls.requests, 1)
  assert(f.calls.polls <= 601 && f.calls.sleeps <= 600)
})
test('restart reserve is checked again after real initial observations, before mutation', async () => {
  const f = fixture(), inspect = f.ownerControl.inspect
  f.binding.expiresAt = new Date(f.now() + 66000).toISOString()
  f.ownerControl.inspect = () => { f.advance(2000); return inspect() }
  start(f); await refused(phase(f, 'world-restart', 'frontend-ready')); assert.equal(f.calls.requests, 0)
})
test('room observations require a different current room and exactly the current pair', async () => {
  const f = start(fixture()); assert.equal((await phase(f, 'room-switch', 'frontend-ready')).status, 'OBSERVED')
  f.switchRoom(); assert.equal((await phase(f, 'room-switch', 'after-observation')).status, 'OBSERVED')
  assert.equal(f.calls.arms + f.calls.requests + f.calls.probes, 0)
  const unchanged = start(fixture()); await phase(unchanged, 'room-switch', 'frontend-ready')
  await refused(phase(unchanged, 'room-switch', 'after-observation'))
})
for (const [name, mutate] of [
  ['foreign backend binding', f => { f.observed.binding.eventId = uuid(99) }],
  ['ghost principal', f => { f.observed.memberships[1].userId = uuid(99) }],
  ['duplicate principal', f => { f.observed.memberships[1].userId = f.principalIds[0] }],
  ['disconnected member', f => { f.observed.memberships[1].disconnected = true }],
  ['stale owner epoch', f => { f.observed.memberships[1].ownerEpoch = uuid(99) }],
  ['unobserved App socket', f => { f.summary.observedPrincipals = 1 }],
  ['fifth lifetime connection', f => { f.summary.acceptedConnections = 5 }],
]) test(`fresh owner/relay join rejects ${name} before any fault`, async () => {
  const f = fixture(); mutate(f); start(f)
  await refused(phase(f, 'foreground', 'frontend-ready')); assert.equal(f.calls.arms + f.calls.requests + f.calls.probes, 0)
})
test('cross-group, wrong phase, widened bounds and duplicate sequence never issue another effect', async () => {
  for (const mutate of [request => { request.actor = 'actor-b' }, request => { request.sequence = 2 },
    request => { request.step = 'after-observation' }, request => { request.request.maximumFrames = 2 },
    request => { request.binding.candidate = 'f'.repeat(40) }]) {
    const f = start(fixture()), request = f.request('packet-gap', 'frontend-ready'); mutate(request)
    await refused(f.checkpoint(request)); assert.equal(f.calls.arms, 0)
  }
  const f = start(fixture()); await foreground(f)
  await refused(phase(f, 'room-switch', 'frontend-ready')); assert.equal(f.calls.requests + f.calls.arms, 0)
})
test('synchronous reentry and concurrent calls cannot double-arm the actual relay', async () => {
  const f = fixture(); let nested, invoked = false
  f.parameters.guard = () => { if (!invoked) { invoked = true; nested = f.checkpoint(f.request('packet-gap', 'frontend-ready')); nested.catch(() => {}) } }
  start(f); const first = phase(f, 'packet-gap', 'frontend-ready')
  await refused(nested); assert.equal((await first).status, 'ARMED'); assert.equal(f.calls.arms, 1)
})
test('caller data and configuration mutations during a pending inspect cannot change the armed request', async () => {
  const f = fixture(), inspect = f.ownerControl.inspect
  let release
  f.ownerControl.inspect = () => new Promise(resolve => { release = () => resolve(inspect()) })
  start(f); const request = f.request('packet-gap', 'frontend-ready'), running = f.checkpoint(request)
  await Promise.resolve(); await Promise.resolve()
  request.request.maximumFrames = 999; request.caseId = 'world-restart'
  f.config.members[0].userId = uuid(99); f.binding.candidate = 'f'.repeat(40)
  release(); assert.equal((await running).action, 'drop-next-delta'); assert.equal(f.calls.arms, 1)
})
test('aborting a hanging mutation retires without retry and ignores its later private failure', async () => {
  const f = fixture(); let rejectLate
  f.ownerControl.requestWorldRestart = () => { f.calls.requests++; return new Promise((_, reject) => { rejectLate = reject }) }
  start(f); const running = phase(f, 'world-restart', 'frontend-ready')
  while (!rejectLate) await Promise.resolve()
  f.lifetime.abort(); await refused(running)
  rejectLate(new Error(privateMarker)); await Promise.resolve()
  await refused(phase(f, 'world-restart', 'frontend-ready')); assert.equal(f.calls.requests, 1)
})
test('an uncertain restart request is not retried even if the private owner already queued it', async () => {
  const f = fixture(), request = f.ownerControl.requestWorldRestart
  f.ownerControl.requestWorldRestart = (...args) => { request(...args); throw new Error(privateMarker) }
  start(f); await refused(phase(f, 'world-restart', 'frontend-ready'))
  await refused(phase(f, 'world-restart', 'frontend-ready')); assert.equal(f.calls.requests, 1)
})
test('per-call abort, expiry and guard failure stop before a mutation, without private exception text', async () => {
  for (const mode of ['abort', 'expiry', 'guard']) {
    const f = fixture(), callController = new AbortController()
    if (mode === 'guard') f.parameters.guard = () => { throw new Error(privateMarker) }
    start(f)
    if (mode === 'expiry') f.advance(176000)
    if (mode === 'abort') callController.abort()
    await refused(f.checkpoint(f.request('packet-gap', 'frontend-ready'), { signal: callController.signal }))
    assert.equal(f.calls.arms, 0)
  }
})
