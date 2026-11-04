// Assembly source checks only. No Vite, canonical decoder or managed cohort runs.
import assert from 'node:assert/strict'
import test from 'node:test'
import { execFileSync } from 'node:child_process'
import { resolve } from 'node:path'
import { mkdtemp, readFile, writeFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { loadQa05Components, qualifiedQa05Components, validateQa05Binding, attachQa05Runtime } from './qa05-runtime.mjs'
import { pinnedSource } from './pinned-source.mjs'

const repo = process.env.MCR_IN04_SOURCE_REPO
assert(repo, 'An allocated source guardian must provide the source repository')
const published = 'f0f2e2187de8eb60f11b350890c6139758cfdb91'
const bytes = path => execFileSync('git', ['show', published + ':' + path], { cwd: repo, maxBuffer: 512 * 1024 })
const backend = 'backend/scripts/mcr/in04_wall_runtime/control.mjs'

test('published immutable components assemble through verified data URLs without control I/O', async () => {
  const result = await loadQa05Components({ source: { bytes }, backendBytes: bytes(backend) })
  assert(Object.isFrozen(result))
  for (const name of ['installRelay', 'createFaultController', 'createCheckpoints', 'createTicketProbe']) assert.equal(typeof result[name], 'function')
  assert.equal(typeof result.ownerControl.inspect, 'function')
})

test('every changed frontend or backend component is refused before use', async () => {
  for (const path of Object.keys(qualifiedQa05Components)) {
    const changed = name => name === path ? Buffer.from('// changed qualification input') : bytes(name)
    await assert.rejects(loadQa05Components({ source: { bytes: changed }, backendBytes: changed(backend) }), /Qualified component bytes differ/)
  }
})

test('canonical contract imports use selected Git bytes rather than physical-checkout fallback', () => {
  const source = pinnedSource(repo, published)
  for (const path of ['contracts/realtime/v1/world.schema.json', 'contracts/core/v1/core.schema.json']) {
    assert.equal(source.plugin.load(resolve(repo, path)), bytes(path).toString())
  }
})

function fixture() {
  const candidate = 'a'.repeat(40), now = Date.now()
  const binding = { candidate, environmentId: 'environment', eventId: 'event', windowId: 'window', expiresAt: new Date(now + 60000).toISOString() }
  const settings = { environmentId: binding.environmentId, eventId: binding.eventId, origin: 'http://127.0.0.1:5555' }
  const config = { releaseRevision: candidate, ...settings, windowId: binding.windowId, members: [{ userId: 'a' }, { userId: 'b' }] }
  return { candidate, now, binding, settings, config, guard() {}, signal: new AbortController().signal }
}

test('frontend and backend candidate/environment/event/origin/window bindings must agree', () => {
  const value = fixture()
  assert.equal(validateQa05Binding(value, value.now), true)
  for (const key of ['releaseRevision', 'environmentId', 'eventId', 'origin', 'windowId']) {
    assert.throws(() => validateQa05Binding({ ...value, config: { ...value.config, [key]: 'foreign' } }, value.now))
  }
  assert.throws(() => validateQa05Binding({ ...value, binding: { ...value.binding, ready: true } }, value.now))
  assert.throws(() => validateQa05Binding({ ...value, config: { ...value.config, members: [{ userId: 'a' }, { userId: 'a' }] } }, value.now))
})

test('window bounds, aborted lifetime and failed guard cannot authorize attachment', () => {
  const value = fixture()
  for (const delta of [5000, 15 * 60000 + 1, -1]) {
    assert.throws(() => validateQa05Binding({ ...value, binding: { ...value.binding, expiresAt: new Date(value.now + delta).toISOString() } }, value.now))
  }
  for (const delta of [5001, 15 * 60000]) assert.equal(validateQa05Binding({ ...value,
    binding: { ...value.binding, expiresAt: new Date(value.now + delta).toISOString() } }, value.now), true)
  const abort = new AbortController(); abort.abort()
  assert.throws(() => validateQa05Binding({ ...value, signal: abort.signal }, value.now))
  assert.throws(() => validateQa05Binding({ ...value, guard: () => false }, value.now))
  assert.throws(() => validateQa05Binding(value, NaN))
})

test('a foreign frontend binding performs no module, decoder, observer or listener work', async () => {
  const value = fixture()
  let effects = 0
  await assert.rejects(attachQa05Runtime({ ...value, repo, candidate: 'b'.repeat(40),
    source: { bytes() { effects++; throw Error('not reached') } },
    server: { ssrLoadModule() { effects++; throw Error('not reached') } } }))
  assert.equal(effects, 0)
})

test('new restart artifact imports verified bytes without replacing the historical artifact or claiming runtime readiness', async () => {
  const temporary = await mkdtemp(resolve(tmpdir(), 'restart-import-'))
  const previous = process.env.MCR_IN04_FRONTEND_HANDOFF
  try {
    const path = resolve(temporary, 'frontend.json')
    // Explicit synthetic import fixture only; no observer socket or server.
    await writeFile(path, JSON.stringify({ candidate: 'a'.repeat(40), environmentId: 'environment', eventId: 'event',
      origin: 'http://127.0.0.1:5555', settingsSha256: 'b'.repeat(64), token: 'c'.repeat(64), phaserModulePath: '/@fs/test/deps/phaser.js' }), { mode: 0o600, flag: 'wx' })
    process.env.MCR_IN04_FRONTEND_HANDOFF = path
    const artifact = await readFile(new URL('./qualified-runtime-restart.mjs', import.meta.url))
    assert(artifact.includes(Buffer.from('40fddf11e1520452b73966c8043939cbc35d65ced0e2935515018ab1051cf350')))
    const module = await import('data:text/javascript;base64,' + artifact.toString('base64'))
    for (const name of ['inspect', 'observeRestart', 'muralBinding']) assert.equal(typeof module.runtimeQualification[name], 'function')
    const historical = await readFile(new URL('./qualified-runtime.mjs', import.meta.url))
    const { sha256 } = await import('./pinned-source.mjs')
    assert.equal(sha256(historical), '7b1807be10b7a6a717bb10e66b1395ef12a6c06fa047db8ccb0a53059408283c')
  } finally {
    if (previous === undefined) delete process.env.MCR_IN04_FRONTEND_HANDOFF
    else process.env.MCR_IN04_FRONTEND_HANDOFF = previous
    await rm(temporary, { recursive: true })
  }
})
