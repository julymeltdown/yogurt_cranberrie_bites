// Private in-process translation only. Root supplies the actual relay, retained
// owner control and optional consumed-ticket probe. No service or authority is
// created here; synthetic hooks cannot qualify an actual App/producer journey.
import { createHash } from 'node:crypto'
import { setTimeout as sleepDefault } from 'node:timers/promises'
import { isDeepStrictEqual as equal } from 'node:util'
import { checkpointVersion, recoveryCases } from './checkpoints.mjs'

const UUID = /^[a-f0-9]{8}(?:-[a-f0-9]{4}){3}-[a-f0-9]{12}$/
const groups = Object.freeze([
  ['packet-gap', 'packet-delay', 'slow-client'], ['foreground', 'ticket-replay'],
  ['world-restart'], ['room-switch'],
])
const appCounts = Object.freeze({ actors: 2, playersPerStream: 2, gamesPerApp: 1, canvasesPerApp: 1, ghostSprites: 0 })
const countKeys = ['acceptedFrames', 'acceptedBytes', 'deliveredFrames', 'deliveredBytes', 'droppedFrames', 'droppedBytes',
  'heldFrames', 'heldBytes', 'queuedFrames', 'queuedBytes', 'peakQueuedFrames', 'peakQueuedBytes',
  'sendAttempts', 'pauseAttempts', 'pauseCompleted', 'resumeAttempts', 'resumeCompleted']
const need = value => { if (!value) throw new Error('QA05_OWNER_PROOF_UNAVAILABLE') }
const integer = value => Number.isSafeInteger(value) && value >= 0
const text = value => typeof value === 'string' && value.length > 0 && value.length <= 256
const exactKeys = (value, keys) => value && typeof value === 'object' && !Array.isArray(value)
  && equal(Object.keys(value).sort(), [...keys].sort())
const idFor = (windowId, caseId) => createHash('sha256').update(`${windowId}:${caseId}`).digest('hex').slice(0, 32)

export function createOwnerCheckpoints({ binding, config, principalIds, relay, ownerControl,
  probeConsumedTicket, guard, signal, now = Date.now, sleep = (ms, options) => sleepDefault(ms, undefined, options) }) {
  need(exactKeys(binding, ['candidate', 'environmentId', 'eventId', 'windowId', 'expiresAt']))
  need(/^[a-f0-9]{40}$/.test(binding.candidate) && Number.isFinite(Date.parse(binding.expiresAt)))
  for (const key of ['environmentId', 'eventId', 'windowId']) need(UUID.test(binding[key]))
  need(Array.isArray(principalIds) && principalIds.length === 2 && new Set(principalIds).size === 2 && principalIds.every(id => UUID.test(id)))
  need(config?.releaseRevision === binding.candidate && config.environmentId === binding.environmentId
    && config.eventId === binding.eventId && config.windowId === binding.windowId)
  need(equal(config.members?.map(member => member.userId), principalIds))
  need(/^[a-f0-9]{64}$/.test(config.contractHashes?.domain) && /^[a-f0-9]{64}$/.test(config.contractHashes?.realtime))
  need(URL.canParse(config.origin))
  const origin = new URL(config.origin)
  need(origin.protocol === 'http:' && ['127.0.0.1', '[::1]', 'localhost'].includes(origin.hostname)
    && origin.port && origin.origin === config.origin)
  for (const fn of [guard, now, sleep, relay?.arm, relay?.proof, relay?.summary,
    ownerControl?.inspect, ownerControl?.requestWorldRestart, ownerControl?.checkpoint]) need(typeof fn === 'function')
  need(probeConsumedTicket === undefined || typeof probeConsumedTicket === 'function')
  need(signal instanceof AbortSignal && !signal.aborted)
  const bound = Object.freeze({ ...binding }), principals = Object.freeze([...principalIds])
  // Only the documented noncredential owner configuration is passed onward.
  const cfg = Object.freeze({ releaseRevision: bound.candidate, environmentId: bound.environmentId,
    eventId: bound.eventId, windowId: bound.windowId, origin: config.origin,
    contractHashes: Object.freeze({ domain: config.contractHashes.domain, realtime: config.contractHashes.realtime }),
    members: Object.freeze(principals.map(userId => Object.freeze({ userId }))) })
  const backendBinding = { candidate: bound.candidate, environmentId: bound.environmentId, eventId: bound.eventId,
    windowId: bound.windowId, origin: cfg.origin, domainHash: cfg.contractHashes.domain,
    realtimeHash: cfg.contractHashes.realtime, members: [...principals] }
  let retired = false, busy = false, sequence = 0, group, caseIndex = 0, stageIndex = 0
  let initial = null, completedRestart = null, lastNow = -Infinity
  function clock() {
    const at = now(); need(Number.isFinite(at) && at >= lastNow); lastNow = at; return at
  }
  function live(callSignal) {
    need(!retired && !signal.aborted && !callSignal?.aborted && Date.parse(bound.expiresAt) - clock() > 5000)
    const guarded = guard()
    if (guarded?.then) { Promise.resolve(guarded).catch(() => {}); need(false) }
    need(guarded !== false)
    need(!signal.aborted && !callSignal?.aborted && Date.parse(bound.expiresAt) - clock() > 5000)
  }
  // Even a hanging private hook cannot keep this facade pending indefinitely.
  // Unknown effects retire it; a late result is consumed but never used/retried.
  async function call(fn, callSignal) {
    live(callSignal)
    let abort
    const cancelled = new Promise((_, reject) => {
      abort = () => reject(new Error('QA05_OWNER_PROOF_UNAVAILABLE'))
      callSignal.addEventListener('abort', abort, { once: true })
    })
    try {
      const result = await Promise.race([Promise.resolve().then(() => { live(callSignal); return fn() }), cancelled])
      live(callSignal); return result
    } finally { callSignal.removeEventListener('abort', abort) }
  }
  function observation(value, connected) {
    need(value?.state === 'OBSERVED' && equal(value.binding, backendBinding))
    const generation = value.generation
    need(generation && text(generation.systemIdentifier) && text(generation.postmasterStartedAt)
      && Number.isSafeInteger(generation.concertPid) && generation.concertPid > 0
      && text(generation.concertStartedAt) && /^[a-f0-9]{64}$/.test(generation.executableSha256))
    need(Array.isArray(value.rooms) && value.rooms.length > 0 && value.rooms.length <= 16
      && new Set(value.rooms.map(room => room.roomId)).size === value.rooms.length)
    for (const room of value.rooms) need(UUID.test(room.roomId) && UUID.test(room.ownerEpoch) && text(room.ownerProcessId) && text(room.leaseUntil))
    need(equal(generation.ownerEpochs, value.rooms.map(({ roomId, ownerEpoch }) => ({ roomId, ownerEpoch }))))
    need(Array.isArray(value.memberships) && value.memberships.length <= 2
      && new Set(value.memberships.map(member => member.userId)).size === value.memberships.length)
    for (const member of value.memberships) need(principals.includes(member.userId) && typeof member.disconnected === 'boolean'
      && value.rooms.some(room => room.roomId === member.roomId && room.ownerEpoch === member.ownerEpoch))
    if (connected) need(value.memberships.length === 2 && value.memberships.every(member => !member.disconnected)
      && value.memberships[0].roomId === value.memberships[1].roomId)
    return structuredClone(value)
  }
  function processIdentity(generation) {
    const { ownerEpochs: _epochs, ...identity } = generation
    return identity
  }
  function sameMembership(a, b) {
    return equal([...a.memberships].sort((x, y) => x.userId.localeCompare(y.userId)),
      [...b.memberships].sort((x, y) => x.userId.localeCompare(y.userId)))
  }
  async function inspect(callSignal, connected = true) {
    return observation(await call(() => ownerControl.inspect(cfg), callSignal), connected)
  }
  async function actualRelay(callSignal) {
    const summary = await call(() => relay.summary(), callSignal)
    need(summary?.disposed === false && summary.activePairs === 2 && summary.observedPrincipals === 2
      && integer(summary.acceptedConnections) && summary.acceptedConnections >= 2
      && summary.acceptedConnections <= (group === groups[0] ? 2 : group === groups[1] ? 3 : 4))
    const proofs = []
    for (const principal of principals) {
      const proof = await call(() => relay.proof(principal), callSignal)
      need(proof?.connected === true && integer(proof.receivedFrames) && proof.receivedFrames >= 1
        && integer(proof.forwardedBytes) && proof.forwardedBytes > 0
        && ['IDLE', 'COMPLETE', 'ARMED'].includes(proof.fault?.state))
      proofs.push(proof)
    }
    need(proofs[1].fault.state === 'IDLE')
    if (caseIndex === 0 && stageIndex === 0) need(summary.acceptedConnections === 2 && proofs[0].fault.state === 'IDLE')
    return proofs
  }
  function frameProof(proof, request, complete) {
    const limits = request.request, fault = proof
    need(fault && fault.state === (complete ? 'COMPLETE' : 'ARMED') && fault.caseId === request.caseId
      && fault.requestId === idFor(bound.windowId, request.caseId) && fault.action === limits.action
      && fault.failure === null && fault.closeFailed === false)
    for (const key of countKeys) need(integer(fault[key]))
    need(Number.isFinite(fault.armedAtMs) && Number.isFinite(fault.deadlineMs) && Number.isFinite(fault.releaseDueAtMs)
      && fault.deadlineMs === fault.armedAtMs + limits.maximumMs
      && fault.releaseDueAtMs === fault.deadlineMs - (request.caseId === 'packet-gap' ? 0 : Math.min(10, Math.floor(limits.maximumMs / 10))))
    need(fault.acceptedFrames === fault.deliveredFrames + fault.droppedFrames + fault.queuedFrames
      && fault.acceptedBytes === fault.deliveredBytes + fault.droppedBytes + fault.queuedBytes
      && fault.sendAttempts === fault.deliveredFrames)
    need(fault.heldFrames <= limits.maximumFrames && fault.peakQueuedFrames <= limits.maximumFrames
      && fault.heldBytes <= (limits.maximumBytes ?? 65546) && fault.peakQueuedBytes <= (limits.maximumBytes ?? 65546)
      && fault.queuedFrames <= fault.peakQueuedFrames && fault.queuedBytes <= fault.peakQueuedBytes)
    if (request.caseId === 'slow-client') {
      need(fault.pauseAttempts === 1 && fault.pauseCompleted === 1 && Number.isFinite(fault.pausedAtMs)
        && fault.pausedAtMs >= fault.armedAtMs && fault.pausedAtMs <= fault.deadlineMs)
      if (complete) need(fault.resumeAttempts === 1 && fault.resumeCompleted === 1 && fault.paused === false
        && Number.isFinite(fault.resumedAtMs) && fault.resumedAtMs >= fault.releaseDueAtMs && fault.resumedAtMs <= fault.completedAtMs)
      else need(fault.paused === true && fault.resumeAttempts === 0 && fault.resumeCompleted === 0)
    } else need(fault.pauseAttempts + fault.pauseCompleted + fault.resumeAttempts + fault.resumeCompleted === 0 && fault.paused === false)
    if (complete) {
      need(Number.isFinite(fault.completedAtMs) && fault.completedAtMs >= fault.armedAtMs && fault.completedAtMs <= fault.deadlineMs
        && fault.queuedFrames === 0 && fault.queuedBytes === 0)
      if (request.caseId === 'packet-gap') need(fault.droppedFrames === 1 && fault.droppedBytes > 0
        && fault.droppedBytes <= 65546 && fault.heldFrames === 0 && fault.heldBytes === 0)
      else need(fault.completedAtMs >= fault.releaseDueAtMs && fault.droppedFrames === 0 && fault.droppedBytes === 0
        && fault.heldFrames === fault.peakQueuedFrames && fault.heldBytes === fault.peakQueuedBytes
        && fault.deliveredFrames >= fault.heldFrames && fault.deliveredBytes >= fault.heldBytes)
      if (request.caseId === 'packet-delay') need(fault.heldFrames > 0 && fault.heldBytes >= fault.heldFrames)
    } else need(fault.completedAtMs === null)
    return { state: fault.state, ...Object.fromEntries(countKeys.map(key => [key, fault[key]])),
      durationMs: complete ? fault.completedAtMs - fault.armedAtMs : null }
  }
  function restartRecord(value, complete) {
    need(value && equal(value.binding, backendBinding) && value.requestId === idFor(bound.windowId, 'world-restart')
      && value.action === 'WORLD_RESTART' && ['QUEUED', 'STOPPING', 'LEASE_WAIT', 'REPLACEMENT_STARTING', 'COMPLETE'].includes(value.state)
      && Number.isFinite(value.requestedAtEpoch))
    const beforeObservation = observation(value.before, true)
    // Lease renewal timestamps may advance between inspect and queued request;
    // process/epoch identity and the exact current membership must still agree.
    need(equal(beforeObservation.generation, initial.generation) && sameMembership(beforeObservation, initial))
    if (value.state === 'COMPLETE') {
      const after = observation(value.after, false), before = initial.generation, next = after.generation
      need(next.systemIdentifier === before.systemIdentifier && next.postmasterStartedAt === before.postmasterStartedAt
        && next.executableSha256 === before.executableSha256 && next.concertPid !== before.concertPid)
      need(equal(after.rooms.map(room => room.roomId), initial.rooms.map(room => room.roomId))
        && after.rooms.every(room => !initial.rooms.some(old => old.ownerEpoch === room.ownerEpoch)))
      need(Number.isFinite(value.naturalOwnerLeaseWaitSeconds) && value.naturalOwnerLeaseWaitSeconds >= 5.1
        && Number.isFinite(value.stoppedAtEpoch) && value.stoppedAtEpoch >= value.requestedAtEpoch
        && Number.isFinite(value.completedAtEpoch) && value.completedAtEpoch >= value.stoppedAtEpoch
        && value.completedAtEpoch - value.requestedAtEpoch <= 60)
      completedRestart = after
    } else need(!complete)
    return value.state
  }
  function replayProof(value, requestId) {
    need(exactKeys(value, ['version', 'binding', 'requestId', 'action', 'connectionAttempts', 'authenticateMessages', 'snapshots',
      'closeCode', 'closed', 'membershipBefore', 'membershipAfter']))
    need(value.version === 1 && equal(value.binding, bound) && value.requestId === requestId
      && value.action === 'CONSUMED_TICKET_REPLAY' && value.connectionAttempts === 1 && value.authenticateMessages === 1
      && value.snapshots === 0 && value.closeCode === 4401 && value.closed === true)
    for (const key of ['membershipBefore', 'membershipAfter']) need(equal(value[key], { connected: 2, duplicates: 0 }))
    return { connectionAttempts: 1, authenticateMessages: 1, snapshots: 0, closeCode: 4401, closed: true }
  }
  async function execute(request, callSignal) {
    const { caseId, step } = request, ready = step === 'frontend-ready'
    if (ready) {
      initial = await inspect(callSignal); await actualRelay(callSignal)
      if (caseId === 'ticket-replay' && !probeConsumedTicket) return { status: 'UNAVAILABLE', proof: { reason: 'QA05_CONSUMED_TICKET_PROBE_NOT_BOUND' } }
      if (caseId === 'world-restart') {
        need(Date.parse(bound.expiresAt) - clock() >= 65000)
        const result = await call(() => ownerControl.requestWorldRestart(cfg, {
          requestId: idFor(bound.windowId, caseId), beforeGeneration: structuredClone(initial.generation),
        }), callSignal)
        restartRecord(result, false)
        return { status: 'ARMED', proof: { kind: 'WORLD_RESTART_ACCEPTED' } }
      }
      if (groups[0].includes(caseId)) {
        const fault = await call(() => relay.arm(principals[0], { caseId, requestId: idFor(bound.windowId, caseId), ...request.request }), callSignal)
        return { status: 'ARMED', proof: { kind: 'ACTUAL_RELAY_ARMED', fault: frameProof(fault, request, false) } }
      }
      if (caseId === 'ticket-replay') {
        const requestId = idFor(bound.windowId, caseId)
        const probeSignal = AbortSignal.any([callSignal, AbortSignal.timeout(request.request.maximumMs)])
        const proof = replayProof(await call(() => probeConsumedTicket({ config: cfg, requestId, signal: probeSignal }), probeSignal), requestId)
        const after = await inspect(callSignal); await actualRelay(callSignal)
        need(equal(processIdentity(initial.generation), processIdentity(after.generation)) && sameMembership(initial, after))
        return { status: 'ARMED', proof: { kind: 'CONSUMED_TICKET_REPLAY_EXECUTED', ...proof } }
      }
      return { status: 'OBSERVED', proof: { kind: 'CURRENT_OWNER_AND_APP_OBSERVATION', apps: { ...appCounts }, connected: 2, duplicates: 0 } }
    }
    if (caseId === 'world-restart' && step === 'producer-ready') {
      const until = clock() + 60000
      while (true) {
        const result = await call(() => ownerControl.checkpoint(cfg, idFor(bound.windowId, caseId)), callSignal)
        const state = restartRecord(result, false)
        if (state === 'COMPLETE') return { status: 'APPLIED', proof: { kind: 'WORLD_ONLY_REPLACEMENT_COMPLETE', replacements: 1, naturalLeaseWait: true } }
        need(clock() < until)
        await call(() => sleep(Math.min(100, until - clock()), { signal: callSignal }), callSignal)
      }
    }
    let fault
    if (groups[0].includes(caseId)) {
      const until = clock() + request.request.maximumMs
      while (true) {
        const pair = (await actualRelay(callSignal))[0]
        if (pair.fault.state === 'COMPLETE') { fault = frameProof(pair.fault, request, true); break }
        frameProof(pair.fault, request, false)
        need(clock() < until)
        await call(() => sleep(Math.min(25, until - clock()), { signal: callSignal }), callSignal)
      }
    } else await actualRelay(callSignal)
    const after = await inspect(callSignal)
    if (caseId === 'world-restart') need(completedRestart && equal(after.generation, completedRestart.generation))
    else {
      need(equal(processIdentity(initial.generation), processIdentity(after.generation)))
      if (caseId === 'room-switch') need(initial.memberships[0].roomId !== after.memberships[0].roomId)
      else need(sameMembership(initial, after))
    }
    return { status: request.request.action === 'observe-only' ? 'OBSERVED' : 'APPLIED',
      proof: { kind: fault ? 'ACTUAL_RELAY_EFFECT_COMPLETE' : 'CURRENT_OWNER_AND_APP_OBSERVATION',
        apps: { ...appCounts }, connected: 2, duplicates: 0, ...(fault && { fault }) } }
  }
  return async function checkpoint(request, { signal: requestSignal } = {}) {
    // Reserve before the first callback/await, including guard reentry.
    if (busy || retired) throw new Error('QA05_OWNER_CHECKPOINT_UNAVAILABLE')
    busy = true
    try {
      live(requestSignal)
      need(exactKeys(request, ['version', 'binding', 'sequence', 'caseId', 'step', 'actor', 'request', 'observation'])
        && request.version === checkpointVersion && equal(request.binding, bound) && request.actor === 'actor-a'
        && request.sequence === sequence + 1 && Object.hasOwn(recoveryCases, request.caseId)
        && equal(request.request, recoveryCases[request.caseId]))
      need(request.observation && Object.entries(appCounts).every(([key, value]) => request.observation[key] === value)
        && Object.keys(request.observation).every(key => Object.hasOwn(appCounts, key) || key === 'qualification')
        && (request.observation.qualification === undefined || (typeof request.observation.qualification === 'string' && request.observation.qualification.length <= 128)))
      if (!group) { group = groups.find(cases => cases[0] === request.caseId); need(group) }
      const steps = request.caseId === 'world-restart' ? ['frontend-ready', 'producer-ready', 'after-observation'] : ['frontend-ready', 'after-observation']
      need(request.caseId === group[caseIndex] && request.step === steps[stageIndex])
      sequence++
      // Freeze untrusted mutable caller data before any private I/O.
      const frozen = structuredClone(request)
      const maximumMs = frozen.step === 'producer-ready' ? 60000 : 35000
      const timeout = AbortSignal.timeout(Math.max(1, Math.floor(Math.min(maximumMs, Date.parse(bound.expiresAt) - clock() - 5000))))
      const callSignal = AbortSignal.any([signal, timeout, ...(requestSignal ? [requestSignal] : [])])
      const result = await execute(frozen, callSignal)
      live(callSignal)
      if (result.status === 'UNAVAILABLE') retired = true
      else if (++stageIndex === steps.length) { stageIndex = 0; caseIndex++; initial = null }
      return { version: checkpointVersion, binding: bound, sequence: frozen.sequence, caseId: frozen.caseId,
        step: frozen.step, action: frozen.request.action, ...result }
    } catch {
      retired = true
      // Never expose private callback exceptions, assertion values or raw owner
      // observations (identities, processes, paths, credentials) to the driver.
      throw new Error('QA05_OWNER_CHECKPOINT_UNAVAILABLE')
    } finally { busy = false }
  }
}
