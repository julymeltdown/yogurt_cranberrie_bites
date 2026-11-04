import assert from 'node:assert/strict'
import { readFile, writeFile, readdir } from 'node:fs/promises'
import { execFileSync } from 'node:child_process'
import { resolve, relative, join } from 'node:path'
import { digest, productionFindings } from './model.mjs'

const root = resolve(import.meta.dirname, '../../../..'), frontend = join(root, 'frontend')
const [mode, output, temporary] = process.argv.slice(2)
const git = (...args) => execFileSync('git', args, { cwd: root, encoding: 'utf8' }).trim()
const json = async path => JSON.parse(await readFile(path, 'utf8'))
let report
if (mode === 'dependencies') {
  const manifest = await json(join(frontend, 'package.json'))
  assert.equal(process.version, 'v' + manifest.engines.node)
  const declared = await readFile(join(frontend, 'pnpm-lock.yaml'))
  const installed = await readFile(join(frontend, 'node_modules/.pnpm/lock.yaml'))
  assert.equal(digest(installed), digest(declared), 'Retained dependency installation differs from candidate lockfile')
  const packages = {}
  for (const name of Object.keys({ ...manifest.dependencies, ...manifest.devDependencies }).sort()) {
    const file = join(frontend, 'node_modules', name, 'package.json'), bytes = await readFile(file)
    packages[name] = { version: JSON.parse(bytes).version, manifest_sha256: digest(bytes) }
  }
  report = { package_manager: manifest.packageManager, node: process.version, lock_sha256: digest(declared), packages, nonempty_count: Object.keys(packages).length }
} else if (mode === 'build') {
  const { build } = await import('vite')
  const modules = new Set(), site = join(temporary, 'site')
  await build({ root: frontend, configLoader: 'runner', cacheDir: join(temporary, 'vite-cache'),
    build: { outDir: site, emptyOutDir: true }, plugins: [{ name: 'qa10-emitted-module-inventory', generateBundle(_options, bundle) {
      for (const chunk of Object.values(bundle)) if (chunk.type === 'chunk') {
        for (const [id, module] of Object.entries(chunk.modules)) if (module.renderedLength > 0 && id.startsWith(frontend + '/') && !id.includes('/node_modules/')) modules.add(relative(root, id).split('?')[0])
      }
    } }] })
  const assets = []
  for (const entry of await readdir(site, { recursive: true, withFileTypes: true })) {
    if (!entry.isFile()) continue
    const path = relative(site, join(entry.parentPath, entry.name)).replaceAll('\\', '/')
    // Public assets are copied unchanged by Vite; scripts/styles/HTML constitute this static check.
    if (!/\.(?:js|css|html)$/.test(path)) continue
    const bytes = await readFile(join(site, path)); assets.push({ path, sha256: digest(bytes), bytes: bytes.length, text: bytes.toString('utf8') })
  }
  assert.ok(modules.size && assets.length && assets.some(a => a.path === 'index.html'), 'Production entry or emitted code is empty')
  report = { entry: 'frontend/index.html', modules: [...modules].sort(), assets: assets.map(({ text: _text, ...a }) => a),
    findings: productionFindings([...modules], assets), nonempty_count: modules.size }
} else if (mode === 'production-static') {
  report = await json(output)
  assert.ok(report.modules.length && report.assets.length, 'No emitted graph to inspect')
  assert.deepEqual(report.findings, [], 'Production fixture/mock or credential marker found; see bounded findings without values')
  console.log(JSON.stringify({ nonempty_count: report.modules.length, findings: [], limitation: 'Known emitted fixture/mock paths and narrow credential markers only; not a complete secret or authorization audit.' }))
  process.exit(0)
} else if (mode === 'task-trace') {
  const candidate = git('rev-parse', 'HEAD'), plan = await json(join(root, 'generated/TASKS_EN.json'))
  const paths = git('ls-tree', '-r', '--name-only', candidate, '--', 'frontend/src', 'frontend/e2e/release/qa10').split('\n')
  const tasks = []
  for (const task of plan.tasks.filter(t => /^FE\d\d$/.test(t.id) || t.id === 'QA10')) {
    const owners = task.owned_paths.map(p => p.replace(/^FS\//, 'frontend/src/')).filter(p => p.startsWith('frontend/'))
    const supplemental = task.id === 'QA10' ? ['frontend/e2e/release/qa10/'] : []
    const files = paths.filter(p => [...owners, ...supplemental].some(prefix => prefix.endsWith('/') ? p.startsWith(prefix) : p === prefix))
    const source = files.filter(p => !p.includes('/__tests__/') && /\.[cm]?[jt]sx?$/.test(p))
    const commit = source.length ? git('log', '-1', '--format=%H', candidate, '--', ...source) : null
    if (commit) execFileSync('git', ['merge-base', '--is-ancestor', commit, candidate], { cwd: root })
    tasks.push({ task: task.id, canonical_owned_paths: owners, supplemental_assignment: supplemental, source_paths: source,
      latest_source_commit: commit, ancestry_checked: !!commit, state: commit ? 'SOURCE_MAPPED' : 'NOT_MAPPED', acceptance: 'NOT_ASSESSED' })
  }
  report = { candidate, canonical_plan: 'generated/TASKS_EN.json', tasks, unmapped: tasks.filter(t => !t.latest_source_commit).map(t => t.task),
    nonempty_count: tasks.filter(t => t.latest_source_commit).length, limitation: 'Source-path and ancestor linkage only. Canonical ownership paths may differ from implementation; unmapped rows are gaps, and no task completion is inferred.' }
  assert.ok(report.nonempty_count, 'No task/source ancestry was found')
} else throw new Error('Unknown worker mode')
await writeFile(output, JSON.stringify(report, null, 2) + '\n')
console.log(JSON.stringify({ mode, nonempty_count: report.nonempty_count, report: relative(root, output) }))
