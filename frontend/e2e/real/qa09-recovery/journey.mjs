import { caseDefinition } from './cases.mjs'
import { actions, observeDom } from './actions.mjs'
import { createManagedAdmissionGuard } from './managed-admission.mjs'

/** Root's existing managed session supplies this config/port/page and accepted
 * finite deadline. Keep the original injected driver for owner composition;
 * this entry binds its guards to current managed observations before any action.
 */
export async function observeManagedRecoveryCase({ config, runtime, deadlineEpochMs, ...options }) {
  // The current IN04 observer reauthorizes all three actors on every inspect.
  // Deletion/closure can intentionally withdraw those actors or event access;
  // they require an actual lifecycle-aware owner observer, not this live-profile guard.
  if (!['QA09-DB-RESTART', 'QA09-BACKUP-RESTORE', 'QA09-OUTBOX-REPLAY', 'QA09-PENDING-PICKUP'].includes(options.caseId)) {
    throw new Error('QA09_LIFECYCLE_OBSERVER_REQUIRED')
  }
  const guard = createManagedAdmissionGuard({ config, runtime, page: options.page,
    signal: options.signal, deadlineEpochMs })
  return observeRecoveryCase({ ...options, guard })
}

/** Source-first reusable driver. The actual runtime owner supplies a qualified page,
 * immutable run binding and backend boundary rendezvous. No native launcher is
 * enabled until those interfaces and a resource slot are explicitly available.
 * This produces observations, never whole-QA09 acceptance from a callback/fixture.
 */
export async function observeRecoveryCase({ caseId, page, observer, guard, waitBoundary, waitForOutcome, reason, jobId, expectedRevision, signal }) {
  const definition = caseDefinition(caseId)
  if (typeof guard !== 'function' || typeof waitBoundary !== 'function' || typeof waitForOutcome !== 'function'
    || !(signal instanceof AbortSignal)) throw new Error('QA09_RUNTIME_PORTS_REQUIRED')
  const bounded = async fn => {
    signal.throwIfAborted()
    let abort
    const interrupted = new Promise((_, reject) => {
      abort = () => reject(new Error('QA09_RESOURCE_OR_RUNTIME_WINDOW_ENDED'))
      signal.addEventListener('abort', abort, { once: true })
    })
    try { return await Promise.race([Promise.resolve().then(fn), interrupted]) }
    finally { signal.removeEventListener('abort', abort) }
  }
  const observation = { caseId, status: 'IN_PROGRESS', before: null, after: null, boundaryObserved: false,
    qualification: 'Frontend observations only. Backend fault execution, restored-state consistency and durable-effect assertions require separately bound actual producer evidence.' }
  try {
    await bounded(guard)
    observation.before = { dom: await bounded(() => observeDom(page)), transport: observer.snapshot() }
    // This rendezvous is notification only: frontend must never restart/restore a DB.
    await bounded(() => waitBoundary({ caseId, boundary: definition.boundary, signal }))
    observation.boundaryObserved = true
    await bounded(guard)
    await bounded(() => actions[definition.action](page, { reason, jobId, expectedRevision }))
    await bounded(() => waitForOutcome({ caseId, page, signal }))
    await bounded(guard)
    observation.after = { dom: await bounded(() => observeDom(page)), transport: observer.snapshot() }
    return observation
  } catch {
    // Playwright exceptions can contain DOM/URL/response excerpts. Preserve only
    // our safe partial observation, never the original exception or its cause.
    const error = new Error(signal.aborted ? 'QA09_RESOURCE_OR_RUNTIME_WINDOW_ENDED' : 'QA09_FRONTEND_OBSERVATION_FAILED')
    error.observation = observation
    throw error
  }
}
