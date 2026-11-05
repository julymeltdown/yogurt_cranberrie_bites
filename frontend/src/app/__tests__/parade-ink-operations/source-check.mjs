// Developer-owned whole frontend typecheck without installing or linking dependencies.
// The external finite lease runner must establish and continuously enforce both guards.
import assert from 'node:assert/strict'
import { createRequire } from 'node:module'
import { resolve } from 'node:path'

assert.equal(process.env.PI12_SOURCE_ACCEPTED, '1', 'A fresh finite source-validation ACCEPT is required')
assert(process.env.PI12_DEPS_ROOT, 'Retained dependency root is required')
const require = createRequire(resolve(process.env.PI12_DEPS_ROOT, '../package.json'))
const ts = require('typescript'), root = resolve(import.meta.dirname, '../../../..')
const config = ts.readConfigFile(resolve(root, 'tsconfig.json'), ts.sys.readFile)
if (config.error) throw Error(ts.flattenDiagnosticMessageText(config.error.messageText, '\n'))
const parsed = ts.parseJsonConfigFileContent(config.config, ts.sys, root)
const options = { ...parsed.options, noEmit: true, incremental: false, tsBuildInfoFile: undefined,
  typeRoots: [resolve(process.env.PI12_DEPS_ROOT, '@types'), process.env.PI12_DEPS_ROOT] }
const resolutionCache = ts.createModuleResolutionCache(root, file => file, options)
const host = ts.createCompilerHost(options), fallback = resolve(process.env.PI12_DEPS_ROOT, '../src/__pi12_resolution__.ts')
host.resolveModuleNames = (names, file) => names.map(name => {
  const current = ts.resolveModuleName(name, file, options, ts.sys, resolutionCache).resolvedModule
  return current ?? (!name.startsWith('.') && !name.startsWith('/')
    ? ts.resolveModuleName(name, fallback, options, ts.sys, resolutionCache).resolvedModule : undefined)
})
const program = ts.createProgram(parsed.fileNames, options, host)
const diagnostics = [...parsed.errors, ...ts.getPreEmitDiagnostics(program)]
if (diagnostics.length) {
  process.stderr.write(ts.formatDiagnosticsWithColorAndContext(diagnostics, {
    getCanonicalFileName: file => file, getCurrentDirectory: () => root, getNewLine: () => '\n',
  }))
  process.exitCode = 1
} else process.stdout.write(`PASS: whole frontend typecheck (${parsed.fileNames.length} source files; no emit, no dependency link)\n`)
