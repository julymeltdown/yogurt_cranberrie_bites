import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { createHash } from 'node:crypto'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import ts from 'typescript'

const frontend = resolve(dirname(fileURLToPath(import.meta.url)), '../../../..')
const config = resolve(frontend, 'src/app/feed-entry/__tests__/tsconfig.json')
const overlays = new Map()
for (const [key, relative, required] of [
  ['MCR_FEED_ROOT_SOCIAL', 'src/app/social-composition/panels.tsx', true],
  ['MCR_FEED_ROOT_MEDIA', 'src/app/media-entry/RootMediaView.tsx', false],
]) {
  const file = process.env[key + '_FILE'], hash = process.env[key + '_SHA256']
  if (!required && !file && !hash) continue
  assert(file?.startsWith('/') && /^[a-f0-9]{64}$/.test(hash ?? ''), 'Use the exact borrowed owner module')
  const source = readFileSync(file, 'utf8')
  assert.equal(createHash('sha256').update(source).digest('hex'), hash)
  overlays.set(resolve(frontend, relative), source)
}
const input = ts.readConfigFile(config, ts.sys.readFile)
if (input.error) throw Error(ts.flattenDiagnosticMessageText(input.error.messageText, '\n'))
const parsed = ts.parseJsonConfigFileContent(input.config, ts.sys, dirname(config))
const options = { ...parsed.options, noEmit: true, incremental: false }
const host = ts.createCompilerHost(options), original = host.getSourceFile.bind(host)
const used = new Set()
host.getSourceFile = (file, languageVersion, onError, shouldCreateNewSourceFile) => {
  const path = resolve(file), source = overlays.get(path)
  if (source === undefined) return original(file, languageVersion, onError, shouldCreateNewSourceFile)
  used.add(path)
  return ts.createSourceFile(file, source, languageVersion, true, ts.ScriptKind.TSX)
}
const program = ts.createProgram(parsed.fileNames, options, host)
const diagnostics = [...parsed.errors, ...ts.getPreEmitDiagnostics(program)]
if (diagnostics.length) {
  process.stderr.write(ts.formatDiagnostics(diagnostics, { getCurrentDirectory: () => frontend,
    getCanonicalFileName: file => file, getNewLine: () => '\n' }))
  process.exitCode = 1
}
for (const path of overlays.keys()) assert(used.has(path), 'Expected owner source was not part of the type program')
process.stdout.write(JSON.stringify({ noEmit: true, incremental: false, allowImportingTsExtensions: options.allowImportingTsExtensions,
  borrowedModules: [...used], diagnostics: diagnostics.length }) + '\n')
