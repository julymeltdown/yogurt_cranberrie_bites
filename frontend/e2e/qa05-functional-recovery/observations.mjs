// Read-only oracle for messages validated by the actual App C05 decoder.
// Identity/positions stay in RAM. summary() is the only durable report shape.
import assert from 'node:assert/strict'

export function createWorldObservation({ eventId, actorId, maximumPlayers }) {
  assert(typeof eventId === 'string' && typeof actorId === 'string')
  assert(Number.isInteger(maximumPlayers) && maximumPlayers >= 2 && maximumPlayers <= 8)
  const sockets = new Set(), pending = new Set()
  let current, baseline = null, players = new Map(), streamSeq = -1
  let admission = null, retired = false
  const counts = { opens: 0, peakSockets: 0, snapshots: 0, deltas: 0, gaps: 0, resyncs: 0,
    invalid: 0, foreignSnapshots: 0, ignoredObsolete: 0, pendingPeak: 0, pendingOverflow: 0,
    inputs: 0, authenticatedMessages: 0, lastCloseCode: null, terminalReason: null }
  const clear = () => { baseline = null; players.clear(); pending.clear(); streamSeq = -1 }
  const own = id => !retired && sockets.has(id) && current === id
  return {
    open(id) {
      if (retired) return
      assert(!sockets.has(id), 'Duplicate socket observation')
      sockets.add(id); current = id; clear(); admission = null
      counts.opens++; counts.peakSockets = Math.max(counts.peakSockets, sockets.size)
    },
    close(id, code) {
      if (retired || !sockets.delete(id)) return
      if (current === id) { counts.lastCloseCode = code; current = undefined; clear() }
    },
    invalid(id) { if (own(id)) { counts.invalid++; clear() } },
    message(id, direction, message) {
      if (!own(id)) { if (!retired) counts.ignoredObsolete++; return }
      if (direction === 'client') {
        if (message.type === 'authenticate') { counts.authenticatedMessages++; return }
        if (message.type === 'resync') { counts.resyncs++; clear(); return }
        if (message.type !== 'input') return
        counts.inputs++
        if (admission && admission.firstInput === null) admission.firstInput = {
          seq: message.seq, neutral: message.axisX === 0 && message.axisY === 0 && message.buttons.length === 0,
        }
        if (!baseline) return
        pending.add(message.seq)
        counts.pendingPeak = Math.max(counts.pendingPeak, pending.size)
        // An observation failure cannot turn the observer into an unbounded queue.
        if (pending.size > 64) { counts.pendingOverflow++; pending.delete(pending.values().next().value) }
        return
      }
      assert.equal(direction, 'server')
      if (message.type === 'snapshot') {
        if (message.eventId !== eventId || message.selfUserId !== actorId) {
          counts.foreignSnapshots++; clear(); return
        }
        assert(message.players.length <= maximumPlayers, 'Observed cohort exceeds this allocated small functional cohort')
        baseline = { eventId: message.eventId, selfUserId: message.selfUserId, roomId: message.roomId,
          roomEpoch: message.roomEpoch, streamId: message.streamId, cause: message.cause }
        players = new Map(message.players.map(p => [p.userId, { ...p.position }]))
        assert.equal(players.size, message.players.length, 'Duplicate roster membership')
        assert(players.has(actorId), 'Self missing from authoritative baseline')
        pending.clear(); streamSeq = 0; counts.snapshots++; counts.terminalReason = null
        admission = { number: counts.snapshots, firstInput: null }
      } else if (message.type === 'delta') {
        if (!baseline || message.streamId !== baseline.streamId || message.roomEpoch !== baseline.roomEpoch) {
          counts.ignoredObsolete++; return
        }
        if (message.streamSeq !== streamSeq + 1) { counts.gaps++; clear(); return }
        streamSeq = message.streamSeq; counts.deltas++
        for (const id of message.removes) players.delete(id)
        for (const p of message.upserts) players.set(p.userId, { ...p.position })
        if (message.selfMotion) players.set(actorId, { ...message.selfMotion.position })
        assert(players.size <= maximumPlayers, 'Observed cohort exceeds allocation')
        for (const seq of pending) if (seq <= message.ackInputSeq) pending.delete(seq)
      } else if (message.type === 'resyncRequired' || message.type === 'revoke') {
        counts.terminalReason = message.reason; clear()
      }
    },
    read() {
      return { baseline: baseline && { ...baseline }, players: new Map([...players].map(([id, p]) => [id, { ...p }])),
        streamSeq, pending: pending.size, activeSockets: sockets.size,
        admission: admission && { ...admission, firstInput: admission.firstInput && { ...admission.firstInput } } }
    },
    summary() {
      return { ...counts, activeSockets: sockets.size, rosterCount: players.size, pendingInputs: pending.size,
        baselineAvailable: baseline !== null, firstInput: admission?.firstInput && { ...admission.firstInput },
        qualification: 'Observed wire only; private product queue bounds use retained source proof. No HTTP command replay claim.' }
    },
    dispose() { retired = true; sockets.clear(); current = undefined; admission = null; clear() },
  }
}

export function assertPairConverged(observations, renderings) {
  assert.equal(observations.length, 2); assert.equal(renderings.length, 2)
  const a = observations[0].read(), b = observations[1].read()
  assert(a.baseline && b.baseline, 'Two current authoritative baselines required')
  assert.notEqual(a.baseline.selfUserId, b.baseline.selfUserId, 'Independent App identities required')
  for (const key of ['eventId', 'roomId', 'roomEpoch']) assert.equal(a.baseline[key], b.baseline[key], 'Different current world binding')
  for (let i = 0; i < 2; i++) {
    const state = [a, b][i], report = observations[i].summary(), rendering = renderings[i]
    assert.equal(state.activeSockets, 1); assert.equal(report.peakSockets, 1)
    assert.equal(report.invalid + report.foreignSnapshots + report.pendingOverflow, 0)
    assert.equal(state.players.size, 2, 'Exactly the authorized functional pair must remain')
    assert(state.players.has(a.baseline.selfUserId) && state.players.has(b.baseline.selfUserId))
    assert.equal(rendering.games, 1); assert.equal(rendering.canvases, 1)
    assert.equal(rendering.sprites.length, 2, 'Actual renderer contains a missing or ghost player')
    const ordered = points => [...points].map(p => [p.x, p.y]).sort((x, y) => x[0] - y[0] || x[1] - y[1])
    assert.deepEqual(ordered(rendering.sprites), ordered(state.players.values()), 'Actual renderer has not converged to the authoritative roster')
  }
  for (const [id, p] of a.players) assert.deepEqual(p, b.players.get(id), 'Pair authoritative positions differ')
  return { actors: 2, playersPerStream: 2, gamesPerApp: 1, canvasesPerApp: 1, ghostSprites: 0 }
}
