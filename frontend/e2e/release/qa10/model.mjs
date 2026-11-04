import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'

export const SCHEMA = 'mcr.qa10.frontend.v1'
export const GATES = ['dependencies', 'python-formats', 'typecheck', 'lint', 'vitest', 'node-tests', 'build', 'ui-origin', 'production-static', 'task-trace']
export const digest = bytes => createHash('sha256').update(bytes).digest('hex')
export const safePath = value => typeof value === 'string' && value.length > 0 && !value.includes('\\') && !value.includes('\0') && !value.startsWith('/') && value.split('/').every(p => p && p !== '.' && p !== '..')

export function assertCandidate(candidate, expected, tree = candidate.tree) {
  assert.match(expected, /^[a-f0-9]{40}$/, 'An exact candidate commit is required')
  assert.equal(candidate.commit, expected, 'Candidate mismatch; ancestry alone is insufficient')
  assert.equal(candidate.tree, tree, 'Candidate tree mismatch')
  assert.equal(candidate.dirty, false, 'Dirty candidate cannot be qualified')
}

export function inventory(entries) {
  const result = { vitest: [], node: [], playwright: [], unsupported: [] }
  for (const { path, source } of entries) {
    if (!path.startsWith('frontend/') || !/\.[cm]?[jt]sx?$/.test(path)) continue
    const named = /\.(?:test|spec|node-test)\.[cm]?[jt]sx?$/.test(path)
    if (/(?:from\s*|import\s*\()['"]node:test['"]/.test(source)) result.node.push(path)
    else if (named) {
      const runner = /['"]vitest['"]/.test(source) ? 'vitest' : /['"]@playwright\/test['"]/.test(source) ? 'playwright' : 'unsupported'
      result[runner].push(path)
    }
  }
  for (const paths of Object.values(result)) paths.sort()
  assert.ok(result.vitest.length && result.node.length, 'Both supported runners require nonempty inventories')
  return result
}

export function vitestCounts(report, paths, root) {
  assert.ok(report.numTotalTests > 0 && report.numPassedTests > 0, 'Vacuous Vitest result')
  const actual = report.testResults.map(f => f.name.replace(root + '/', '')).sort()
  assert.deepEqual(actual, [...paths].sort(), 'Vitest file inventory differs from candidate discovery')
  for (const f of report.testResults) assert.ok(f.assertionResults.length, `No tests collected: ${f.name}`)
  return { total: report.numTotalTests, passed: report.numPassedTests, failed: report.numFailedTests,
    skipped: report.numPendingTests ?? 0, files: actual.length,
    skipped_tests: report.testResults.flatMap(f => f.assertionResults.filter(a => ['pending', 'todo', 'skipped'].includes(a.status)).map(a => ({ file: f.name.replace(root + '/', ''), name: a.fullName }))) }
}

export function nodeCounts(text) {
  const count = key => Number(text.match(new RegExp(`^# ${key} (\\d+)$`, 'm'))?.[1] ?? NaN)
  const result = { total: count('tests'), passed: count('pass'), failed: count('fail'), skipped: count('skipped'), todo: count('todo') }
  assert.ok(result.total > 0 && result.passed > 0 && Object.values(result).every(Number.isFinite), 'Vacuous or incomplete Node test footer')
  return result
}

/** A narrow emitted-module/content check. It cannot certify all secrets or authorization. */
export function productionFindings(modules, assets) {
  const findings = []
  for (const path of modules) if (/(?:^|\/)(?:__tests__|fixtures?|e2e|mocks?)(?:\/|\.)/i.test(path) || /(?:^|\/)msw(?:\/|$)/.test(path)) findings.push({ rule: 'fixture-or-mock-module', path })
  for (const { path, text } of assets) {
    for (const [rule, pattern] of [
      ['private-key', /-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----/],
      ['jwt-literal', /eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}/],
      ['credential-literal', /\b(?:accessToken|refreshToken|password|client_secret)\s*[:=]\s*["'][^"'\n]{12,}["']/],
    ]) if (pattern.test(text)) findings.push({ rule, path })
  }
  return findings
}

export function validateReceipt(report, candidate) {
  assert.equal(report.schema, SCHEMA)
  assertCandidate(report.candidate, candidate)
  assert.equal(report.same_candidate_end, true, 'Candidate was not rechecked after execution')
  assert.ok(['PARTIAL', 'FAIL'].includes(report.status), 'Frontend receipt never grants global acceptance')
  assert.equal(new Set(report.commands.map(c => c.id)).size, GATES.length, 'Duplicate or missing gates')
  assert.deepEqual(report.commands.map(c => c.id).sort(), [...GATES].sort(), 'Required gate inventory mismatch')
  for (const c of report.commands) {
    assert.ok(Array.isArray(c.argv) && c.argv.length && safePath(c.cwd), 'Missing command provenance')
    assert.ok(safePath(c.log) && /^[a-f0-9]{64}$/.test(c.report_hash), 'Missing command log pin')
    assert.ok(['PASS', 'FAIL', 'NOT_RUN', 'BLOCKED'].includes(c.status), 'Unknown gate state')
    if (c.status === 'PASS') {
      assert.equal(c.exit_code, 0, 'PASS cannot hide a failed process')
      assert.ok(c.nonempty_count > 0, 'PASS cannot have empty coverage')
    }
  }
  assert.ok(report.unsupported.length, 'Missing whole-QA10 limitations')
  assert.ok(Object.keys(report.source_pins).length && Object.keys(report.contract_pins).length, 'Missing source/contract pins')
  const passed = report.commands.every(c => c.status === 'PASS')
  if (!passed) assert.equal(report.status, 'FAIL', 'Partial status cannot hide failed gates')
  return passed ? 'PASS' : 'FAIL'
}
