import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { dirname, resolve } from 'node:path'

const frontend = resolve(import.meta.dirname, '../../../../..'), repo = dirname(frontend)
const require = createRequire(resolve(frontend, 'package.json')), ts = require('typescript')
const proof = JSON.parse(readFileSync(resolve(repo, 'docs/mcr/evidence/pi07-avatar-parade-ink/part-producer-inputs.json'), 'utf8'))
const frozenReadset = JSON.parse(readFileSync(resolve(repo, 'docs/mcr/evidence/pi07-avatar-parade-ink/source12-readset.json'), 'utf8'))
const sourceRows = new Map(frozenReadset.closure.map(row => [resolve(repo, row.path), row]))
const virtual = new Map(), directories = new Set()
for (const row of proof.immutableInputs) {
  const bytes = execFileSync('git', ['show', `${row.revision}:${row.path}`], { cwd: repo })
  assert.equal(bytes.length, row.bytes)
  assert.equal(createHash('sha256').update(bytes).digest('hex'), row.sha256)
  const file = resolve(repo, row.path)
  virtual.set(file, bytes.toString())
  for (let directory = dirname(file); directory.startsWith(frontend); directory = dirname(directory)) directories.add(directory)
}
const config = ts.readConfigFile(resolve(frontend, 'tsconfig.json'), ts.sys.readFile)
if (config.error) throw Error(ts.flattenDiagnosticMessageText(config.error.messageText, '\n'))
const parsed = ts.parseJsonConfigFileContent(config.config, ts.sys, frontend)
const options = { ...parsed.options, noEmit: true, incremental: false, composite: false, tsBuildInfoFile: undefined }
const host = ts.createCompilerHost(options)
const originalRead = host.readFile.bind(host), originalExists = host.fileExists.bind(host)
const originalSource = host.getSourceFile.bind(host), originalDirectory = host.directoryExists?.bind(host)
host.readFile = file => virtual.get(resolve(file)) ?? originalRead(file)
host.fileExists = file => virtual.has(resolve(file)) || originalExists(file)
host.directoryExists = directory => directories.has(resolve(directory)) || !!originalDirectory?.(directory)
host.getSourceFile = (file, language, onError, fresh) => {
  const path = resolve(file)
  if (virtual.has(path)) return ts.createSourceFile(file, virtual.get(path), language, true)
  if (path.startsWith(repo + '/') && !path.includes('/node_modules/')) {
    const row = sourceRows.get(path)
    assert(row, `Unfrozen imported source: ${file}`)
    const bytes = readFileSync(path)
    assert.equal(bytes.length, row.bytes)
    assert.equal(createHash('sha256').update(bytes).digest('hex'), row.sha256, file)
  }
  return originalSource(file, language, onError, fresh)
}
const roots = ['src/features/avatar/motion-adoption/MotionEditor.tsx',
  'src/features/avatar/__tests__/parade-ink/motion-presentation.test.tsx'].map(file => resolve(frontend, file))
const program = ts.createProgram(roots, options, host)
const diagnostics = [...parsed.errors, ...ts.getPreEmitDiagnostics(program)]
for (const diagnostic of diagnostics) process.stderr.write(ts.formatDiagnosticsWithColorAndContext([diagnostic], {
  getCurrentDirectory: () => frontend, getCanonicalFileName: file => file, getNewLine: () => '\n',
}))
console.log(JSON.stringify({ qualification: 'Consumer and imported dependency types with exact producer Git postimages',
  producer: proof.producerRevision, virtualFiles: virtual.size, rootFiles: roots.length, diagnostics: diagnostics.length }))
process.exitCode = diagnostics.length ? 1 : 0
