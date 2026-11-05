// Run only under a fresh finite source-qualification offer, never source prep.
import assert from 'node:assert/strict'
import { createRequire } from 'node:module'
import { spawnSync } from 'node:child_process'
import { readFile, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'
import { execFileSync } from 'node:child_process'
import { sparseSource } from './sparse-source.mjs'
assert.equal(process.version, 'v24.22.2')
const frontend = resolve(import.meta.dirname, '../../..')
const tooling = process.env.PI09_TOOLING_ROOT
const out = process.env.PI09_SOURCE_OUTPUT
assert(tooling && (out?.startsWith(resolve(frontend, '../docs/mcr/evidence/parade-ink-wall-b') + '/') || out?.startsWith('/private/tmp/mcr-pi09-source-')))
const tool = createRequire(resolve(tooling, 'package.json'))
const { startVitest } = await import(pathToFileURL(tool.resolve('vitest/node')))
const { default: react } = await import(pathToFileURL(tool.resolve('@vitejs/plugin-react')))
const pkg = JSON.parse(await readFile(resolve(frontend, 'package.json'), 'utf8'))
const aliases = []
for (const name of Object.keys({ ...pkg.dependencies, ...pkg.devDependencies })) {
  if (name.startsWith('@types/')) continue
  assert(/^[a-z0-9@/-]+$/.test(name))
  const root = resolve(tooling, 'node_modules', name)
  const metadata = JSON.parse(await readFile(resolve(root, 'package.json'), 'utf8'))
  const choose = item => typeof item === 'string' ? item : item?.import ? choose(item.import) : item?.default ? choose(item.default) : undefined
  const entry = choose(metadata.exports?.['.'])
  aliases.push({ find: new RegExp('^' + name + '$'), replacement: entry ? resolve(root, entry) : tool.resolve(name) })
  if (name === 'react' || name === 'react-dom') aliases.push({ find: new RegExp('^' + name + '/'), replacement: root + '/' })
}
let ctx
if (process.env.PI09_SOURCE_TYPES_ONLY !== '1') {
try {
  ctx = await startVitest('test', [
    'src/features/mural/editor/__tests__/parade-ink-wall-b/presentation.test.tsx',
    'src/features/mural/editor/__tests__/reference-target/presentation.test.tsx',
    'src/features/mural/editor/__tests__/wall-tools-b/presentation.test.tsx',
    'src/features/mural/editor/__tests__/wall-tools-b/tools.test.ts',
    'src/features/mural/editor/__tests__/input.test.ts',
  ], { root: frontend, config: false, run: true, maxWorkers: 1, fileParallelism: false,
    reporters: ['default', 'json'], outputFile: { json: out } },
  { plugins: [react()], resolve: { alias: aliases }, cacheDir: process.env.PI09_SOURCE_TEMP })
  const result = JSON.parse(await readFile(out, 'utf8'))
  assert(result.success && result.numFailedTests === 0)
} finally { await ctx?.close() }
}

const temp = process.env.PI09_SOURCE_TEMP
assert(temp?.startsWith('/private/tmp/mcr-pi09-'))
const typeConfig = resolve(temp, 'tsconfig.json')
await writeFile(typeConfig, JSON.stringify({ extends: resolve(frontend, 'tsconfig.json'), compilerOptions: { noEmit: true, incremental: false, types: [resolve(tooling, 'node_modules/vite/client.d.ts')], paths: { vite: [resolve(tooling, 'node_modules/vite/dist/node/index.d.ts')], '@vitejs/plugin-react': [resolve(tooling, 'node_modules/@vitejs/plugin-react/dist/index.d.ts')], '@tailwindcss/vite': [resolve(tooling, 'node_modules/@tailwindcss/vite/dist/index.d.mts')], react: [resolve(tooling, 'node_modules/@types/react')], 'react/*': [resolve(tooling, 'node_modules/@types/react/*')], 'react-dom': [resolve(tooling, 'node_modules/@types/react-dom')], 'react-dom/*': [resolve(tooling, 'node_modules/@types/react-dom/*')], '*': [resolve(tooling, 'node_modules/*')] }, typeRoots: [resolve(tooling, 'node_modules/@types')] }, include: [resolve(frontend, 'src'), resolve(frontend, 'vite.config.ts')] }, null, 2))
const checks = {}
for (const [name, executable, args] of [
  ['nativeHelperSyntax', process.execPath, ['--check', resolve(import.meta.dirname, 'app.mjs')]],
  ['sourceHelperSyntax', process.execPath, ['--check', resolve(import.meta.dirname, 'source.mjs')]],
  ['lint', resolve(tooling, 'node_modules/.bin/oxlint'), ['src/features/mural/editor/MuralEditor.tsx', 'src/features/mural/editor/__tests__/parade-ink-wall-b/presentation.test.tsx']],
]) {
  if (process.env.PI09_SOURCE_TYPES_ONLY === '1' && ['lint', 'nativeHelperSyntax'].includes(name)) continue
  const run = spawnSync(executable, args, { cwd: frontend, encoding: 'utf8', timeout: 90000, maxBuffer: 4 * 1024 ** 2 })
  checks[name] = { status: run.status, stdout: run.stdout, stderr: run.stderr, error: run.error?.message }
}
const ts = tool('typescript')
const repo = resolve(frontend, '..'), pin = execFileSync('git', ['rev-parse', 'HEAD'], {cwd:repo}).toString().trim()
const sparseInputs = sparseSource(repo,pin)
const config = ts.readConfigFile(typeConfig, ts.sys.readFile)
const parsed = ts.parseJsonConfigFileContent(config.config, ts.sys, temp)
const host = ts.createCompilerHost(parsed.options)
const originalExists=host.fileExists.bind(host), originalRead=host.readFile.bind(host), originalDirectory=host.directoryExists?.bind(host)
host.fileExists=path=>originalExists(path)||sparseInputs.tracked.has(path)
host.readFile=path=>originalRead(path)??sparseInputs.read(path)
host.directoryExists=path=>!!originalDirectory?.(path)||sparseInputs.directories.has(path)
const roots=[...new Set([...parsed.fileNames,...[...sparseInputs.tracked].filter(path=>path.startsWith(frontend+'/src/')&&/\.[cm]?tsx?$/.test(path))])]
const program=ts.createProgram(roots,parsed.options,host)
const diagnostics=[...(config.error?[config.error]:[]),...parsed.errors,...ts.getPreEmitDiagnostics(program)]
checks.typecheck={status:diagnostics.length?2:0,stdout:ts.formatDiagnosticsWithColorAndContext(diagnostics,{getCanonicalFileName:p=>p,getCurrentDirectory:()=>frontend,getNewLine:()=>'\n'}),stderr:'',rootFiles:roots.length,noEmit:parsed.options.noEmit,incremental:parsed.options.incremental,sparseInputs:sparseInputs.readset}
await writeFile(out.replace(/\.json$/, '-checks.json'), JSON.stringify(checks, null, 2) + '\n')
assert(Object.values(checks).every(check => check.status === 0), 'Typecheck/lint failed; preserve evidence and fix')
