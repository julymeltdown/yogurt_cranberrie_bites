// Focused new consumer roots, complete transitive imports; no emit or tracked dependency link.
// Adapted from retained motion-adoption/typecheck.mjs at bd087fde.
import { createRequire } from 'node:module'
import { resolve } from 'node:path'
import { readdirSync } from 'node:fs'
const dependencies = process.env.MCR_FRONTEND_DEPENDENCIES
if (!dependencies) throw Error('Retained dependencies required')
const require = createRequire(resolve(dependencies, 'package.json')), ts = require('typescript')
const frontend = resolve(import.meta.dirname, '../../../../..')
const config = ts.readConfigFile(resolve(frontend, 'tsconfig.json'), ts.sys.readFile)
const parsed = ts.parseJsonConfigFileContent(config.config, ts.sys, frontend)
const leaf = resolve(import.meta.dirname, '..')
const files = readdirSync(leaf, { recursive: true }).filter(name => /\.(ts|tsx)$/.test(name)).map(name => resolve(leaf, name))
const program = ts.createProgram([...files, resolve(dependencies, 'node_modules/vite/client.d.ts')], { ...parsed.options, incremental: false, composite: false, tsBuildInfoFile: undefined,
  paths: { react: [resolve(dependencies, 'node_modules/@types/react/index.d.ts')], 'react/*': [resolve(dependencies, 'node_modules/@types/react/*')],
    'react-dom': [resolve(dependencies, 'node_modules/@types/react-dom/index.d.ts')], 'react-dom/*': [resolve(dependencies, 'node_modules/@types/react-dom/*')],
    '*': [resolve(dependencies, 'node_modules/*')] }, typeRoots: [resolve(dependencies, 'node_modules/@types')], types: ['node', 'react'] })
const diagnostics = ts.getPreEmitDiagnostics(program)
for (const diagnostic of diagnostics.slice(0, 25)) process.stderr.write(ts.formatDiagnosticsWithColorAndContext([diagnostic], {
  getCurrentDirectory: () => frontend, getCanonicalFileName: name => name, getNewLine: () => '\n' }))
console.log(JSON.stringify({ roots: files.length, transitiveFiles: program.getSourceFiles().length, diagnostics: diagnostics.length }))
process.exitCode = diagnostics.length ? 1 : 0
