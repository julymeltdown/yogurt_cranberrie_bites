import assert from 'node:assert/strict'
import test from 'node:test'
import { acceptsRequest, bindRuntimePacket, parseServingConfiguration, servingOptions } from './serve.mjs'

const now = Date.parse('2026-11-04T17:00:00Z')
const configuration = () => ({ version: 'mcr-in03-serving/v1', allocation: 'UNIT_SERVE_ONLY',
  origin: 'http://127.0.0.1:5173', gatewayOrigin: 'http://127.0.0.1:63320',
  availabilityRevision: 'a'.repeat(40), frontendRevision: 'b'.repeat(40),
  runtimePacket: '/private/unit-only/packet.json', activeUntil: '2026-11-04T17:08:00Z' })
const packet = () => ({ browser_origin: configuration().origin, gateway_url: configuration().gatewayOrigin,
  event_id: '9ab386e2-a82a-41e4-829d-000000000010', runtime: { private: true,
    origin: configuration().origin, eventId: '9ab386e2-a82a-41e4-829d-000000000010',
    controlOwner: 'unit-private-control-owner', expiresAt: '2026-11-04T17:05:00Z' },
  authProducer: { origin: configuration().origin, eventId: '9ab386e2-a82a-41e4-829d-000000000010',
    controlOwner: 'unit-private-control-owner', expiresAt: '2026-11-04T17:05:00Z', custody: 'PRIVATE_MODE_0600' } })

test('accepts only an explicit closed private serving configuration', () => {
  const config = parseServingConfiguration(configuration(), now)
  assert(Object.isFrozen(config)); assert.equal(config.origin, configuration().origin)
  for (const change of [{ token: 'unit-never-accepted' }, { version: 'other' }, { allocation: '' },
    { availabilityRevision: '' }, { frontendRevision: 'unqualified' }, { runtimePacket: 'relative.json' },
    { activeUntil: '2026-11-04T16:59:59Z' }]) assert.throws(() => parseServingConfiguration({ ...configuration(), ...change }, now))
})
test('rejects external, ambiguous, credentialed, self-proxy and non-origin URLs', () => {
  for (const origin of ['https://127.0.0.1:5173', 'http://localhost:5173', 'http://0.0.0.0:5173', 'http://example.test:5173',
    'http://127.0.0.1:80', 'http://user:password@127.0.0.1:5173', 'http://127.0.0.1:5173/path', 'http://127.0.0.1:5173/',
    'http://127.0.0.1:5173?query', 'http://127.0.0.1:5173#fragment']) {
    assert.throws(() => parseServingConfiguration({ ...configuration(), origin }, now))
    assert.throws(() => parseServingConfiguration({ ...configuration(), gatewayOrigin: origin }, now))
  }
  assert.throws(() => parseServingConfiguration({ ...configuration(), gatewayOrigin: configuration().origin }, now))
})
test('binds the exact borrowed origin/event/owner and earlier runtime expiry without making an Auth claim', () => {
  const config = parseServingConfiguration(configuration(), now)
  assert.equal(bindRuntimePacket(config, packet(), now), Date.parse('2026-11-04T17:05:00Z'))
  assert.equal(bindRuntimePacket({ ...config, activeUntil: '2026-11-04T17:02:00Z' }, packet(), now), Date.parse('2026-11-04T17:02:00Z'))
  for (const field of ['origin', 'eventId', 'controlOwner', 'expiresAt']) {
    const input = packet(); input.runtime[field] = 'foreign'
    assert.throws(() => bindRuntimePacket(config, input, now))
  }
  assert.throws(() => bindRuntimePacket(config, packet(), Date.parse('2026-11-04T17:05:00Z')))
  assert.throws(() => bindRuntimePacket({ ...config, activeUntil: '2026-11-04T16:59:59Z' }, packet(), now))
  const input = packet(); input.runtime.private = false; assert.throws(() => bindRuntimePacket(config, input, now))
})
test('rejects a foreign Gateway or producer custody without inspecting actor credentials', () => {
  const config = parseServingConfiguration(configuration(), now)
  for (const change of [{ gateway_url: 'http://127.0.0.1:63321' }, { browser_origin: 'http://127.0.0.1:5174' },
    { event_id: 'foreign' }, { authProducer: { ...packet().authProducer, custody: 'PUBLIC' } }]) {
    assert.throws(() => bindRuntimePacket(config, { ...packet(), ...change }, now))
  }
})
test('requires the exact Host and same-origin browser authority while preserving API query values', () => {
  const config = configuration(), request = { url: '/api/concert/events/example/feed?limit=20', headers: { host: '127.0.0.1:5173', origin: config.origin } }
  assert.equal(acceptsRequest(request, config), true)
  assert.equal(acceptsRequest({ ...request, headers: { host: '127.0.0.1:5173' } }, config), true)
  for (const headers of [{ ...request.headers, host: 'example.test:5173' }, { ...request.headers, origin: 'null' },
    { ...request.headers, origin: 'http://127.0.0.1:5174' }, { ...request.headers, 'sec-fetch-site': 'cross-site' }]) {
    assert.equal(acceptsRequest({ ...request, headers }, config), false)
  }
})
test('rejects absolute/protocol-relative paths, traversal and invalid encoding', () => {
  for (const url of ['http://127.0.0.1:63320/api', '//example.test', '/%2fexample.test', '/%2f/example.test',
    '/api/../private', '/api/%2e%2e/private', '/.git/config', '/api\\private', '/api/%5cprivate', '/api/%ZZ']) {
    assert.equal(acceptsRequest({ url, headers: { host: '127.0.0.1:5173' } }, configuration()), false)
  }
})
test('uses isolated Vite options, pinned public revision and a cookie/Origin-preserving API-only proxy', () => {
  const options = servingOptions(configuration(), { repository: '/unit/repository', frontend: '/unit/repository/frontend', dependencies: '/unit/deps', cache: '/unit/private-cache' }, [], packet().event_id)
  assert.equal(options.configFile, false); assert.equal(options.envDir, false); assert.deepEqual(options.envPrefix, [])
  assert.equal(options.define['import.meta.env.VITE_MCR_IN03_AVAILABILITY_REVISION'], JSON.stringify(configuration().availabilityRevision))
  assert.equal(options.define['import.meta.env.VITE_MCR_EVENT_ID'], JSON.stringify(packet().event_id))
  assert.throws(() => servingOptions(configuration(), {}, [], undefined))
  assert.throws(() => servingOptions(configuration(), {}, [], 'guessed-event'))
  assert.equal(options.server.host, '127.0.0.1'); assert.equal(options.server.port, 5173); assert.equal(options.server.strictPort, true)
  assert.equal(options.server.hmr, false); assert.equal(options.server.ws, false); assert.equal(options.server.cors, false)
  const [pattern, proxy] = Object.entries(options.server.proxy)[0]
  assert(new RegExp(pattern).test('/api/concert/')); assert(!new RegExp(pattern).test('/apiculture'))
  assert.equal(proxy.target, configuration().gatewayOrigin); assert.equal(proxy.changeOrigin, false)
  assert.equal(proxy.ws, false); assert.equal(proxy.followRedirects, false); assert.equal(proxy.xfwd, false)
  assert.equal(proxy.rewrite, undefined); assert.equal(proxy.headers, undefined); assert.equal(proxy.cookieDomainRewrite, undefined)
})
test('proxy failure emits an unqualified plain 502, never a fabricated domain denial or 200', () => {
  const proxy = Object.values(servingOptions(configuration(), {}, [], packet().event_id).server.proxy)[0]
  let handler, status, content, body
  proxy.configure({ on(event, callback) { assert.equal(event, 'error'); handler = callback } })
  handler(Error('unit-private-upstream-detail'), {}, { headersSent: false, writeHead(code, headers) { status = code; content = headers }, end(value) { body = value } })
  assert.equal(status, 502); assert.equal(content['Content-Type'], 'text/plain')
  assert(!body.includes('unit-private-upstream-detail')); assert(!body.includes('200'))
})
