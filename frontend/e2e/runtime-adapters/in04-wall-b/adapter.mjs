// Self-contained: the QA runner imports verified bytes through a data URL.
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'

const hash = bytes => createHash('sha256').update(bytes).digest('hex')
const dataModule = bytes => import('data:text/javascript;base64,' + Buffer.from(bytes).toString('base64'))

export async function composeRuntimeAdapter({ backendBytes, backendSha256, frontend }) {
  assert.match(backendSha256, /^[a-f0-9]{64}$/)
  assert.equal(hash(backendBytes), backendSha256, 'Backend adapter source hash mismatch')
  assert(backendBytes.length > 0 && backendBytes.length <= 512 * 1024)
  const backend = (await dataModule(backendBytes)).runtimeQualification
  assert(backend && typeof backend.inspect === 'function' && typeof backend.observeRestart === 'function', 'Genuine backend qualification adapter is required')
  assert.match(frontend.token, /^[a-f0-9]{64}$/)
  assert.match(frontend.settingsSha256, /^[a-f0-9]{64}$/)
  assert.match(frontend.candidate, /^[a-f0-9]{40}$/)
  assert(typeof frontend.phaserModulePath === 'string' && frontend.phaserModulePath.startsWith('/@fs/') && frontend.phaserModulePath.endsWith('/deps/phaser.js'))
  const origin = new URL(frontend.origin)
  assert.equal(origin.origin, frontend.origin)
  assert.equal(origin.protocol, 'http:')
  assert(['localhost', '127.0.0.1', '[::1]'].includes(origin.hostname))
  const sameConfig = config => {
    assert.equal(config.releaseRevision, frontend.candidate)
    assert.equal(config.origin, frontend.origin)
    assert.equal(config.eventId, frontend.eventId)
    assert.equal(config.environmentId, frontend.environmentId)
  }
  async function currentFrontend(config) {
    sameConfig(config)
    const response = await fetch(frontend.origin + '/.mcr-in04/frontend', {
      headers: { 'x-mcr-in04-observer': frontend.token }, redirect: 'error', cache: 'no-store', signal: AbortSignal.timeout(5000),
    })
    assert.equal(response.status, 200, 'Canonical frontend server is unavailable')
    assert(Number(response.headers.get('content-length')) > 0 && Number(response.headers.get('content-length')) <= 32768)
    const reader = response.body.getReader(), chunks = []
    let length = 0
    try {
      for (;;) {
        const next = await reader.read()
        if (next.done) break
        length += next.value.length; assert(length <= 32768, 'Frontend observation size bound')
        chunks.push(next.value)
      }
    } finally { await reader.cancel() }
    const value = JSON.parse(Buffer.concat(chunks).toString())
    for (const key of ['candidate', 'eventId', 'environmentId', 'settingsSha256']) assert.equal(value[key], frontend[key])
    assert.equal(value.origin, frontend.origin)
    assert(Date.parse(value.expiresAt) > Date.now() && Date.parse(value.expiresAt) <= Date.now() + 15 * 60 * 1000)
    const phaser = new URL(value.phaserModuleUrl)
    assert.equal(phaser.origin, frontend.origin); assert.equal(phaser.pathname, frontend.phaserModulePath)
    assert(!phaser.username && !phaser.password && !phaser.hash && /^\?v=[a-f0-9]+$/.test(phaser.search))
    return value
  }
  return Object.freeze({
    async inspect(config) {
      await currentFrontend(config)
      // Pass the actual backend proof through unchanged. Config is not evidence.
      return backend.inspect(config)
    },
    async observeRestart(config, before) {
      await currentFrontend(config)
      return backend.observeRestart(config, before)
    },
    async muralBinding(config) {
      const value = await currentFrontend(config)
      // Mural placement grants no authority; QA validates it with the production
      // validator, actual C05 map and full real-Phaser raster observation.
      return { candidate: value.candidate, eventId: value.eventId, expiresAt: value.expiresAt,
        configuration: value.configuration, phaserModuleUrl: value.phaserModuleUrl }
    },
  })
}
