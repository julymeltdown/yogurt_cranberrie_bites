import assert from 'node:assert/strict'
import { spawn, execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { existsSync, lstatSync, mkdirSync, openSync, closeSync, readdirSync, readFileSync, realpathSync, readlinkSync, rmSync, statfsSync, symlinkSync, unlinkSync, writeFileSync } from 'node:fs'
import { dirname, isAbsolute, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../../../../..'), frontend = join(root, 'frontend')
const need = key => { assert(process.env[key], `Missing ${key}`); return process.env[key] }
const output = need('MCR_FEED_CARRY'), pinned = need('MCR_FEED_NODE'), dependencies = need('MCR_FEED_DEPENDENCIES')
const allocation = need('MCR_FEED_ALLOCATION'), run = need('MCR_FEED_RUN')
const cutoff = Date.parse(need('MCR_FEED_ACTIVE_UNTIL')), native = process.env.MCR_FEED_NATIVE === '1'
const maxFiles = Number(need('MCR_FEED_MAX_FILES')), maxBytes = Number(need('MCR_FEED_MAX_BYTES'))
const maxProcesses = Number(need('MCR_FEED_MAX_PROCESSES'))
const maxRssBytes = Number(need('MCR_FEED_MAX_RSS_BYTES'))
assert([output, pinned, dependencies].every(isAbsolute)); assert(/^[a-z0-9-]{1,50}$/.test(run))
assert(Number.isSafeInteger(maxFiles) && maxFiles > 0 && Number.isSafeInteger(maxBytes) && maxBytes > 0)
assert(Number.isSafeInteger(maxProcesses) && maxProcesses >= 2)
assert(Number.isSafeInteger(maxRssBytes) && maxRssBytes > 0)
assert(existsSync(output) && !output.startsWith(root + '/'))
assert(cutoff > Date.now() + 45_000 && cutoff < Date.now() + 900_000)
const link = join(frontend, 'node_modules'), cache = join(output, `feed-${run}-cache`), tmp = join(output, `feed-${run}-tmp`)
const present = path => { try { lstatSync(path); return true } catch (error) { if (error.code === 'ENOENT') return false; throw error } }
const hash = path => createHash('sha256').update(readFileSync(path)).digest('hex')
assert.equal(process.version, 'v24.22.2'); assert.equal(realpathSync(process.execPath), realpathSync(pinned))
assert.equal(hash(join(frontend, 'pnpm-lock.yaml')), hash(join(dirname(dependencies), 'pnpm-lock.yaml')))
assert(!present(link) && !present(cache) && !present(tmp))
const env = { ...process.env, PATH: dirname(pinned) + ':' + process.env.PATH, TMPDIR: tmp, MCR_FEED_CACHE: cache,
  MCR_FEED_OUTPUT: join(output, `feed-${run}-native`), NODE_DISABLE_COMPILE_CACHE: '1', NO_COLOR: '1' }
delete env.NODE_COMPILE_CACHE
assert.equal(execFileSync('node', ['--version'], { env, encoding: 'utf8' }).trim(), 'v24.22.2')
const sources = ['frontend/src/app/feed-entry', 'docs/mcr/frontend/feed-root-composition.md', 'docs/mcr/evidence/feed-root-composition',
  'frontend/src/app/in03-entry', 'docs/mcr/frontend/in03-production-availability.md', 'docs/mcr/evidence/in03-production-availability',
  'frontend/src/features/feed-thread/FeedThread.tsx', 'frontend/src/features/event-feed/__tests__/thread-author',
  'frontend/e2e/real/social', 'frontend/e2e/real/social.spec.ts', 'frontend/src/shared/api/__tests__/c07-social-client',
  'docs/mcr/frontend/c07-social-client-in03.md', 'docs/mcr/frontend/in03-public-journey.md', 'docs/mcr/evidence/c07-social-client-in03', 'docs/mcr/evidence/in03-public-journey'].map(path => join(root, path))
const gitDir = execFileSync('git', ['rev-parse', '--absolute-git-dir'], { cwd: root, encoding: 'utf8' }).trim()
const gitIds = [...new Set(execFileSync('git', ['rev-list', '--objects', '654974128a27c948a449fa9e58f466b8e612361d..HEAD',
  'codex/mcr-qa-IN03-stack-runtime', 'codex/mcr-fe-IN03-public-journey'], { cwd: root, encoding: 'utf8' }).trim().split('\n').filter(Boolean).map(line => line.split(' ')[0]))]
const gitObjects = gitIds.length ? execFileSync('git', ['cat-file', '--batch-check=%(objectsize)'], { cwd: root, encoding: 'utf8', input: gitIds.join('\n') + '\n' }).trim().split('\n').map(Number) : []
assert(gitObjects.every(Number.isSafeInteger))
function usage() {
  let files = gitObjects.length, bytes = gitObjects.reduce((sum, size) => sum + size, 0), temporaryBytes = 0
  const visit = path => {
    let info
    try { info = lstatSync(path) } catch (error) { if (error.code === 'ENOENT') return; throw error }
    if (info.isDirectory()) {
      let names
      try { names = readdirSync(path) } catch (error) { if (error.code === 'ENOENT') return; throw error }
      for (const name of names) visit(join(path, name))
    }
    else { files++; bytes += info.size; if (path.startsWith(cache + '/') || path.startsWith(tmp + '/')) temporaryBytes += info.size }
  }
  for (const path of [output, ...sources, link, join(root, '.git'), gitDir]) visit(path)
  const disk = statfsSync(output)
  return { files, bytes, temporaryBytes, freeBytes: disk.bavail * disk.bsize }
}
const initial = usage()
assert(initial.freeBytes >= 16 * 1024 ** 3 && initial.files < maxFiles && initial.bytes < maxBytes)
const prefix = 'src/app/feed-entry/__tests__/'
const jobs = [
  ['tsc', ['node_modules/typescript/bin/tsc', '-p', prefix + 'tsconfig.json', '--noEmit', '--incremental', 'false', '--pretty', 'false']],
  ['unit', ['node_modules/vitest/vitest.mjs', 'run', '--configLoader', 'native', '--config', prefix + 'vitest.config.ts']],
  ['lint', ['node_modules/oxlint/bin/oxlint', 'src/app/feed-entry']],
  ['native-syntax', ['--check', prefix + 'native.mjs']],
]
if (native) jobs.splice(0, jobs.length,
  ...(process.env.MCR_FEED_REMAINING === '1' || process.env.MCR_FEED_SKIP_PASSED_TYPES === '1' ? [] : [['changed-fixture-tsc', ['node_modules/typescript/bin/tsc', '-p', prefix + 'tsconfig.json', '--noEmit', '--incremental', 'false', '--pretty', 'false']]]),
  ['supervisor-syntax', ['--check', prefix + 'qualify.mjs']],
  ['native-syntax', ['--check', prefix + 'native.mjs']])
if (process.env.MCR_FEED_THREAD_AUTHOR_SOURCE === '1') jobs.splice(0, jobs.length,
  ...(process.env.MCR_FEED_THREAD_TYPES_PASSED === '1' ? [] : [['thread-coupled-types', [prefix + 'thread-types.mjs']]]),
  ['thread-author-unit', ['node_modules/vitest/vitest.mjs', 'run', '--configLoader', 'native', '--config', 'src/features/event-feed/__tests__/thread-author/vitest.config.ts']],
  ['thread-author-lint', ['node_modules/oxlint/bin/oxlint', '--deny-warnings', 'src/features/feed-thread/FeedThread.tsx', 'src/features/event-feed/__tests__/thread-author']],
  ['supervisor-syntax', ['--check', prefix + 'qualify.mjs']],
  ['native-syntax', ['--check', prefix + 'native.mjs']])
if (process.env.MCR_FEED_ADMISSION === '1') jobs.push(['admission', ['--test', 'e2e/real/social/admission-original.test.mjs']],
  ['admission-tsc', ['node_modules/typescript/bin/tsc', '-p', 'e2e/real/social/tsconfig.json', '--noEmit', '--incremental', 'false', '--pretty', 'false']])
if (process.env.MCR_IN03_SOURCE === '1') jobs.splice(0, jobs.length,
  ['in03-scoped-types', ['node_modules/typescript/bin/tsc', '-p', 'src/app/in03-entry/__tests__/tsconfig.json', '--noEmit', '--incremental', 'false', '--pretty', 'false']],
  ['in03-availability-unit', ['node_modules/vitest/vitest.mjs', 'run', '--configLoader', 'native', '--config', 'src/app/in03-entry/__tests__/vitest.config.ts']],
  ['in03-source-lint', ['node_modules/oxlint/bin/oxlint', '--deny-warnings', 'src/app/in03-entry', 'e2e/real/social', 'e2e/real/social.spec.ts']],
  ['supervisor-syntax', ['--check', prefix + 'qualify.mjs']])
const summary = { allocation, run, source: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(),
  node: process.version, nodeSha256: hash(pinned), lockSha256: hash(join(frontend, 'pnpm-lock.yaml')), initial, jobs: [], liveIN03: 'NOT_RUN', status: 'IN_PROGRESS' }
const children = new Map()
let setup = false, violated = false
let peakProcesses = 1
let peakRssBytes = 0
function processCount() {
  const rows = execFileSync('/bin/ps', ['-axo', 'pid=,ppid=,rss='], { encoding: 'utf8' }).trim().split('\n').map(line => line.trim().split(/\s+/).map(Number))
  const owned = new Set([process.pid])
  for (let changed = true; changed;) {
    changed = false
    for (const [pid, parent] of rows) if (owned.has(parent) && !owned.has(pid)) { owned.add(pid); changed = true }
  }
  peakProcesses = Math.max(peakProcesses, owned.size)
  const rssBytes = rows.filter(([pid]) => owned.has(pid)).reduce((sum, row) => sum + row[2] * 1024, 0)
  peakRssBytes = Math.max(peakRssBytes, rssBytes)
  return { count: owned.size, rssBytes }
}
function enforce() {
  const state = usage(), processes = processCount()
  const allowed = Date.now() < cutoff && state.freeBytes >= 8 * 1024 ** 3 && state.files <= maxFiles && state.bytes <= maxBytes
    && state.temporaryBytes <= 100 * 1024 ** 2 && state.bytes - state.temporaryBytes - initial.bytes <= 8 * 1024 ** 2
    && processes.count <= maxProcesses && processes.rssBytes <= maxRssBytes
  if (!allowed) { summary.guardViolation = { utc: new Date().toISOString(), state, processes }; console.log(JSON.stringify({ guardViolation: summary.guardViolation })) }
  return allowed
}
function stop(child, signal = 'SIGTERM') { try { process.kill(-child.pid, signal) } catch (error) { if (error.code !== 'ESRCH') throw error } }
async function launch(name, args, background = false) {
  assert(!violated && enforce(), 'Actual resource guard refused child launch')
  const log = join(output, `feed-${run}-${name}.log`), fd = openSync(log, 'wx'), start = Date.now()
  const child = spawn(pinned, args, { cwd: frontend, env, stdio: ['ignore', fd, fd], detached: true })
  children.set(child.pid, child)
  const completion = new Promise((resolveJob, reject) => {
    child.once('error', reject)
    child.once('exit', (exit, signal) => { children.delete(child.pid); closeSync(fd); resolveJob({ name, args, exit, signal, startUTC: new Date(start).toISOString(), milliseconds: Date.now() - start }) })
  })
  if (background) return { child, completion }
  const result = await completion; summary.jobs.push(result); console.log(JSON.stringify(result))
  assert(!violated && result.exit === 0, `Failed ${name}; retain evidence and repair only inside the accepted lease`)
}
const monitor = setInterval(() => {
  let allowed = false
  try { allowed = enforce() } catch (error) { summary.guardError = { utc: new Date().toISOString(), message: error.message }; console.log(JSON.stringify({ guardError: summary.guardError })) }
  if (!allowed) {
    violated = true
    for (const child of children.values()) { stop(child); setTimeout(() => stop(child, 'SIGKILL'), 1500).unref() }
  }
}, 250)
try {
  mkdirSync(cache); mkdirSync(tmp); symlinkSync(dependencies, link); setup = true
  assert.equal(realpathSync(link), realpathSync(dependencies))
  if (native) {
    const port = Number(need('MCR_FEED_PORT'))
    assert(Number.isInteger(port) && port > 1024 && port < 65536)
    env.MCR_FEED_ORIGIN = `http://127.0.0.1:${port}/`
    summary.cssInputs = []
    for (const [name, prefixEnv, path] of [
      ['feed', 'MCR_FEED_CSS', 'frontend/src/features/event-feed/feed.css'],
      ['root', 'MCR_FEED_ROOT_CSS', 'frontend/src/shared/styles/cozy-community-backgrounds-v1.css'],
    ]) {
      const revision = need(prefixEnv + '_REVISION'), sha256 = need(prefixEnv + '_SHA256')
      assert(/^[0-9a-f]{40}$/.test(revision) && /^[0-9a-f]{64}$/.test(sha256))
      const css = execFileSync('git', ['show', `${revision}:${path}`], { cwd: root, maxBuffer: 256 * 1024 })
      assert.equal(createHash('sha256').update(css).digest('hex'), sha256)
      const file = join(output, `feed-${run}-${name}-owner.css`)
      writeFileSync(file, css, { flag: 'wx', mode: 0o444 }); env[prefixEnv + '_FILE'] = file
      summary.cssInputs.push({ revision, sha256, path, file, readonlyBorrow: true })
    }
    summary.moduleInputs = []
    for (const [name, prefixEnv, path] of [
      ['root', 'MCR_FEED_ROOT_MEDIA', 'frontend/src/app/media-entry/RootMediaView.tsx'],
      ['social', 'MCR_FEED_ROOT_SOCIAL', 'frontend/src/app/social-composition/panels.tsx'],
    ]) {
      if (!process.env[prefixEnv + '_REVISION'] && !process.env[prefixEnv + '_SHA256']) continue
      const revision = need(prefixEnv + '_REVISION'), sha256 = need(prefixEnv + '_SHA256')
      assert(/^[0-9a-f]{40}$/.test(revision) && /^[0-9a-f]{64}$/.test(sha256))
      const source = execFileSync('git', ['show', `${revision}:${path}`], { cwd: root, maxBuffer: 256 * 1024 })
      assert.equal(createHash('sha256').update(source).digest('hex'), sha256)
      const file = join(output, `feed-${run}-${name}-owner.tsx`)
      writeFileSync(file, source, { flag: 'wx', mode: 0o444 }); env[prefixEnv + '_FILE'] = file
      summary.moduleInputs.push({ revision, sha256, path, file, readonlyBorrow: true })
    }
    for (const [name, args] of jobs) await launch(name, args)
    const server = await launch('vite', ['node_modules/vite/bin/vite.js', '--configLoader', 'native', '--config', prefix + 'vite.config.ts', '--host', '127.0.0.1', '--port', String(port), '--strictPort'], true)
    try {
      let ready = false
      for (let tries = 0; tries < 100 && !violated && children.has(server.child.pid); tries++) {
        try { ready = (await fetch(env.MCR_FEED_ORIGIN + prefix + 'preview.html')).ok } catch { /* Bounded local startup only. */ }
        if (ready) break
        await new Promise(resolveWait => setTimeout(resolveWait, 100))
      }
      assert(ready, 'Allocated local Vite did not start')
      await launch('native', [prefix + 'native.mjs'])
    } finally { stop(server.child); setTimeout(() => stop(server.child, 'SIGKILL'), 1500).unref(); summary.jobs.push(await server.completion) }
  } else for (const [name, args] of jobs) await launch(name, args)
  summary.status = native ? 'SYNTHETIC_NATIVE_PASSED' : 'SOURCE_CHECKS_PASSED'
} catch (error) { summary.status = 'FAILED'; summary.error = error.message; process.exitCode = 1 }
finally {
  clearInterval(monitor)
  for (const child of children.values()) stop(child, 'SIGKILL')
  if (setup) { assert.equal(readlinkSync(link), dependencies); unlinkSync(link) }
  if (present(cache)) rmSync(cache, { recursive: true })
  if (present(tmp)) rmSync(tmp, { recursive: true })
  summary.final = usage(); summary.releasedUTC = new Date().toISOString(); summary.linkRemoved = !present(link); summary.peakProcesses = peakProcesses; summary.peakRssBytes = peakRssBytes
  writeFileSync(join(output, `feed-${run}-summary.json`), JSON.stringify(summary, null, 2) + '\n', { flag: 'wx' })
  console.log(JSON.stringify({ status: summary.status, final: summary.final, releasedUTC: summary.releasedUTC, linkRemoved: summary.linkRemoved }))
}
