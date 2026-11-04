// Type-check this new fixture against the same read-only Root Git import graph.
import assert from 'node:assert/strict'
import { resolve, relative } from 'node:path'
import { writeFile } from 'node:fs/promises'
import ts from 'typescript'
import { frozenSource } from './frozen-source.mjs'

const repo = resolve(import.meta.dirname, '../../../../..'), frontend = resolve(repo, 'frontend')
const source = frozenSource(repo, process.env.MCR_PROFILE_DEFAULT_SOURCE)
const output = process.env.MCR_PROFILE_DEFAULT_TYPES_OUTPUT
assert(output?.startsWith(repo + '/docs/mcr/evidence/profile-activity-media/default-flow/'))
const own = 'frontend/src/app/__tests__/profile-default-flow/'
const key = path => relative(repo, path).replaceAll('\\', '/')
const frozen = path => { const name = key(path); return !name.startsWith(own) && source.files.has(name) }
const read = path => frozen(path) ? source.bytes(key(path)).toString() : ts.sys.readFile(path)
const exists = path => frozen(path) || ts.sys.fileExists(path)
const directories = new Map()
for (const path of source.files) {
  let parent = path.slice(0, path.lastIndexOf('/'))
  while (parent.includes('/')) { directories.set(resolve(repo, parent), true); parent = parent.slice(0, parent.lastIndexOf('/')) }
}
const directoryExists = path => directories.has(resolve(path)) || ts.sys.directoryExists(path)
const configPath = resolve(import.meta.dirname, 'tsconfig.json'), config = ts.readConfigFile(configPath, read)
assert(!config.error, 'Fixture type configuration must parse')
const parsed = ts.parseJsonConfigFileContent(config.config, { ...ts.sys, readFile: read, fileExists: exists, directoryExists }, import.meta.dirname)
const options = { ...parsed.options, noEmit: true, incremental: false, allowImportingTsExtensions: true }
const host = ts.createCompilerHost(options)
host.readFile = read; host.fileExists = exists; host.directoryExists = directoryExists
host.getCurrentDirectory = () => frontend
host.realpath = path => frozen(path) ? resolve(path) : ts.sys.realpath(path)
host.getSourceFile = (path, version, onError) => {
  const value = read(path)
  if (value === undefined) { onError?.('Unable to read ' + path); return undefined }
  return ts.createSourceFile(path, value, version, true)
}
host.writeFile = () => { throw Error('No type output or peer file writes are allowed') }
const program = ts.createProgram(parsed.fileNames, options, host)
const diagnostics = [...parsed.errors, ...ts.getPreEmitDiagnostics(program)]
if (diagnostics.length) process.stderr.write(ts.formatDiagnosticsWithColorAndContext(diagnostics, {
  getCurrentDirectory: () => frontend, getCanonicalFileName: path => path, getNewLine: () => '\n',
}))
await writeFile(output, JSON.stringify({ candidate: source.revision, typescript: ts.version,
  scope: 'New Profile default fixture and its exact frozen Root production import graph; not a replay of old test suites',
  diagnostics: diagnostics.map(item => ({ code: item.code, file: item.file ? key(item.file.fileName) : null,
    message: ts.flattenDiagnosticMessageText(item.messageText, '\n') })), sourceReceipts: source.receipts(),
  status: diagnostics.length ? 'FAIL' : 'PASS' }, null, 2) + '\n')
process.exitCode = diagnostics.length ? 1 : 0
