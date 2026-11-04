import assert from 'node:assert/strict'
import { spawn, execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { closeSync, existsSync, lstatSync, mkdirSync, openSync, readFileSync, readdirSync, readlinkSync, realpathSync, rmSync, statfsSync, symlinkSync, unlinkSync, writeFileSync } from 'node:fs'
import { dirname, isAbsolute, join } from 'node:path'
import { root, inputManifest } from './cohort.mjs'

// Run ONLY after accepting a named resource allocation. This script grants no allocation.
// Fixed absolute cwd, immutable active cutoff, checked setup, one child at a time, owned cleanup.
const required = key => { const value = process.env[key]; assert.ok(value, `Missing ${key}`); return value }
const output = required('MCR_C07_ALLOCATION_ROOT')
const dependencies = required('MCR_C07_DEPENDENCIES')
const pinned = required('MCR_C07_PINNED_NODE')
const allocation = required('MCR_C07_ALLOCATION')
const runId = required('MCR_C07_RUN_ID')
const cutoff = Date.parse(required('MCR_C07_ACTIVE_UNTIL'))
assert.ok(/^[a-z0-9-]{1,50}$/.test(runId), 'Invalid run identifier')
assert.ok([output, dependencies, pinned].every(isAbsolute), 'Allocation and input paths must be absolute')
assert.ok(!output.startsWith(root) && existsSync(output), 'Use the existing granted private allocation root')
assert.ok(Number.isFinite(cutoff) && cutoff > Date.now() + 45000 && cutoff < Date.now() + 900000, 'Invalid active cutoff; cleanup reserve is separate')
const frontend = join(root, 'frontend'), evidence = join(root, 'docs/mcr/evidence/c07-social-client-in03')
const link = join(frontend, 'node_modules'), cache = join(output, 'cache'), tmp = join(output, 'tmp')
const hash = file => createHash('sha256').update(readFileSync(file)).digest('hex')
const present = path => { try { lstatSync(path); return true } catch (error) { if (error.code === 'ENOENT') return false; throw error } }
const env = { ...process.env, PATH: dirname(pinned) + ':' + process.env.PATH, TMPDIR: tmp,
  MCR_C07_CACHE: cache, NODE_DISABLE_COMPILE_CACHE: '1', NO_COLOR: '1' }
delete env.MCR_C07_REAL_CONFIG; delete env.MCR_C07_DRIFT_PROBE; delete env.NODE_COMPILE_CACHE
const expectedNode = 'v' + JSON.parse(readFileSync(join(frontend, 'package.json'), 'utf8')).engines.node
assert.equal(process.version, expectedNode, 'Parent runtime must be the retained Node version')
assert.equal(realpathSync(process.execPath), realpathSync(pinned), 'Invoke the absolute pinned Node binary')
assert.equal(execFileSync('node', ['--version'], { env, encoding: 'utf8' }).trim(), expectedNode, 'All child PATH probes must resolve pinned Node')
assert.equal(hash(join(frontend, 'pnpm-lock.yaml')), hash(join(dirname(dependencies), 'pnpm-lock.yaml')), 'Read-only dependency lock mismatch')
assert.deepEqual(inputManifest(), JSON.parse(readFileSync(join(evidence, 'inputs.json'), 'utf8')), 'Public cohort changed')
assert.ok(!present(link) && !present(cache) && !present(tmp), 'Owned setup paths must be absent; never replace another owner')
const sources = [join(frontend, 'e2e/real/social'), join(frontend, 'e2e/real/social.spec.ts'),
  join(frontend, 'src/shared/api/__tests__/c07-social-client'), evidence, join(root, 'docs/mcr/frontend/c07-social-client-in03.md')]
function usage() {
  const files = new Set()
  const visit = path => {
    if (!present(path)) return
    const info = lstatSync(path)
    if (info.isDirectory()) for (const name of readdirSync(path)) visit(join(path, name))
    else files.add(path)
  }
  for (const path of [output, ...sources, link]) visit(path)
  const disk = statfsSync(output)
  return { files: files.size, bytes: [...files].reduce((sum, path) => sum + lstatSync(path).size, 0), freeBytes: disk.bavail * disk.bsize }
}
const initial = usage()
assert.ok(initial.freeBytes >= 16 * 1024 ** 3 && initial.files < 200 && initial.bytes < 100 * 1024 ** 2, 'Fresh resource preflight failed')
const summaryPath = join(output, runId + '-summary.json')
assert.ok(!present(summaryPath), 'Never overwrite an earlier run')
const summary = { allocation, runId, source: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(),
  node: process.version, nodeSha256: hash(pinned), lockSha256: hash(join(frontend, 'pnpm-lock.yaml')), activeUntil: new Date(cutoff).toISOString(), initial, jobs: [], status: 'IN_PROGRESS' }
const config = 'src/shared/api/__tests__/c07-social-client/vitest.config.ts'
const jobs = [
  ['normal', ['node_modules/vitest/vitest.mjs', 'run', '--configLoader', 'native', '--config', config], 0],
  ['drift', ['node_modules/vitest/vitest.mjs', 'run', 'src/shared/api/__tests__/c07-social-client/drift-probe.test.ts', '--configLoader', 'native', '--config', config], 1],
  ['tsc', ['node_modules/typescript/bin/tsc', '-p', 'src/shared/api/__tests__/c07-social-client/tsconfig.json', '--noEmit', '--incremental', 'false', '--pretty', 'false'], 0],
  ['admission', ['e2e/real/social/admission.mjs'], 2],
]
let ownsLink = false, ownsCache = false, ownsTmp = false
try {
  mkdirSync(cache); ownsCache = true
  mkdirSync(tmp); ownsTmp = true
  symlinkSync(dependencies, link); ownsLink = true
  // No compiler/test child can be launched before ALL setup assertions succeeded.
  assert.equal(realpathSync(link), realpathSync(dependencies))
  for (const [name, args, expected] of jobs) {
    assert.ok(Date.now() < cutoff, 'Active cutoff reached')
    const log = join(output, `${runId}-${name}.log`), fd = openSync(log, 'wx')
    const start = Date.now(), childEnv = { ...env, ...(name === 'drift' ? { MCR_C07_DRIFT_PROBE: 'method' } : {}) }
    let stopped = false
    const child = spawn(pinned, args, { cwd: frontend, env: childEnv, stdio: ['ignore', fd, fd], detached: true })
    const monitor = setInterval(() => {
      const current = usage()
      if (Date.now() >= cutoff || current.freeBytes < 8 * 1024 ** 3 || current.files > 200 || current.bytes > 100 * 1024 ** 2) {
        stopped = true
        try { process.kill(-child.pid, 'SIGTERM') } catch { /* Exit may already be in flight. */ }
      }
    }, 250)
    let result
    try { result = await new Promise((resolveJob, reject) => { child.once('error', reject); child.once('exit', (exit, signal) => resolveJob({ exit, signal })) }) }
    finally { clearInterval(monitor); closeSync(fd) }
    const record = { name, command: [pinned, ...args], environment: { PATH_PREFIX: dirname(pinned), TMPDIR: tmp, MCR_C07_CACHE: cache, ...(name === 'drift' ? { MCR_C07_DRIFT_PROBE: 'method' } : {}) },
      startUTC: new Date(start).toISOString(), milliseconds: Date.now() - start, ...result, expected, stopped }
    summary.jobs.push(record); console.log(JSON.stringify(record))
    assert.ok(!stopped && result.exit === expected, `Unexpected ${name} result; repair only within the accepted remaining round`)
    if (name === 'drift') assert.match(readFileSync(log, 'utf8'), /C07 metadata drift createConcertSocialRequest: method/)
    if (name === 'admission') assert.equal(JSON.parse(readFileSync(log, 'utf8')).status, 'NOT_RUN')
  }
  summary.status = 'FIXTURE_CHECKS_PASSED'; summary.IN03 = 'NOT_RUN'; summary.C07 = 'UNRECONCILED'
} catch (error) { summary.status = 'FAILED'; summary.error = error.message; process.exitCode = 1 }
finally {
  if (ownsLink) { assert.equal(readlinkSync(link), dependencies); unlinkSync(link) }
  if (ownsCache) rmSync(cache, { recursive: true })
  if (ownsTmp) rmSync(tmp, { recursive: true })
  summary.releasedUTC = new Date().toISOString(); summary.final = usage(); summary.ownedLinkRemoved = !present(link)
  writeFileSync(summaryPath, JSON.stringify(summary, null, 2) + '\n', { flag: 'wx' })
  console.log(JSON.stringify({ status: summary.status, releasedUTC: summary.releasedUTC, usage: usage(), ownedLinkRemoved: summary.ownedLinkRemoved }))
}
