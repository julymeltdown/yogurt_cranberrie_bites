// TARGETED changed TEST_ONLY fixture strict check; original options and all transitive imports remain real. No full-program qualification claim.
import { createRequire } from 'node:module'
import { resolve, dirname } from 'node:path'
import { root, front, paths, has, bytes } from './git-inputs.mjs'
const require = createRequire(resolve(front, 'package.json'))
const ts = require('typescript')
const directories = new Set()
for (const p of paths) { let d = dirname(resolve(root, p)); while (d.startsWith(root)) { directories.add(d); d = dirname(d) } }
const readFile = path => has(path) ? bytes(path)?.toString() : ts.sys.readFile(path)
const fileExists = path => has(path) || ts.sys.fileExists(path)
const configPath = resolve(front, 'tsconfig.json')
const config = ts.readConfigFile(configPath, readFile)
if (config.error) throw Error(ts.flattenDiagnosticMessageText(config.error.messageText, '\n'))
const parseHost = { ...ts.sys, readFile, fileExists,
  readDirectory(directory, extensions, excludes, includes, depth) {
    const disk = ts.sys.readDirectory(directory, extensions, excludes, includes, depth)
    // This is exactly the ordinary include: src and vite.config.ts; no file exclusion.
    const canonical = [...paths].filter(p => p.startsWith('frontend/src/') && /\.(?:ts|tsx)$/.test(p)).map(p => resolve(root, p))
    return [...new Set([...disk, ...canonical])]
  } }
const parsed = ts.parseJsonConfigFileContent(config.config, parseHost, front)
parsed.fileNames = [resolve(front, 'src/app/world-entry/__tests__/parade-ink-map-places/rooms-virtual-help.ts')]
parsed.options.incremental = false
parsed.options.paths = { ...parsed.options.paths, zod: ['/Users/lhs/Desktop/mcr/frontend/node_modules/.pnpm/zod@4.4.3/node_modules/zod/index.d.cts'] }
const host = ts.createCompilerHost(parsed.options)
host.readFile = readFile; host.fileExists = fileExists
const originalDirectoryExists = host.directoryExists?.bind(host)
host.directoryExists = p => directories.has(p) || !!originalDirectoryExists?.(p)
const marker = phase => console.log(JSON.stringify({ phase, rootFiles: parsed.fileNames.length, memory: process.memoryUsage() }))
marker('BEFORE_TARGETED_FIXTURE_PROGRAM')
const program = ts.createProgram(parsed.fileNames, parsed.options, host)
marker('AFTER_TARGETED_FIXTURE_PROGRAM')
const diagnostics = [...parsed.errors, ...ts.getPreEmitDiagnostics(program)]
marker('AFTER_TARGETED_FIXTURE_DIAGNOSTICS')
console.log(JSON.stringify({ qualification: 'TARGETED_CHANGED_TEST_FIXTURE_STRICT_WITH_REAL_TRANSITIVE_IMPORTS', rootFiles: parsed.fileNames.length, sourceFiles: program.getSourceFiles().length, diagnostics: diagnostics.length }))
if (diagnostics.length) console.error(ts.formatDiagnosticsWithColorAndContext(diagnostics, { getCanonicalFileName: p => p, getCurrentDirectory: () => front, getNewLine: () => '\n' }))
process.exitCode = diagnostics.length ? 1 : 0
