import { test } from 'node:test'
import assert from 'node:assert/strict'
import { producerProjection, producerReportProjection } from './producer-binding.mjs'
import { cases } from './cases.mjs'

// Synthetic envelope guards, never executed producer/recovery evidence.
const expected = { caseId: 'QA09-BACKUP-RESTORE', runId: 'a'.repeat(32), backend: 'b'.repeat(40), frontend: 'c'.repeat(40) }
const fixture = () => ({ version: 1, runId: expected.runId, caseId: expected.caseId,
  sourceRevisions: { backend: expected.backend, frontend: expected.frontend }, state: 'NOT_RUN',
  phases: { before: '2026-11-04T12:00:00Z', boundary: null, recovered: '2026-11-04T12:00:01+00:00' },
  producerEvidence: { path: 'native.json', sha256: 'd'.repeat(64) },
  observations: { supportingPhysicalRestorePassed: true }, limitations: ['PRIVATE_DIAGNOSTIC_DO_NOT_EXPORT'] })

test('successful segment with missing original boundary remains NOT_RUN, opaque BE fields discarded', () => {
  const projection = producerProjection(fixture(), expected)
  assert.equal(projection.state, 'NOT_RUN'); assert.equal(projection.hasOriginalBoundary, false)
  assert.equal(JSON.stringify(projection).includes('supportingPhysicalRestorePassed'), false)
  assert.equal(JSON.stringify(projection).includes('PRIVATE_DIAGNOSTIC'), false)
})

test('run, case and candidate substitution are rejected', () => {
  for (const patch of [{ runId: 'e'.repeat(32) }, { caseId: 'QA09-DELETION' },
    { sourceRevisions: { backend: 'e'.repeat(40), frontend: expected.frontend } },
    { sourceRevisions: { backend: expected.backend, frontend: 'e'.repeat(40) } }]) {
    assert.throws(() => producerProjection({ ...fixture(), ...patch }, expected), /QA09_PRODUCER_BINDING_INVALID/)
  }
})

test('missing actual phases cannot be replaced by malformed or out-of-order timestamps', () => {
  for (const phases of [{ before: null, boundary: '2026-11-04T12:00:00Z', recovered: null },
    { before: '2026-11-04T12:00:01Z', boundary: '2026-11-04T12:00:00Z', recovered: null },
    { before: '2026-11-04T12:00:00+09:00', boundary: null, recovered: null },
    { before: '2026-02-30T12:00:00Z', boundary: null, recovered: null }]) {
    assert.throws(() => producerProjection({ ...fixture(), phases }, expected), /QA09_PRODUCER_BINDING_INVALID/)
  }
})

test('artifact traversal, foreign paths, raw URLs and false accepted states are rejected', () => {
  for (const path of ['../native.json', '/private/native.json', 'https://example.com/native.json', '.hidden']) {
    assert.throws(() => producerProjection({ ...fixture(), producerEvidence: { path, sha256: 'd'.repeat(64) } }, expected))
  }
  assert.throws(() => producerProjection({ ...fixture(), state: 'PASS' }, expected))
  assert.throws(() => producerProjection({ ...fixture(), state: 'ACCEPTED' }, expected))
})

test('unknown frontend pin and all-null phase placeholders cannot qualify the original boundary', () => {
  const projection = producerProjection({ ...fixture(), sourceRevisions: { backend: expected.backend },
    producerEvidence: null, phases: { before: null, boundary: null, recovered: null } }, { ...expected, frontend: null })
  assert.equal(projection.frontendSourceBound, false); assert.equal(projection.hasOriginalBoundary, false)
})

const reportFixture = () => ({ version: 1, runId: expected.runId, state: 'IN_PROGRESS', wholeQa09Accepted: false,
  sourceRevisions: { backend: expected.backend },
  cases: Object.keys(cases).map(caseId => ({ ...fixture(), caseId })) })

test('actual six-case producer container selects only the exact expected case', () => {
  const report = reportFixture()
  report.cases.find(item => item.caseId !== expected.caseId).producerEvidence = null
  const projection = producerReportProjection(report, expected)
  assert.equal(projection.caseId, expected.caseId)
  assert.equal(projection.state, 'NOT_RUN')
  assert.equal(projection.hasOriginalBoundary, false)
})

test('container substitution, missing/duplicate cases and false whole acceptance are rejected', () => {
  const original = reportFixture()
  for (const patch of [{ runId: 'f'.repeat(32) }, { sourceRevisions: { backend: 'f'.repeat(40) } },
    { wholeQa09Accepted: true }, { cases: original.cases.slice(1) },
    { cases: [...original.cases.slice(1), original.cases[1]] }]) {
    assert.throws(() => producerReportProjection({ ...original, ...patch }, expected), /QA09_PRODUCER_BINDING_INVALID/)
  }
})

test('a foreign case run cannot borrow the correct enclosing report identity', () => {
  const report = reportFixture()
  report.cases[0].runId = 'f'.repeat(32)
  assert.throws(() => producerReportProjection(report, expected), /QA09_PRODUCER_BINDING_INVALID/)
})
