import { test } from 'node:test'
import assert from 'node:assert/strict'
import { actions } from './actions.mjs'
import { observeRecoveryCase } from './journey.mjs'

// Synthetic action safety checks only. They do not execute or qualify the product.
const jobId = '10000000-0000-4000-8000-000000000001'
function closurePage({ actualJob = jobId, revisionVisible = true } = {}) {
  const clicks = []
  const review = {
    locator() { return { innerText: async () => actualJob } },
    getByText() { return { isVisible: async () => revisionVisible } },
    getByRole(_role, { name }) { return { click: async () => clicks.push(name) } },
  }
  const form = {
    getByRole(role, { name }) {
      return role === 'textbox' ? { fill: async () => {} } : { click: async () => clicks.push(name) }
    },
  }
  return { clicks, getByRole(role) { return role === 'form' ? form : review } }
}

test('explicit closure resume checks exact expected job before destructive confirmation', async () => {
  const wrong = closurePage({ actualJob: '20000000-0000-4000-8000-000000000001' })
  await assert.rejects(actions.resumeClosure(wrong, { reason: 'Recover exact original job', jobId, expectedRevision: 4 }), /QA09_RESUME_REVIEW_MISMATCH/)
  assert.equal(wrong.clicks.includes('Confirm resume closure'), false)
})

test('changed review revision cannot be confirmed', async () => {
  const stale = closurePage({ revisionVisible: false })
  await assert.rejects(actions.resumeClosure(stale, { reason: 'Recover exact original job', jobId, expectedRevision: 4 }), /QA09_RESUME_REVIEW_MISMATCH/)
  assert.equal(stale.clicks.includes('Confirm resume closure'), false)
})

test('missing exact revision/reason cannot even prepare a resume', async () => {
  const page = closurePage()
  await assert.rejects(actions.resumeClosure(page, { reason: '', jobId, expectedRevision: 4 }), /QA09_RESUME_REASON_REQUIRED/)
  await assert.rejects(actions.resumeClosure(page, { reason: 'recover', jobId, expectedRevision: 0 }), /QA09_RESUME_BINDING_REQUIRED/)
  assert.deepEqual(page.clicks, [])
})

test('correct review issues exactly one explicit resume confirmation', async () => {
  const page = closurePage()
  await actions.resumeClosure(page, { reason: 'Recover exact original job', jobId, expectedRevision: 4 })
  assert.deepEqual(page.clicks, ['Retry failed closure steps', 'Confirm resume closure'])
})

test('unqualified/missing runtime ports cannot manufacture a frontend observation', async () => {
  await assert.rejects(observeRecoveryCase({ caseId: 'QA09-DELETION' }), /QA09_RUNTIME_PORTS_REQUIRED/)
})

test('expired resource signal aborts before any page access or action', async () => {
  const controller = new AbortController(); controller.abort()
  let calls = 0
  await assert.rejects(observeRecoveryCase({ caseId: 'QA09-PENDING-PICKUP',
    guard: () => { calls++ }, waitBoundary() { calls++ }, waitForOutcome() { calls++ }, signal: controller.signal }))
  assert.equal(calls, 0)
})

test('resource cancellation interrupts a stalled guard without a DOM/action or fake success', async () => {
  const controller = new AbortController()
  const run = observeRecoveryCase({ caseId: 'QA09-PENDING-PICKUP',
    guard: () => new Promise(() => {}), waitBoundary() { assert.fail('No fault rendezvous after cancellation') },
    waitForOutcome() { assert.fail('No fake outcome after cancellation') }, signal: controller.signal })
  controller.abort()
  await assert.rejects(run, /QA09_RESOURCE_OR_RUNTIME_WINDOW_ENDED/)
})

test('upstream guard errors never export private diagnostic values and preserve partial state', async () => {
  const controller = new AbortController()
  await assert.rejects(observeRecoveryCase({ caseId: 'QA09-DELETION',
    guard() { throw new Error('PRIVATE_TOKEN_IN_ERROR') }, waitBoundary() {}, waitForOutcome() {}, signal: controller.signal }), error => {
    assert.equal(error.message, 'QA09_FRONTEND_OBSERVATION_FAILED')
    assert.equal(error.cause, undefined)
    assert.equal(error.observation.status, 'IN_PROGRESS')
    assert.equal(error.observation.boundaryObserved, false)
    assert.equal(JSON.stringify(error.observation).includes('PRIVATE_TOKEN'), false)
    return true
  })
})
