import { resolve } from 'node:path'
import { readdir } from 'node:fs/promises'
import { here, root, requireDependency } from './runtime.mjs'
const ts = requireDependency('typescript')
const existing = ts.readConfigFile(resolve(root, 'frontend/tsconfig.json'), ts.sys.readFile)
if (existing.error) throw new Error('Existing frontend type configuration unavailable')
const files = (await readdir(here)).filter(name => name.endsWith('.ts')).map(name => resolve(here, name))
files.push(resolve(here, 'node_modules/vite/client.d.ts'))
// Reuse strict production options in memory; no central config or build cache is changed.
const parsed = ts.parseJsonConfigFileContent({ compilerOptions: { ...existing.config.compilerOptions,
  incremental: false, noEmit: true, types: [],
  paths: {
    react: [resolve(here, 'node_modules/@types/react')], 'react/*': [resolve(here, 'node_modules/@types/react/*')],
    'react-dom': [resolve(here, 'node_modules/@types/react-dom')], 'react-dom/*': [resolve(here, 'node_modules/@types/react-dom/*')],
    '*': [resolve(here, 'node_modules/*')],
  } }, files }, ts.sys, here)
const program = ts.createProgram(files, parsed.options)
const diagnostics = [...parsed.errors, ...ts.getPreEmitDiagnostics(program)]
if (diagnostics.length) {
  console.error(ts.formatDiagnosticsWithColorAndContext(diagnostics, { getCanonicalFileName: name => name, getCurrentDirectory: () => root, getNewLine: () => '\n' }))
  process.exitCode = 1
} else console.log('QA04 strict harness/transitive public-interface types: PASS')
