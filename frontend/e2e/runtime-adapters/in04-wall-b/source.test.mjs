// Focused implementation checks only. The HTTP fixture below is explicitly
// synthetic and cannot qualify actors, a backend deployment or real IN04.
import assert from 'node:assert/strict'
import { createServer } from 'node:http'
import { mkdtemp, writeFile, chmod, symlink, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'
import test from 'node:test'
import { pinnedSource, sha256 } from './pinned-source.mjs'
import { ownedBytes, loopbackOrigin, productionValidators } from './inputs.mjs'
import { composeRuntimeAdapter } from './adapter.mjs'
import { prepareAdapter } from './prepare.mjs'

const repo = process.env.MCR_IN04_SOURCE_REPO
const candidate = process.env.MCR_IN04_SOURCE_CANDIDATE
assert(repo && candidate, 'Tests require explicit existing repository and published source candidate')

test('canonical loader pins main and rejects absent application imports rather than falling back', () => {
  const source = pinnedSource(repo, candidate)
  assert.match(source.bytes('frontend/index.html').toString(), /src="\/src\/main.tsx"/)
  assert.equal(source.plugin.load(resolve(repo, 'frontend/src/main.tsx')), source.bytes('frontend/src/main.tsx').toString())
  assert.throws(() => source.plugin.resolveId('./absent-in04-runtime.ts', resolve(repo, 'frontend/src/main.tsx')))
  assert.throws(() => source.bytes('frontend/e2e/uncommitted-private-input.json'))
  assert(source.receipts().every(item => /^[a-f0-9]{64}$/.test(item.sha256)))
})

test('only explicit exact loopback origins accepted', () => {
  assert.equal(loopbackOrigin('http://127.0.0.1:3000').port, '3000')
  for (const value of ['https://127.0.0.1:3000', 'http://127.0.0.1:3000/', 'http://a:b@localhost:3000', 'http://example.test:3000', 'http://localhost', 'http://localhost:80']) {
    assert.throws(() => loopbackOrigin(value))
  }
})

test('owned private input denies symlink, permissions, hash and size drift', async () => {
  const directory = await mkdtemp(resolve(tmpdir(), 'mcr-in04-input-check-'))
  try {
    const path = resolve(directory, 'input.json'), bytes = Buffer.from('{"version":1}')
    await writeFile(path, bytes, { mode: 0o600 })
    assert.deepEqual(await ownedBytes(path, { privateOnly: true, hash: sha256(bytes) }), bytes)
    await assert.rejects(ownedBytes(path, { hash: '0'.repeat(64) }))
    await assert.rejects(ownedBytes(path, { maximum: 1 }))
    await symlink(path, resolve(directory, 'link'))
    await assert.rejects(ownedBytes(resolve(directory, 'link')))
    await chmod(path, 0o644)
    await assert.rejects(ownedBytes(path, { privateOnly: true }))
  } finally { await rm(directory, { recursive: true }) }
})

test('uses exact canonical production validators, never a second placement schema', async () => {
  const source = pinnedSource(repo, candidate), validators = await productionValidators(source)
  const mural = JSON.parse(source.bytes('frontend/src/app/world-entry/mural-production/placement.json'))
  assert(validators.mural(mural))
  assert.equal(validators.mural({ ...mural, kind: 'PHYSICAL' }), null)
  assert.equal(validators.localInput({}), null)
})

async function fixture(run) {
  const directory = await mkdtemp(resolve(tmpdir(), 'mcr-in04-adapter-check-'))
  const frontend = { candidate, eventId: '10000000-0000-4000-8000-000000000001', environmentId: '10000000-0000-4000-8000-000000000002',
    token: '1'.repeat(64), settingsSha256: '2'.repeat(64), phaserModulePath: '/@fs/private/synthetic-source-fixture/deps/phaser.js' }
  let status = 200, overrides = {}
  const server = createServer((request, response) => {
    assert.equal(request.headers['x-mcr-in04-observer'], frontend.token)
    const body = JSON.stringify({ ...frontend, token: undefined, expiresAt: new Date(Date.now() + 60000).toISOString(),
      phaserModuleUrl: frontend.origin + frontend.phaserModulePath + '?v=01234567', configuration: { fixture: 'not-production-validation' }, ...overrides })
    response.writeHead(status, { 'content-length': Buffer.byteLength(body), 'content-type': 'application/json' }); response.end(body)
  })
  await new Promise(done => server.listen(0, '127.0.0.1', done))
  frontend.origin = 'http://127.0.0.1:' + server.address().port
  const backendBytes = Buffer.from('export const runtimeQualification={async inspect(c){return {syntheticSourceFixture:true,candidate:c.releaseRevision}},async observeRestart(c,b){return {syntheticSourceFixture:true,before:b}}}')
  const config = { releaseRevision: candidate, eventId: frontend.eventId, environmentId: frontend.environmentId, origin: frontend.origin }
  try { await run({ directory, frontend, backendBytes, config, set: (code, changes) => { status = code; overrides = changes } }) }
  finally { server.closeAllConnections(); await new Promise(done => server.close(done)); await rm(directory, { recursive: true }) }
}

test('delegates actual proof unchanged and binds only observed frontend; no authority derived from config', () => fixture(async ({ frontend, backendBytes, config }) => {
  const adapter = await composeRuntimeAdapter({ backendBytes, backendSha256: sha256(backendBytes), frontend })
  assert.deepEqual(await adapter.inspect(config), { syntheticSourceFixture: true, candidate })
  assert.deepEqual(await adapter.observeRestart(config, { roomEpoch: 'a', streamId: 'b' }), { syntheticSourceFixture: true, before: { roomEpoch: 'a', streamId: 'b' } })
  assert.deepEqual((await adapter.muralBinding(config)).configuration, { fixture: 'not-production-validation' })
  await assert.rejects(adapter.inspect({ ...config, releaseRevision: '0'.repeat(40) }))
}))

test('missing frontend, expired observation and different source/settings fail closed', () => fixture(async ({ frontend, backendBytes, config, set }) => {
  const adapter = await composeRuntimeAdapter({ backendBytes, backendSha256: sha256(backendBytes), frontend })
  for (const [status, changes] of [[503, {}], [200, { expiresAt: '2000-01-01T00:00:00Z' }], [200, { settingsSha256: '3'.repeat(64) }], [200, { candidate: '0'.repeat(40) }],
    [200, { phaserModuleUrl: 'https://different.test/phaser.js' }], [200, { phaserModuleUrl: frontend.origin + frontend.phaserModulePath }],
    [200, { phaserModuleUrl: frontend.origin + frontend.phaserModulePath + '?v=0123&extra=1' }],
    [200, { phaserModuleUrl: frontend.origin + frontend.phaserModulePath + '?v=ABCD' }]]) {
    set(status, changes); await assert.rejects(adapter.muralBinding(config))
  }
}))

test('backend source mismatch or missing capability refuses composition', () => fixture(async ({ frontend, backendBytes }) => {
  await assert.rejects(composeRuntimeAdapter({ backendBytes, backendSha256: '0'.repeat(64), frontend }))
  const absent = Buffer.from('export const runtimeQualification={}');
  await assert.rejects(composeRuntimeAdapter({ backendBytes: absent, backendSha256: sha256(absent), frontend }))
}))

test('emitted private adapter is self-contained through QA-style data URL loading', () => fixture(async ({ directory, frontend, backendBytes, config }) => {
  const backendPath = resolve(directory, 'backend.mjs'), frontendHandoffPath = resolve(directory, 'frontend.json'), outputPath = resolve(directory, 'adapter.mjs')
  await writeFile(backendPath, backendBytes, { mode: 0o600 }); await writeFile(frontendHandoffPath, JSON.stringify(frontend), { mode: 0o600 })
  const result = await prepareAdapter({ backendPath, backendSha256: sha256(backendBytes), frontendHandoffPath, outputPath })
  const bytes = await ownedBytes(outputPath, { privateOnly: true, maximum: 1024 * 1024, hash: result.adapterSha256 })
  const loaded = await import('data:text/javascript;base64,' + bytes.toString('base64'))
  assert.deepEqual(await loaded.runtimeQualification.inspect(config), { syntheticSourceFixture: true, candidate })
  await assert.rejects(prepareAdapter({ backendPath, backendSha256: sha256(backendBytes), frontendHandoffPath, outputPath }))
  assert.equal(pathToFileURL(outputPath).protocol, 'file:')
}))
