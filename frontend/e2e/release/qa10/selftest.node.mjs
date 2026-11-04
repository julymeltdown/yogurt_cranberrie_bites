import { test } from 'node:test'
import assert from 'node:assert/strict'
import { assertCandidate, digest, inventory, nodeCounts, productionFindings, safePath, vitestCounts, validateReceipt, SCHEMA, GATES } from './model.mjs'

const commit = 'a'.repeat(40), tree = 'b'.repeat(40)
test('candidate binding rejects stale ancestors, wrong trees and dirty input', () => {
  assertCandidate({ commit, tree, dirty: false }, commit, tree)
  for (const candidate of [{ commit: 'c'.repeat(40), tree, dirty: false }, { commit, tree: 'c'.repeat(40), dirty: false }, { commit, tree, dirty: true }]) assert.throws(() => assertCandidate(candidate, commit, tree))
  assert.throws(() => assertCandidate({ commit, tree, dirty: false }, 'HEAD'))
})
test('dispatch retains Node probes renamed outside default discovery and reports real-service suites', () => {
  const result = inventory([
    { path: 'frontend/src/a.test.ts', source: "import {it} from 'vitest'" },
    { path: 'frontend/e2e/privacy/live-gate.node.mjs', source: "import {test} from 'node:test'" },
    { path: 'frontend/e2e/metrics.test.mjs', source: "import {test} from 'node:test'" },
    { path: 'frontend/e2e/real/world.spec.ts', source: "import {test} from '@playwright/test'" },
    { path: 'frontend/e2e/unknown.test.ts', source: 'unknown()' },
  ])
  assert.deepEqual(result.node, ['frontend/e2e/metrics.test.mjs', 'frontend/e2e/privacy/live-gate.node.mjs'])
  assert.equal(result.vitest.length, 1); assert.equal(result.playwright.length, 1); assert.equal(result.unsupported.length, 1)
  assert.throws(() => inventory([]))
})
test('genuine test results require nonempty exact file coverage, retaining skipped cases', () => {
  const f = { name: '/repo/frontend/src/a.test.ts', assertionResults: [{ status: 'passed' }, { status: 'pending', fullName: 'requires real service' }] }
  const r = { numTotalTests: 2, numPassedTests: 1, numFailedTests: 0, numPendingTests: 1, testResults: [f] }
  assert.equal(vitestCounts(r, ['frontend/src/a.test.ts'], '/repo').skipped_tests.length, 1)
  assert.throws(() => vitestCounts({ ...r, numPassedTests: 0 }, ['frontend/src/a.test.ts'], '/repo'))
  assert.throws(() => vitestCounts(r, ['frontend/src/b.test.ts'], '/repo'))
  assert.throws(() => vitestCounts({ ...r, testResults: [{ ...f, assertionResults: [] }] }, ['frontend/src/a.test.ts'], '/repo'))
  assert.equal(nodeCounts('# tests 2\n# pass 1\n# fail 0\n# skipped 1\n# todo 0\n').total, 2)
  assert.throws(() => nodeCounts('# tests 0\n# pass 0\n# fail 0\n# skipped 0\n# todo 0\n'))
  assert.throws(() => nodeCounts('runner launched'))
})
test('production checks inspect emitted fixture/mock modules and secret markers without exporting values', () => {
  assert.deepEqual(productionFindings(['frontend/src/app/router.tsx'], [{ path: 'assets/app.js', text: 'const token=getToken()' }]), [])
  const findings = productionFindings(['frontend/src/app/__tests__/fixture.ts', 'frontend/src/world/network/mock.ts'], [{ path: 'assets/app.js', text: '-----BEGIN PRIVATE KEY-----\nSECRET' }])
  assert.equal(findings.length, 3); assert.ok(!JSON.stringify(findings).includes('SECRET'))
  assert.equal(productionFindings([], [{ path: 'x.js', text: 'const accessToken="sensitive-test-marker"' }]).length, 1)
})
test('receipt consumer refuses missing, duplicate, empty or falsely passing gates', () => {
  const r = { schema: SCHEMA, status: 'PARTIAL', same_candidate_end: true, candidate: { commit, tree, dirty: false },
    commands: GATES.map(id => ({ id, argv: ['actual', 'command'], cwd: 'frontend', status: 'PASS', exit_code: 0, nonempty_count: 1, log: `${id}.log`, report_hash: digest('log') })),
    unsupported: [{ criterion: 'live services', status: 'NOT_RUN' }], source_pins: { x: digest('x') }, contract_pins: { x: digest('x') } }
  assert.equal(validateReceipt(r, commit), 'PASS')
  for (const mutate of [v => { v.candidate.commit = 'c'.repeat(40) }, v => { v.same_candidate_end = false }, v => { v.commands.pop() }, v => { v.commands[1] = v.commands[0] }, v => { v.commands[0].nonempty_count = 0 }, v => { v.commands[0].exit_code = 1 }, v => { v.status = 'PASS' }, v => { v.unsupported = [] }]) {
    const value = structuredClone(r); mutate(value); assert.throws(() => validateReceipt(value, commit))
  }
})
test('artifact paths reject escapes and absolute paths', () => {
  for (const path of ['../outside', '/outside', 'a/../b', 'a\\b', '']) assert.equal(safePath(path), false)
  assert.equal(safePath('reports/vitest.json'), true)
})
