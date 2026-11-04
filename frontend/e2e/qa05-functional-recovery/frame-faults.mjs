// Opaque byte-preserving fault utility, not a server or C05 decoder. Root owns
// actual complete-frame boundaries, classification, one upstream and sink I/O.
import assert from 'node:assert/strict'

const LIMITS = Object.freeze({
  'drop-next-delta': { caseId: 'packet-gap', frames: 1, bytes: 65546, ms: 1000 },
  'delay-downstream': { caseId: 'packet-delay', frames: 8, bytes: 262144, ms: 250 },
  'pause-downstream': { caseId: 'slow-client', frames: 64, bytes: 262144, ms: 1000 },
})

export function createFrameFaultController({ send, pauseUpstream, resumeUpstream, close,
  now = () => performance.now(), setTimer = setTimeout, clearTimer = clearTimeout }) {
  for (const fn of [send, pauseUpstream, resumeUpstream, close, now, setTimer, clearTimer]) assert.equal(typeof fn, 'function')
  let state = 'IDLE', action = null, fault = null, timer = null, paused = false, lastNow = -Infinity
  let queue = [], queueBytes = 0
  const used = new Set()
  let proof = null
  const safeReason = error => /^QA05_[A-Z_]+$/.test(error?.message ?? '') ? error.message : 'QA05_CALLBACK_UNCERTAIN'
  function clock() {
    const value = now()
    assert(Number.isFinite(value) && value >= lastNow, 'QA05_CLOCK_INVALID')
    lastNow = value
    return value
  }
  function cancelTimer() {
    if (timer !== null) { const current = timer; timer = null; call(clearTimer, [current], 'QA05_TIMER_UNCERTAIN') }
  }
  function retire(reason) {
    if (state === 'RETIRED') return
    state = 'RETIRED'; fault = reason
    // Clear retained traffic before calling any fallible external callback.
    queue = []; queueBytes = 0
    try { cancelTimer() } catch { /* Closing remains mandatory. */ }
    if (proof) { proof.failure = reason; proof.queuedFrames = 0; proof.queuedBytes = 0 }
    // Never resume or flush after an unknown effect. Only the actual owner can
    // close its existing pair; no substitute frame/connection is manufactured.
    try { close() } catch { if (proof) proof.closeFailed = true }
  }
  function call(fn, args, failure) {
    let value
    try { value = fn(...args) } catch { throw new Error(failure) }
    if (value?.then) {
      // A forbidden async callback may already have side effects. Consume its
      // rejection without logging private details, retire and never retry it.
      Promise.resolve(value).catch(() => {})
      throw new Error(failure)
    }
    if (value === false) throw new Error(failure)
  }
  function deliver(frame) {
    proof.sendAttempts++
    call(send, [frame.bytes, { isBinary: frame.isBinary }], 'QA05_SEND_UNCERTAIN')
    proof.deliveredFrames++; proof.deliveredBytes += frame.bytes.length
  }
  function flush(at) {
    while (queue.length) {
      const frame = queue[0]
      deliver(frame)
      assert(clock() <= proof.deadlineMs, 'QA05_TIMER_LATE')
      queue.shift(); queueBytes -= frame.bytes.length
    }
    proof.queuedFrames = 0; proof.queuedBytes = 0
    if (paused) {
      proof.resumeAttempts++
      call(resumeUpstream, [], 'QA05_RESUME_UNCERTAIN')
      paused = false; proof.resumeCompleted++; proof.resumedAtMs = clock()
      assert(proof.resumedAtMs <= proof.deadlineMs, 'QA05_TIMER_LATE')
    }
    proof.completedAtMs = Math.max(at, clock()); state = 'COMPLETE'
  }
  function deadline() {
    timer = null
    if (state !== 'ARMED') return
    try {
      const at = clock()
      // An event-loop stall cannot silently extend the admitted hold/TTL.
      if (at > proof.deadlineMs) throw new Error('QA05_TIMER_LATE')
      assert(at >= proof.releaseDueAtMs, 'QA05_TIMER_EARLY')
      if (action.action === 'drop-next-delta' || (action.action === 'delay-downstream' && proof.heldFrames === 0)) throw new Error('QA05_NO_FAULT_EFFECT')
      flush(at)
    } catch (error) { retire(safeReason(error)) }
  }
  function arm(request) {
    assert(state === 'IDLE' || state === 'COMPLETE', 'QA05_FAULT_NOT_ARMABLE')
    assert(request && Object.keys(request).every(key => ['caseId', 'requestId', 'action', 'maximumFrames', 'maximumBytes', 'maximumMs'].includes(key)))
    const limits = LIMITS[request.action]
    assert(limits && request.caseId === limits.caseId, 'QA05_CASE_ACTION_MISMATCH')
    assert(typeof request.requestId === 'string' && /^[a-zA-Z0-9:_-]{1,128}$/.test(request.requestId))
    assert(used.size < 3 && !used.has(request.requestId), 'QA05_FAULT_DUPLICATE_OR_LIMIT')
    assert(Number.isInteger(request.maximumFrames) && request.maximumFrames >= 1 && request.maximumFrames <= limits.frames)
    assert(Number.isInteger(request.maximumMs) && request.maximumMs >= 1 && request.maximumMs <= limits.ms)
    const bytes = request.maximumBytes ?? limits.bytes
    assert(Number.isInteger(bytes) && bytes >= 1 && bytes <= limits.bytes)
    const at = clock()
    action = Object.freeze({ ...request, maximumBytes: bytes }); used.add(request.requestId)
    proof = { caseId: request.caseId, requestId: request.requestId, action: request.action, armedAtMs: at,
      deadlineMs: at + request.maximumMs,
      releaseDueAtMs: at + request.maximumMs - (request.action === 'drop-next-delta' ? 0 : Math.min(10, Math.floor(request.maximumMs / 10))),
      completedAtMs: null, acceptedFrames: 0, acceptedBytes: 0,
      deliveredFrames: 0, deliveredBytes: 0, droppedFrames: 0, droppedBytes: 0,
      heldFrames: 0, heldBytes: 0, queuedFrames: 0, queuedBytes: 0, peakQueuedFrames: 0, peakQueuedBytes: 0,
      sendAttempts: 0, pauseAttempts: 0, pauseCompleted: 0, resumeAttempts: 0, resumeCompleted: 0,
      pausedAtMs: null, resumedAtMs: null, failure: null, closeFailed: false }
    state = 'ARMED'
    try {
      if (request.action === 'pause-downstream') {
        proof.pauseAttempts++; call(pauseUpstream, [], 'QA05_PAUSE_UNCERTAIN')
        paused = true; proof.pauseCompleted++; proof.pausedAtMs = clock()
      }
      // Node truncates fractional timer delays. Round up so a normal timer
      // cannot fire before releaseDueAtMs solely because of that truncation.
      timer = setTimer(deadline, Math.max(0, Math.ceil(proof.releaseDueAtMs - clock())))
      assert(timer !== null && timer !== undefined, 'QA05_TIMER_NOT_BOUND')
      if (timer?.then) { Promise.resolve(timer).catch(() => {}); throw new Error('QA05_TIMER_UNCERTAIN') }
    } catch (error) { retire(safeReason(error)) }
    return readProof()
  }
  function acceptServerFrame(originalBytes, { isBinary, isDelta }) {
    if (state === 'RETIRED') return readProof()
    try {
      const at = clock()
      assert(originalBytes instanceof Uint8Array && originalBytes.length > 0 && originalBytes.length <= 65546, 'QA05_FRAME_BOUND')
      assert(typeof isBinary === 'boolean' && typeof isDelta === 'boolean', 'QA05_CANONICAL_CLASSIFICATION_REQUIRED')
      if (state === 'IDLE') {
        call(send, [Buffer.from(originalBytes), { isBinary }], 'QA05_SEND_UNCERTAIN')
        return readProof()
      }
      if (state === 'ARMED' && at >= proof.deadlineMs) throw new Error('QA05_FAULT_EXPIRED')
      const held = state === 'ARMED' && action.action !== 'drop-next-delta'
      if (held && (queue.length + 1 > action.maximumFrames || queueBytes + originalBytes.length > action.maximumBytes)) throw new Error('QA05_QUEUE_BOUND')
      if (state === 'ARMED' && action.action === 'drop-next-delta' && isDelta && originalBytes.length > action.maximumBytes) throw new Error('QA05_FRAME_BOUND')
      // Copy only after all admission limits. No payload is exposed by proof.
      const frame = { bytes: Buffer.from(originalBytes), isBinary }
      proof.acceptedFrames++; proof.acceptedBytes += frame.bytes.length
      if (state === 'ARMED' && action.action === 'drop-next-delta' && isDelta) {
        proof.droppedFrames++; proof.droppedBytes += frame.bytes.length
        proof.completedAtMs = at; cancelTimer(); state = 'COMPLETE'
      } else if (held) {
        queue.push(frame); queueBytes += frame.bytes.length
        proof.heldFrames++; proof.heldBytes += frame.bytes.length
        proof.queuedFrames = queue.length; proof.queuedBytes = queueBytes
        proof.peakQueuedFrames = Math.max(proof.peakQueuedFrames, queue.length)
        proof.peakQueuedBytes = Math.max(proof.peakQueuedBytes, queueBytes)
      } else deliver(frame)
    } catch (error) { retire(safeReason(error)) }
    return readProof()
  }
  function readProof() {
    return { state, failure: fault, paused, ...(proof && { ...proof }),
      qualification: 'Measured supplied-frame handling and synchronous actual-owner callback handoff only. Root binds canonical C05 classification, actual I/O and one upstream. Peer delivery/server queue/membership require separate actual observations.' }
  }
  function dispose() { retire('QA05_DISPOSED'); return readProof() }
  return Object.freeze({ arm, acceptServerFrame, readProof, dispose })
}
