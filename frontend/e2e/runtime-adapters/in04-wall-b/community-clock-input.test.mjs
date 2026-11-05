import assert from 'node:assert/strict'
import test from 'node:test'
import { mkdtemp, writeFile, chmod, symlink, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { createRequire } from 'node:module'
import { pathToFileURL } from 'node:url'
import { readCommunityClockInput, compiledClockIdentity, validateCommunityClockInput, clockInputModule } from './community-clock-input.mjs'
import { buildApplicationRelease } from './application-release.mjs'
import { pinnedSource, sha256 } from './pinned-source.mjs'

const current = { candidate: 'a'.repeat(40), environmentId: 'clock-source-fixture', eventId: '11111111-1111-1111-1111-111111111111',
  origin: 'http://127.0.0.1:54231', bootstrapContractSha256: 'b'.repeat(64), concertBinarySha256: 'c'.repeat(64), serverClockSource: 'd'.repeat(40) }
const context = { candidate: current.candidate, settings: current, source: { files: new Set(['frontend' + clockInputModule]) } }
function fixture(binding = current) {
  return { current: binding, measurement: { kind: 'MCR_C03_CLOCK_MEASUREMENT_V1', classification: 'MEASURED_RUNTIME', evidenceSha256: 'e'.repeat(64),
    binding, browser: { userAgent: 'synthetic source-only test', platform: 'fixture', crossOriginIsolated: false },
    samples: [{ requestStartedMs: 100, responseReceivedMs: 110, serverTime: '2026-11-05T00:00:00Z' },
      { requestStartedMs: 10100, responseReceivedMs: 10110, serverTime: '2026-11-05T00:00:10Z' },
      { requestStartedMs: 20100, responseReceivedMs: 20110, serverTime: '2026-11-05T00:00:20Z' }],
    observationsMs: Array.from({ length: 203 }, (_, i) => i * 100), uninterrupted: true } }
}
async function privateFixture(run, bytes = Buffer.from(JSON.stringify(fixture()))) {
  const directory = await mkdtemp(join(tmpdir(), 'mcr-clock-input-test-'))
  const path = join(directory, 'clock.json')
  try {
    await writeFile(path, bytes, { mode: 0o600, flag: 'wx' })
    await run({ path, sha256: sha256(bytes), current }, directory)
  } finally { await rm(directory, { recursive: true, force: true }) }
}

test('absence supplies no clock input or synthetic measurement', async () => {
  assert.equal(await readCommunityClockInput(undefined, context), null)
  await assert.rejects(readCommunityClockInput(null, context))
})
test('private input retains exact whitespace bytes and a path-free compiled digest', async () => {
  const bytes = Buffer.from('  ' + JSON.stringify(fixture()) + '\n')
  await privateFixture(async descriptor => {
    const input = await readCommunityClockInput(descriptor, context)
    assert.equal(input.raw, bytes.toString())
    assert.deepEqual(compiledClockIdentity(input), { name: 'import.meta.env.VITE_MCR_COMMUNITY_CLOCK_INPUT', encoding: 'utf-8', bytes: bytes.length, sha256: sha256(bytes) })
    assert(Object.isFrozen(input.current))
  }, bytes)
})
test('digest mismatch, public permissions and symbolic links are refused', async () => {
  await privateFixture(async (descriptor, directory) => {
    await assert.rejects(readCommunityClockInput({ ...descriptor, sha256: 'f'.repeat(64) }, context), /hash mismatch/)
    await chmod(descriptor.path, 0o644)
    await assert.rejects(readCommunityClockInput(descriptor, context), /restricted permissions/)
    await chmod(descriptor.path, 0o600)
    const link = join(directory, 'link.json'); await symlink(descriptor.path, link)
    await assert.rejects(readCommunityClockInput({ ...descriptor, path: link }, context))
  })
})
test('independent current binding and descriptor closure fail before ingestion', async () => {
  await privateFixture(async descriptor => {
    for (const key of ['candidate', 'environmentId', 'eventId', 'origin']) {
      await assert.rejects(readCommunityClockInput({ ...descriptor, current: { ...current, [key]: 'different' } }, context), /binding differs/)
    }
    await assert.rejects(readCommunityClockInput({ ...descriptor, extra: true }, context), /Unexpected input fields/)
    await assert.rejects(readCommunityClockInput({ ...descriptor, current: { ...current, extra: true } }, context), /Unexpected input fields/)
    await assert.rejects(readCommunityClockInput(descriptor, { ...context, source: { files: new Set() } }), /lacks the canonical/)
  })
})
test('invalid UTF-8 and oversize files cannot change compiled bytes silently', async () => {
  await privateFixture(async descriptor => { await assert.rejects(readCommunityClockInput(descriptor, context), /UTF-8/) }, Buffer.from([0xff]))
  await privateFixture(async descriptor => { await assert.rejects(readCommunityClockInput(descriptor, context), /size bound/) }, Buffer.alloc(512 * 1024 + 1, 32))
})
test('captured binding cannot substitute for independently supplied current', () => {
  assert.throws(() => validateCommunityClockInput({ raw: '{}', current }, () => ({ current: { ...current, concertBinarySha256: 'f'.repeat(64) } })), /independently supplied/)
  assert.throws(() => validateCommunityClockInput({ raw: '{}', current }, undefined), /decoder is required/)
})
test('selected Git decoder validates the actual SSR path and build identity binds exact compiled bytes', async () => {
  const repo = process.env.MCR_CLOCK_TEST_REPO, candidate = process.env.MCR_CLOCK_TEST_CANDIDATE, dependencies = process.env.MCR_CLOCK_TEST_DEPENDENCIES
  assert(repo && candidate && dependencies, 'An allocated source round must supply the exact existing repository, canonical candidate and readonly dependencies')
  const require = createRequire(resolve(dependencies, '.clock-source-test.cjs'))
  const { createServer } = await import(pathToFileURL(require.resolve('vite')).href)
  const source = pinnedSource(repo, candidate)
  const cacheDirectory = await mkdtemp(join(tmpdir(), 'mcr-clock-ssr-test-'))
  let server
  try {
    server = await createServer({ root: resolve(repo, 'frontend'), configFile: false, envFile: false, publicDir: false, cacheDir: cacheDirectory,
      clearScreen: false, logLevel: 'error', plugins: [source.plugin], resolve: { alias: { zod: resolve(dependencies, 'zod') } },
      optimizeDeps: { noDiscovery: true, include: [] }, server: { middlewareMode: true, hmr: false, watch: null } })
    const { decodeCompiledClockInput } = await server.ssrLoadModule(clockInputModule)
    const actualCurrent = { ...current, candidate }, raw = JSON.stringify(fixture(actualCurrent))
    validateCommunityClockInput({ raw, current: actualCurrent }, decodeCompiledClockInput)
    const bad = fixture(actualCurrent); bad.measurement.uninterrupted = false
    assert.throws(() => validateCommunityClockInput({ raw: JSON.stringify(bad), current: actualCurrent }, decodeCompiledClockInput))
    assert.throws(() => validateCommunityClockInput({ raw, current: { ...actualCurrent, serverClockSource: 'f'.repeat(40) } }, decodeCompiledClockInput))
    const options = { repo, candidate, dependencies, mode: 'vite-serve' }
    const absent = await buildApplicationRelease(options)
    const first = await buildApplicationRelease({ ...options, communityClockInput: { raw } })
    const changed = await buildApplicationRelease({ ...options, communityClockInput: { raw: raw + '\n' } })
    assert.deepEqual(absent.manifest.compiledInputs, [])
    assert.deepEqual(first.manifest.compiledInputs, [compiledClockIdentity({ raw })])
    assert.notEqual(first.release.buildId, absent.release.buildId)
    assert.notEqual(first.release.buildId, changed.release.buildId)
    assert.equal(first.release.sourceRevision, candidate)
  } finally {
    try { await server?.close() } finally { await rm(cacheDirectory, { recursive: true, force: true }) }
  }
})
