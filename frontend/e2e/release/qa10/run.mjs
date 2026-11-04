import assert from 'node:assert/strict'
import { readFile, writeFile, mkdir, mkdtemp, rm, lstat, readlink, statfs } from 'node:fs/promises'
import { execFileSync, spawn } from 'node:child_process'
import { resolve, relative, join, dirname, isAbsolute } from 'node:path'
import { tmpdir } from 'node:os'
import { pathToFileURL } from 'node:url'
import { gzipSync, gunzipSync } from 'node:zlib'
import { SCHEMA, assertCandidate, digest, inventory, nodeCounts, safePath, validateReceipt, vitestCounts } from './model.mjs'

const root = resolve(import.meta.dirname, '../../../..'), frontend = join(root, 'frontend')
const git = (...args) => execFileSync('git', args, { cwd: root, encoding: 'utf8' }).trim()
const readJSON = async path => JSON.parse(await readFile(path, 'utf8'))
const worker = 'e2e/release/qa10/worker.mjs', node = process.execPath
const option = name => { const i = process.argv.indexOf(name); return i < 0 ? null : process.argv[i + 1] }

async function evidenceBytes(path) {
  try { return await readFile(path) } catch (error) {
    if (error.code !== 'ENOENT' || !path.endsWith('.json')) throw error
    return gunzipSync(await readFile(path + '.gz'))
  }
}

async function candidateState(expected) {
  const state = { commit: git('rev-parse', 'HEAD'), tree: git('rev-parse', 'HEAD^{tree}'), dirty: false }
  assertCandidate(state, expected)
  const changed = git('diff', '--name-only', expected, '--')
  assert.equal(changed, '', 'Tracked candidate files changed')
  const untracked = git('ls-files', '--others', '--exclude-standard').split('\n').filter(Boolean)
  for (const path of untracked) {
    if (path === 'frontend/node_modules') {
      assert.ok((await lstat(join(root, path))).isSymbolicLink())
      assert.equal(await readlink(join(root, path)), '/Users/lhs/Desktop/mcr/frontend/node_modules')
    } else assert.ok(path.startsWith('docs/mcr/evidence/release/qa10/'), `Untracked source is not the candidate: ${path}`)
  }
  return state
}

async function runCommand(argv, cwd, log, env) {
  const chunks = []
  const result = await new Promise(done => {
    const child = spawn(argv[0], argv.slice(1), { cwd, env, timeout: 300000, stdio: ['ignore', 'pipe', 'pipe'] })
    child.stdout.on('data', b => chunks.push(b)); child.stderr.on('data', b => chunks.push(b))
    child.on('error', error => { chunks.push(Buffer.from(error.message)); done({ code: 2, signal: null }) })
    child.on('close', (code, signal) => done({ code: code ?? 2, signal }))
  })
  const text = Buffer.concat(chunks).toString().replaceAll(root, '<candidate-checkout>')
  await writeFile(log, text)
  return { ...result, text }
}

async function verify(receipt, expected) {
  const report = await readJSON(receipt), folder = dirname(receipt)
  const checks = validateReceipt(report, expected)
  assert.equal(git('rev-parse', `${expected}^{tree}`), report.candidate.tree, 'Receipt tree is not the named Git candidate')
  for (const [path, hash] of Object.entries(report.artifact_pins)) {
    assert.ok(safePath(path)); assert.equal(digest(await evidenceBytes(join(folder, path))), hash, `Artifact changed: ${path}`)
  }
  for (const c of report.commands) assert.equal(digest(await readFile(join(folder, c.log))), c.report_hash, `Command log changed: ${c.id}`)
  for (const [path, pin] of Object.entries({ ...report.source_pins, ...report.contract_pins })) {
    assert.ok(safePath(path)); assert.equal(git('rev-parse', `${expected}:${path}`), pin.git_blob, `Source pin differs from candidate: ${path}`)
    assert.equal(digest(execFileSync('git', ['show', `${expected}:${path}`], { cwd: root })), pin.sha256, `Source digest differs from candidate: ${path}`)
  }
  console.log(JSON.stringify({ schema: SCHEMA, candidate: expected, status: report.status, frontend_checks: checks,
    whole_QA10: 'BLOCKED', note: 'This validates the named historical candidate, not current HEAD or live acceptance.' }))
  return checks === 'PASS' ? 0 : 1
}

async function run(expected, output, python) {
  const candidate = await candidateState(expected)
  assert.ok(isAbsolute(output) && !output.startsWith(frontend + '/'), 'Use a fresh absolute evidence directory outside frontend')
  const disk = await statfs(root)
  assert.ok(disk.bavail * disk.bsize > 8 * 1024 ** 3, 'At least 8 GiB free disk is required')
  await mkdir(output, { recursive: false })
  const temporary = await mkdtemp(join(tmpdir(), 'mcr-qa10-'))
  const env = { ...process.env, PATH: dirname(node) + ':' + process.env.PATH, PYTHONDONTWRITEBYTECODE: '1',
    ...(python ? { MCR_FE14_PYTHON: python } : {}) }
  const records = [], artifacts = new Set(), began = new Date().toISOString()
  const report = { schema: SCHEMA, candidate, status: 'FAIL', scope: 'Original QA10 bounded frontend candidate contribution; no whole-QA10 or IN05 acceptance.',
    started_at: began, tools: { node: process.version, node_executable: node, python: python ?? null }, source_pins: {}, contract_pins: {}, commands: records,
    test_inventory: {}, unsupported: [], artifact_pins: {}, same_candidate_end: false }
  async function step(id, argv, count, inspect, cwd = frontend) {
    console.log(`QA10 ${id}: running`)
    const log = `${id}.log`, result = await runCommand(argv, cwd, join(output, log), env)
    let details = {}, status = result.code === 0 ? 'PASS' : result.code === 2 ? 'BLOCKED' : 'FAIL'
    try { if (inspect) details = await inspect(result) } catch (error) { details = { validation_error: error.message }; status = 'FAIL' }
    const nonempty = details.nonempty_count ?? count
    if (status === 'PASS' && !(nonempty > 0)) { status = 'FAIL'; details.validation_error = 'Nonempty coverage is required' }
    const record = { id, argv, cwd: relative(root, cwd) || '.', attempted: true, exit_code: result.code, signal: result.signal, log,
      report_hash: digest(await readFile(join(output, log))), nonempty_count: nonempty, status, details }
    records.push(record); artifacts.add(log)
    console.log(`QA10 ${id}: ${status} (exit ${result.code})`)
    return record
  }
  try {
    const rows = execFileSync('git', ['ls-tree', '-r', '-z', expected, '--', 'frontend', 'contracts', 'scripts/mcr', 'generated/TASKS_EN.json', 'docs/mcr/reuse-allowlist.json', 'docs/mcr/frontend/reuse-inventory.json', '.node-version'], { cwd: root, encoding: 'utf8' }).split('\0').filter(Boolean)
    const entries = []
    for (const row of rows) {
      const [metadata, path] = row.split('\t'), blob = metadata.split(' ')[2]
      if (!/\.(?:[cm]?[jt]sx?|css|html|json|yaml|py)$/.test(path) && !['.node-version', 'scripts/mcr/requirements-contracts.txt'].includes(path)) continue
      const bytes = await readFile(join(root, path))
      const target = path.startsWith('contracts/') ? report.contract_pins : report.source_pins
      target[path] = { sha256: digest(bytes), git_blob: blob }
      if (path.startsWith('frontend/')) entries.push({ path, source: bytes.toString('utf8') })
    }
    report.test_inventory = inventory(entries)
    await writeFile(join(output, 'test-inventory.json'), JSON.stringify(report.test_inventory, null, 2) + '\n'); artifacts.add('test-inventory.json')
    report.unsupported = [
      ...report.test_inventory.playwright.map(path => ({ criterion: path, status: 'NOT_RUN', owner: 'Existing IN01/IN02/IN04 real-service owners', reason: 'Requires provisioned real service/accounts; not replaced by fixture tests' })),
      ...report.test_inventory.unsupported.map(path => ({ criterion: path, status: 'NOT_RUN', owner: 'Existing harness owner Root153/1fc', reason: 'Unrecognized test runner' })),
      { criterion: 'BE43 release packaging, backend generation/contract/race/recovery qualification and canonical release consumer', status: 'NOT_RUN', owner: 'Backend/root 01a1a1fc-7f3d-7ff1-9416-e3435c8a6f08' },
      { criterion: 'Full task/IA acceptance, physical devices, full secret audit and deployment environment', status: 'NOT_RUN', owner: 'Existing implementation owners and release integrator; IN05 remains open' },
    ]
    const deps = join(output, 'dependencies.json'); artifacts.add('dependencies.json')
    await step('dependencies', [node, worker, 'dependencies', deps], 0, async () => ({ ...await readJSON(deps) }))
    const preflight = "import runpy,importlib.metadata,json;m=runpy.run_path('../scripts/mcr/validate-assets.py');m['validators']();print(json.dumps({x:importlib.metadata.version(x) for x in ['jsonschema','referencing','rfc3339-validator','rfc3986-validator','six']}))"
    await step('python-formats', [python ?? 'python-required', '-B', '-c', preflight], python ? 5 : 0)
    await step('typecheck', [node, 'node_modules/typescript/bin/tsc', '--noEmit', '--incremental', 'false', '--pretty', 'false'], entries.filter(e => /\.tsx?$/.test(e.path)).length)
    await step('lint', [node, 'node_modules/oxlint/bin/oxlint', 'src', 'vite.config.ts', 'e2e/release/qa10'], entries.filter(e => e.path.startsWith('frontend/src/')).length)
    const vitestFile = join(output, 'vitest.json'); artifacts.add('vitest.json')
    await step('vitest', [node, 'node_modules/vitest/vitest.mjs', 'run', ...report.test_inventory.vitest.map(p => p.slice('frontend/'.length)), '--configLoader', 'runner', '--no-cache', '--maxWorkers=2', '--reporter=json', `--outputFile=${vitestFile}`], 0, async () => {
      const counts = vitestCounts(await readJSON(vitestFile), report.test_inventory.vitest, root)
      return { ...counts, nonempty_count: counts.total }
    })
    await step('node-tests', [node, '--test', '--test-reporter=tap', ...report.test_inventory.node.map(p => p.slice('frontend/'.length))], 0, result => {
      const counts = nodeCounts(result.text); return { ...counts, nonempty_count: counts.total }
    })
    const audit = join(output, 'build-audit.json'); artifacts.add('build-audit.json')
    await step('build', [node, worker, 'build', audit, temporary], 0, async () => ({ nonempty_count: (await readJSON(audit)).nonempty_count }))
    await step('ui-origin', [node, '../scripts/mcr/check-ui-origin.mjs', '--root', root], 0, result => {
      const r = JSON.parse(result.text); assert.equal(r.result, 'PASS'); return { nonempty_count: r.content_files, report: r }
    })
    await step('production-static', [node, worker, 'production-static', audit], 0, result => JSON.parse(result.text))
    const trace = join(output, 'task-trace.json'); artifacts.add('task-trace.json')
    await step('task-trace', [node, worker, 'task-trace', trace], 0, async () => {
      const r = await readJSON(trace)
      if (r.unmapped.length) report.unsupported.push({ criterion: 'Canonical frontend task/source mappings: ' + r.unmapped.join(', '), status: 'NOT_RUN', owner: 'Respective implementation owners / root release composition', reason: 'Source ownership paths differ or have no implementation; no completion inferred' })
      return { nonempty_count: r.nonempty_count, unmapped: r.unmapped }
    })
    assertCandidate(await candidateState(expected), expected, candidate.tree); report.same_candidate_end = true
    report.status = records.every(c => c.status === 'PASS') && !report.test_inventory.unsupported.length ? 'PARTIAL' : 'FAIL'
  } catch (error) { report.error = error.message; report.status = 'FAIL' }
  finally {
    await rm(temporary, { recursive: true, force: true })
    report.finished_at = new Date().toISOString()
    for (const file of artifacts) {
      try {
        const bytes = await readFile(join(output, file))
        report.artifact_pins[file] = digest(bytes)
        if (file === 'vitest.json') {
          await writeFile(join(output, file + '.gz'), gzipSync(bytes))
          await rm(join(output, file))
        }
      } catch { report.unsupported.push({ criterion: file, status: 'NOT_RUN', reason: 'Expected evidence file was not produced' }) }
    }
    await writeFile(join(output, 'receipt.json'), JSON.stringify(report, null, 2) + '\n')
  }
  console.log(JSON.stringify({ candidate: expected, status: report.status, frontend_checks: records.every(c => c.status === 'PASS') ? 'PASS' : 'FAIL', receipt: join(output, 'receipt.json') }))
  return report.status === 'PARTIAL' ? 0 : 1
}

export async function main() {
  const mode = process.argv[2], candidate = option('--candidate')
  const flags = process.argv.slice(3)
  assert.equal(flags.length % 2, 0, 'Every option requires one value')
  const permitted = mode === 'verify' ? ['--candidate', '--receipt'] : ['--candidate', '--output', '--python']
  const seen = new Set()
  for (let i = 0; i < flags.length; i += 2) {
    assert.ok(permitted.includes(flags[i]) && !seen.has(flags[i]) && flags[i + 1] && !flags[i + 1].startsWith('--'), 'Unknown, duplicated or missing option')
    seen.add(flags[i])
  }
  if (mode === 'verify') return verify(resolve(option('--receipt') ?? ''), candidate)
  assert.equal(mode, 'run', 'Usage: run.mjs run --candidate FULL_SHA --output FRESH_ABSOLUTE_DIR --python EXISTING_PYTHON | verify --candidate FULL_SHA --receipt FILE')
  assert.ok(candidate && option('--output'), 'Explicit candidate and output required')
  return run(candidate, option('--output'), option('--python'))
}
if (process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url) {
  try { process.exitCode = await main() } catch (error) { console.error(error.message); process.exitCode = 2 }
}
