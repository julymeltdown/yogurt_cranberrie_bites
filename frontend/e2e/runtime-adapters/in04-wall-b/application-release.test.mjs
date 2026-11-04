// Source checks only; no Vite, browser, server catalog or admission is started.
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { createRequire, stripTypeScriptTypes } from 'node:module'
import { readFile, mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'
import test from 'node:test'
import { ROOT_APPLICATION_VERSION, buildApplicationRelease, canonicalJson } from './application-release.mjs'
import { sha256 } from './pinned-source.mjs'

const repo = process.env.MCR_IN04_SOURCE_REPO
const candidate = process.env.MCR_IN04_SOURCE_CANDIDATE
const dependencies = '/Users/lhs/Desktop/mcr/frontend/node_modules'
const clientPath = resolve(repo, 'frontend/src/shared/release/application.ts')
const clientBytes = await readFile(clientPath)
const client = await import('data:text/javascript;base64,' + Buffer.from(stripTypeScriptTypes(clientBytes.toString(), { mode: 'strip' })).toString('base64'))

test('application release has a closed immutable compatibility shape without an absent-input fallback', async () => {
  const valid = { version: 1, applicationVersion: '0.1.0', buildId: 'a'.repeat(64), sourceRevision: candidate }
  const value = client.readApplicationRelease(valid)
  assert.deepEqual(value, valid); assert(Object.isFrozen(value)); assert.notEqual(value, valid)
  assert.equal(client.capturedApplicationRelease(), null)
  const accessor = { ...valid }
  Object.defineProperty(accessor, 'buildId', { enumerable: true, get() { throw new Error('must not read a claim accessor') } })
  assert.equal(client.readApplicationRelease(accessor), null)
  assert.equal(client.readApplicationRelease(new Proxy({}, { ownKeys() { throw new Error('unavailable') } })), null)
  assert.equal(client.readApplicationRelease({ ...valid, [Symbol('private')]: true }), null)
  const require = createRequire(resolve(dependencies, '.mcr-application-release-types.cjs'))
  const vite = await import(pathToFileURL(require.resolve('vite')).href)
  const output = await vite.transformWithOxc(clientBytes.toString(), clientPath, { lang: 'ts', sourceType: 'module', tsconfig: false,
    define: { 'import.meta.env.VITE_MCR_APPLICATION_RELEASE': JSON.stringify(valid) } })
  const injected = await import('data:text/javascript;base64,' + Buffer.from(output.code).toString('base64'))
  assert.deepEqual(injected.capturedApplicationRelease(), valid)
  assert(Object.isFrozen(injected.capturedApplicationRelease()))
  for (const invalid of [undefined, null, [], '0.1.0', {}, { ...valid, extra: true }, { ...valid, buildId: 'A'.repeat(64) },
    { ...valid, sourceRevision: 'main' }, { ...valid, applicationVersion: '0.1.0-beta' },
    { ...valid, buildId: valid.buildId + '\n' }, { ...valid, sourceRevision: candidate + '\n' }]) assert.equal(client.readApplicationRelease(invalid), null)
})

test('application versions use bounded ASCII numeric tuples, rejecting ambiguous or overflowing claims', () => {
  assert.deepEqual(client.applicationVersionTuple('0.0.0'), [0, 0, 0])
  assert.deepEqual(client.applicationVersionTuple('65535.65535.65535'), [65535, 65535, 65535])
  assert(client.applicationVersionTuple('10.0.0')[0] > client.applicationVersionTuple('2.0.0')[0])
  for (const value of ['01.0.0', '0.00.0', '0.0.01', '65536.0.0', '999999999999999999.0.0', '1.2', '1.2.3.4',
    '-1.0.0', '1.2.3+build', '１.2.3', '1.2.3\n', ' 1.2.3', null, 123]) assert.equal(client.applicationVersionTuple(value), null)
})

test('generated real-source manifest independently binds source bytes, toolchain and explicit version; it is not an output bundle receipt', async () => {
  const manifest = JSON.parse(await readFile(process.env.MCR_APPLICATION_MANIFEST_PATH, 'utf8'))
  const { buildId, ...identity } = manifest
  assert.equal(buildId, sha256(Buffer.from(canonicalJson(identity))))
  assert.equal(identity.applicationVersion, ROOT_APPLICATION_VERSION)
  assert.equal(identity.sourceRevision, candidate); assert.equal(identity.mode, 'vite-serve')
  assert.equal(identity.kind, 'APPLICATION_BUILD_INPUT_IDENTITY')
  assert.deepEqual(identity.emittedOutputs, { status: 'UNAVAILABLE', reason: 'NOT_BUILT_BY_THIS_PUBLISHER', files: [] })
  assert(identity.inputs.length > 0 && identity.inputs.every(value => /^[a-f0-9]{64}$/.test(value.sha256)))
  assert.equal(new Set(identity.inputs.map(value => value.path)).size, identity.inputs.length)
  assert(identity.inputs.some(value => value.path.startsWith('frontend/public/')))
  assert(!identity.inputs.some(value => value.path.includes('/__tests__/')))
  const selected = ['frontend/index.html', 'frontend/package.json', 'frontend/pnpm-lock.yaml', 'frontend/src/main.tsx']
  for (const path of selected) {
    const entry = identity.inputs.find(value => value.path === path)
    const bytes = execFileSync('git', ['show', candidate + ':' + path], { cwd: repo })
    assert(entry); assert.equal(entry.bytes, bytes.length); assert.equal(entry.sha256, sha256(bytes))
  }
  for (const entry of identity.assemblyInputs) assert.equal(entry.sha256, sha256(await readFile(new URL(entry.name, import.meta.url))))
  assert.equal(identity.toolchain.node, process.versions.node)
  assert(identity.toolchain.packages.some(value => value.name === 'vite' && value.version))
  assert(!JSON.stringify(manifest).includes(repo)); assert(!JSON.stringify(manifest).includes(dependencies))
  assert.notEqual(sha256(Buffer.from(canonicalJson({ ...identity, applicationVersion: '0.1.1' }))), buildId)
  assert.notEqual(sha256(Buffer.from(canonicalJson({ ...identity, mode: 'vite-build' }))), buildId)
  assert.notEqual(sha256(Buffer.from(canonicalJson({ ...identity, toolchain: { ...identity.toolchain, node: 'different' } }))), buildId)
  assert.notEqual(sha256(Buffer.from(canonicalJson({ ...identity, sourceRevision: '0'.repeat(40) }))), buildId)
  assert.equal(canonicalJson({ z: 1, a: { y: 2, b: 3 } }), canonicalJson({ a: { b: 3, y: 2 }, z: 1 }))
})

test('unsupported build modes, unpinned sources and mismatched dependency inputs refuse before bulk source hashing', async () => {
  await assert.rejects(buildApplicationRelease({ repo, candidate: 'main', dependencies, mode: 'vite-serve' }))
  await assert.rejects(buildApplicationRelease({ repo, candidate, dependencies, mode: 'preview' }))
  const directory = await mkdtemp(resolve(tmpdir(), 'mcr-application-input-source-'))
  try {
    await mkdir(resolve(directory, 'node_modules'))
    await writeFile(resolve(directory, 'package.json'), '{}')
    await assert.rejects(buildApplicationRelease({ repo, candidate, dependencies: resolve(directory, 'node_modules'), mode: 'vite-serve' }), /Dependency declaration differs/)
  } finally { await rm(directory, { recursive: true }) }
})

test('the new standalone client release leaf passes strict TypeScript with the actual Vite environment types', () => {
  const require = createRequire(resolve(dependencies, '.mcr-application-release-types.cjs'))
  const ts = require('typescript')
  const program = ts.createProgram([clientPath, resolve(dependencies, 'vite/client.d.ts')], {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext, moduleResolution: ts.ModuleResolutionKind.Bundler,
    noEmit: true, strict: true, noUnusedLocals: true, noUnusedParameters: true, exactOptionalPropertyTypes: true,
    skipLibCheck: true, types: [],
  })
  const diagnostics = ts.getPreEmitDiagnostics(program)
  assert.equal(diagnostics.length, 0, ts.formatDiagnosticsWithColorAndContext(diagnostics, {
    getCanonicalFileName: name => name, getCurrentDirectory: () => repo, getNewLine: () => '\n',
  }))
})
