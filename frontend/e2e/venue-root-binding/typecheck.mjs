import { createRequire } from 'node:module'
import { resolve, dirname } from 'node:path'
import { writeFileSync } from 'node:fs'
import { overlay, pins, frontend } from './readset.mjs'
const deps = process.env.MCR_FRONTEND_DEPENDENCIES
if (!deps || !process.env.MCR_FE38_SCRATCH) throw Error('Fresh accepted source allocation required')
const require = createRequire(resolve(deps, 'package.json')), ts = require('typescript')
const original = ts.readConfigFile(resolve(frontend, 'tsconfig.json'), ts.sys.readFile)
const options = { ...original.config.compilerOptions, incremental: false, composite: false,
  paths: { react: [resolve(deps, 'node_modules/@types/react/index.d.ts')], 'react/*': [resolve(deps, 'node_modules/@types/react/*')],
    'react-dom': [resolve(deps, 'node_modules/@types/react-dom/index.d.ts')], 'react-dom/*': [resolve(deps, 'node_modules/@types/react-dom/*')],
    '*': [resolve(deps, 'node_modules/*')] }, typeRoots: [resolve(deps, 'node_modules/@types')], types: ['node', 'react'] }
const parsed = ts.parseJsonConfigFileContent({ compilerOptions: options,
  include: ['src/app/venue-root/**/*.ts', 'src/app/venue-root/**/*.tsx', 'e2e/venue-root-binding/**/*.ts', 'e2e/venue-root-binding/**/*.tsx'] }, ts.sys, frontend)
const host = ts.createCompilerHost(parsed.options), oldRead = host.readFile.bind(host), oldExists = host.fileExists.bind(host)
const oldDirectory = host.directoryExists.bind(host), directories = new Set()
for (const path of overlay.keys()) { let current = dirname(path); while (current !== dirname(current)) { directories.add(current); current = dirname(current) } }
host.fileExists = path => overlay.has(path) || oldExists(path)
host.readFile = path => overlay.get(path) ?? oldRead(path)
host.directoryExists = path => directories.has(path) || oldDirectory(path)
host.getSourceFile = (path, languageVersion) => { const text = host.readFile(path); return text === undefined ? undefined : ts.createSourceFile(path, text, languageVersion) }
const roots = [...parsed.fileNames, resolve(deps, 'node_modules/vite/client.d.ts')]
const program = ts.createProgram(roots, parsed.options, host), diagnostics = ts.getPreEmitDiagnostics(program)
for (const item of diagnostics.slice(0, 35)) process.stderr.write(ts.formatDiagnosticsWithColorAndContext([item], {
  getCurrentDirectory: () => frontend, getCanonicalFileName: name => name, getNewLine: () => '\n' }))
writeFileSync(resolve(process.env.MCR_FE38_SCRATCH, 'venue-root-readset.json'), JSON.stringify(pins, null, 2) + '\n')
if (process.env.MCR_FE38_OUTPUT) writeFileSync(resolve(process.env.MCR_FE38_OUTPUT, '../type-readset.json'), JSON.stringify(pins, null, 2) + '\n')
console.log(JSON.stringify({ roots: roots.length, transitiveFiles: program.getSourceFiles().length, immutableInputs: pins.length, diagnostics: diagnostics.length }))
process.exitCode = diagnostics.length ? 1 : 0
