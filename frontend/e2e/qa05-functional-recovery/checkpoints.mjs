// Consumer ABI proposal. Root owns the actual checkpoint/transparent proxy/child.
// Importing this module creates no controller, service, credentials or authority.
import assert from 'node:assert/strict'

export const checkpointVersion = 1
export const recoveryCases = Object.freeze({
  'packet-gap': { action: 'drop-next-delta', maximumFrames: 1, maximumMs: 1000 },
  'packet-delay': { action: 'delay-downstream', maximumFrames: 8, maximumBytes: 262144, maximumMs: 250 },
  'slow-client': { action: 'pause-downstream', maximumFrames: 64, maximumBytes: 262144, maximumMs: 1000 },
  foreground: { action: 'observe-only' },
  'ticket-replay': { action: 'probe-consumed-ticket', maximumConnections: 1, maximumMs: 3000 },
  'world-restart': { action: 'restart-owned-world', maximumReplacements: 1, maximumMs: 60000 },
  'room-switch': { action: 'observe-only' },
})
for (const value of Object.values(recoveryCases)) Object.freeze(value)

export function createCheckpoints({ binding, invoke, now = Date.now }) {
  assert.equal(typeof invoke, 'function', 'QA05_OWNER_CHECKPOINT_NOT_BOUND')
  assert.deepEqual(Object.keys(binding).sort(), ['candidate', 'environmentId', 'eventId', 'expiresAt', 'windowId'].sort(), 'Only the public owner-window binding may cross this ABI')
  assert.match(binding.candidate, /^[a-f0-9]{40}$/)
  for (const key of ['environmentId', 'eventId', 'windowId']) assert.match(binding[key], /^[a-f0-9]{8}(?:-[a-f0-9]{4}){3}-[a-f0-9]{12}$/)
  assert(Number.isFinite(Date.parse(binding.expiresAt)))
  const frozen = Object.freeze({ ...binding }), stages = new Map()
  let sequence = 0, unavailable = false, inFlight = false
  async function checkpoint(caseId, step, observation) {
    assert(!unavailable, 'QA05_OWNER_CHECKPOINT_UNAVAILABLE')
    assert(!inFlight, 'Concurrent fault checkpoints are prohibited')
    assert(Object.hasOwn(recoveryCases, caseId), 'Unknown QA05 case')
    const steps = caseId === 'world-restart' ? ['frontend-ready', 'producer-ready', 'after-observation'] : ['frontend-ready', 'after-observation']
    assert(steps.includes(step))
    const previous = steps.indexOf(step) === 0 ? 'not-started' : steps[steps.indexOf(step) - 1]
    assert.equal(stages.get(caseId) ?? 'not-started', previous, 'Checkpoint order violation')
    assert(Date.parse(frozen.expiresAt) - now() > 5000, 'QA05_OWNER_WINDOW_EXPIRED')
    if (caseId === 'world-restart' && step === 'frontend-ready') assert(Date.parse(frozen.expiresAt) - now() >= 65000, 'QA05_WORLD_RESTART_RESERVE_REQUIRED')
    const request = { version: checkpointVersion, binding: frozen, sequence: ++sequence, caseId, step,
      actor: 'actor-a', request: recoveryCases[caseId], observation }
    const signal = AbortSignal.timeout(Math.min(step === 'producer-ready' ? 60000 : 35000, Date.parse(frozen.expiresAt) - now() - 5000))
    inFlight = true
    let abort
    let response
    try {
      response = await Promise.race([Promise.resolve().then(() => invoke(request, { signal })),
        new Promise((_, reject) => {
          abort = () => reject(new Error('QA05_OWNER_CHECKPOINT_TIMED_OUT'))
          signal.addEventListener('abort', abort, { once: true })
        })])
    } catch {
      // A fault may already have happened. Never issue the same action again.
      unavailable = true; throw new Error('QA05_OWNER_CHECKPOINT_UNCERTAIN')
    } finally { inFlight = false; signal.removeEventListener('abort', abort) }
    try {
      assert(response && response.version === checkpointVersion, 'Invalid checkpoint version')
      for (const key of ['candidate', 'environmentId', 'eventId', 'windowId', 'expiresAt']) assert.equal(response.binding?.[key], frozen[key], 'Foreign owner-window proof')
      for (const key of ['sequence', 'caseId', 'step']) assert.equal(response[key], request[key], 'Uncorrelated owner checkpoint')
      assert(Date.parse(frozen.expiresAt) - now() > 5000 && !signal.aborted, 'QA05_OWNER_WINDOW_EXPIRED')
      assert(['ARMED', 'OBSERVED', 'APPLIED', 'UNAVAILABLE'].includes(response.status))
      if (response.status === 'UNAVAILABLE') { unavailable = true; throw new Error('QA05_OWNER_CHECKPOINT_UNAVAILABLE') }
      assert.equal(response.action, request.request.action, 'Fault action differs from bounded request')
      assert.equal(response.status, request.request.action === 'observe-only' ? 'OBSERVED'
        : step === 'frontend-ready' ? 'ARMED' : 'APPLIED', 'Arming or observation is not an executed fault')
      assert(response.proof && typeof response.proof === 'object' && !Array.isArray(response.proof), 'Actual owner proof required')
      stages.set(caseId, step)
      return response.proof
    } catch (error) { unavailable = true; throw error }
  }
  return Object.freeze({ checkpoint, binding: frozen, disposition: () => Object.fromEntries(stages) })
}
