import { createRequire } from 'node:module'
import { fileURLToPath } from 'node:url'
const frontend = fileURLToPath(new URL('../../../../frontend/', import.meta.url))
const require = createRequire(frontend + 'package.json')
const ts = require('typescript')
const config = ts.readConfigFile(frontend + 'tsconfig.json', ts.sys.readFile)
const parsed = ts.parseJsonConfigFileContent(config.config, ts.sys, frontend, undefined, frontend + 'tsconfig.json')
const names = ['social-sharing', 'inbox', 'safety', 'account']
const roots = parsed.fileNames.filter(path => names.some(name => path.includes('/features/' + name + '/')))
const program = ts.createProgram(roots, { ...parsed.options, incremental: false, noEmit: true })
const diagnostics = [...parsed.errors, ...ts.getPreEmitDiagnostics(program)]
console.log(JSON.stringify({ rootFiles: roots.length, programFiles: program.getSourceFiles().length, diagnostics: diagnostics.length, scope: names }))
if (diagnostics.length) console.log(ts.formatDiagnosticsWithColorAndContext(diagnostics, { getCanonicalFileName: x => x, getCurrentDirectory: () => frontend, getNewLine: () => '\n' }))
process.exitCode = diagnostics.length ? 1 : 0
