// Consumer of Root's future managed checkpoints, never a fault controller.
import assert from 'node:assert/strict'
import { createCheckpoints, recoveryCases } from './checkpoints.mjs'
import { assertPairConverged } from './observations.mjs'

async function runRecoveryCase({ caseId, control, session }) {
  assert(Object.hasOwn(recoveryCases, caseId))
  assert.equal(session.kind, 'QA05_ACTUAL_DEFAULT_APP_PAIR', 'Actual App attachment required')
  const before = await session.sample()
  const ready = assertPairConverged(before.observations, before.renderings)
  // Capture values now: observations themselves continue to receive real frames.
  const beforeState = before.observations.map(observation => ({ baseline: { ...observation.read().baseline }, counts: observation.summary() }))
  const action = await control.checkpoint(caseId, 'frontend-ready', { ...ready, qualification: 'Actual App wire and renderer observation' })
  // ARMED installs/accepts the action. Actual completion is separately observed.
  if (caseId === 'world-restart') {
    await control.checkpoint(caseId, 'producer-ready', ready)
    await session.reconnect()
  }
  if (caseId === 'foreground') await session.foreground()
  else if (caseId === 'room-switch') await session.switchRoom()
  else if (['packet-gap', 'packet-delay', 'slow-client'].includes(caseId)) await session.move()
  const after = await session.awaitConvergence({ caseId, beforeState, action })
  const convergence = assertPairConverged(after.observations, after.renderings)
  if (caseId === 'world-restart') {
    for (let i = 0; i < 2; i++) {
      assert.notEqual(after.observations[i].read().baseline.roomEpoch, beforeState[i].baseline.roomEpoch, 'Real replacement epoch required')
      assert.notEqual(after.observations[i].read().baseline.streamId, beforeState[i].baseline.streamId, 'Fresh actual stream required')
    }
  }
  if (caseId === 'foreground') {
    assert.deepEqual(after.observations[0].read().admission.firstInput, { seq: 1, neutral: true })
    assert(after.trustedVisibility?.some(v => v.hidden && v.trusted) && after.trustedVisibility?.some(v => !v.hidden && v.trusted), 'Genuine browser visibility required')
  }
  await control.checkpoint(caseId, 'after-observation', convergence)
  // Owner raw observations may contain principal/room/process identity. Root
  // retains their qualified receipt privately; frontend durable results contain
  // no arbitrary callback body or private backend handles.
  return { caseId, status: 'PASS', convergence, ownerCheckpointCompleted: true,
    qualification: 'Combined actual default App and managed producer, only when the actual attachment executes this driver. Source fixtures are not live evidence.' }
}

export async function runRecoveryCases({ caseIds, binding, checkpoint, session }) {
  assert(Array.isArray(caseIds) && caseIds.length > 0 && caseIds.length <= 7)
  assert.equal(new Set(caseIds).size, caseIds.length, 'Never repeat a fault within one owner window')
  for (const caseId of caseIds) assert(Object.hasOwn(recoveryCases, caseId))
  const control = createCheckpoints({ binding, invoke: checkpoint }), results = []
  for (const caseId of caseIds) results.push(await runRecoveryCase({ caseId, control, session }))
  return results
}
