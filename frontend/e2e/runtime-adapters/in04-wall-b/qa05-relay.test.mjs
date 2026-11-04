import assert from 'node:assert/strict'
import { test } from 'node:test'
import http from 'node:http'
import net from 'node:net'
import { createHash } from 'node:crypto'
import { once } from 'node:events'
import { createServerFrameReader, installQa05WorldRelay } from './qa05-relay.mjs'

function frame(payload, opcode = 1) {
  const bytes = Buffer.from(payload), header = Buffer.alloc(bytes.length < 126 ? 2 : bytes.length <= 65535 ? 4 : 10)
  header[0] = 0x80 | opcode
  header[1] = bytes.length < 126 ? bytes.length : bytes.length <= 65535 ? 126 : 127
  if (header.length === 4) header.writeUInt16BE(bytes.length, 2)
  if (header.length === 10) header.writeBigUInt64BE(BigInt(bytes.length), 2)
  return Buffer.concat([header, bytes])
}
const decode = text => {
  // Component fixture ONLY. Actual attachment must supply canonical C05 decode.
  const value = JSON.parse(text)
  assert(value && ['snapshot', 'delta'].includes(value.type))
  return value
}
test('opaque original bytes survive arbitrary TCP boundaries, multiple frames and controls', () => {
  const received = [], controls = []
  const reader = createServerFrameReader({ application: (bytes, message) => received.push([bytes, message.type]),
    control: bytes => controls.push(bytes), decodeServer: decode, guard() {} })
  const a = frame('{"type":"snapshot"}'), ping = frame('x', 9), b = frame('{"type":"delta"}')
  const wire = Buffer.concat([a, ping, b])
  for (let i = 0; i < wire.length; i += 3) reader.feed(wire.subarray(i, i + 3))
  assert.deepEqual(received, [[a, 'snapshot'], [b, 'delta']]); assert.deepEqual(controls, [ping])
  assert.equal(reader.bufferedBytes(), 0)
})
test('extended lengths retain exact 126 and 65536 byte application messages', () => {
  for (const length of [126, 65536]) {
    const sent = frame(' '.repeat(length)), actual = []
    const reader = createServerFrameReader({ application: bytes => actual.push(bytes), control() {}, decodeServer: text => ({ length: text.length }), guard() {} })
    for (let i = 0; i < sent.length; i += 511) reader.feed(sent.subarray(i, i + 511))
    assert.deepEqual(actual, [sent])
  }
})
test('masked, compressed, fragmented, binary, noncanonical and oversized server frames retire without forwarding', () => {
  for (const bytes of [Buffer.from([0x81, 0x80]), Buffer.from([0xc1, 0]), Buffer.from([1, 0]),
    Buffer.from([0x82, 0]), Buffer.from([0x81, 126, 0, 1]), Buffer.from([0x81, 127, 0, 0, 0, 0, 0, 1, 0, 1])]) {
    let forwarded = 0
    const reader = createServerFrameReader({ application() { forwarded++ }, control() { forwarded++ }, decodeServer: decode, guard() {} })
    assert.throws(() => reader.feed(bytes), /QA05_SERVER_FRAME_REFUSED/)
    assert.equal(forwarded, 0); assert.equal(reader.bufferedBytes(), 0)
    assert.throws(() => reader.feed(frame('{"type":"delta"}')), /QA05_READER_RETIRED/)
  }
})
test('UTF-8, canonical decoder, expired guard and fallible sink are fail closed', () => {
  const bad = [
    { wire: frame(Buffer.from([0xc0, 0xaf])), decodeServer: decode, guard() {}, application() {} },
    { wire: frame('{"type":"invented"}'), decodeServer: decode, guard() {}, application() {} },
    { wire: frame('{"type":"delta"}'), decodeServer: decode, guard() { throw Error('expired') }, application() {} },
    { wire: frame('{"type":"delta"}'), decodeServer: decode, guard() {}, application() { throw Error('uncertain') } },
  ]
  for (const opts of bad) {
    const reader = createServerFrameReader({ ...opts, control() {} })
    assert.throws(() => reader.feed(opts.wire), /QA05_SERVER_FRAME_REFUSED/)
    assert.equal(reader.bufferedBytes(), 0)
  }
})

function deadline(ms = 3000) { return AbortSignal.timeout(ms) }
async function listen(server) {
  server.listen(0, '127.0.0.1'); await once(server, 'listening', { signal: deadline() })
  return `http://127.0.0.1:${server.address().port}`
}
async function awaitCondition(check) {
  const until = performance.now() + 2000
  while (!check()) {
    assert(performance.now() < until, 'Loopback condition timed out')
    await new Promise(resolve => setTimeout(resolve, 5))
  }
}
test('actual loopback raw upgrade, principal binding, one delta drop and abort use the same sockets', async t => {
  assert(process.env.MCR_QA05_FRAME_FACTORY, 'Qualified factory data URL is required')
  const { createFrameFaultController } = await import(process.env.MCR_QA05_FRAME_FACTORY)
  const peers = new Set(), seen = []
  const backend = http.createServer(), frontend = http.createServer(), controller = new AbortController()
  let actualBackend = null, relay
  const sockets = new Set()
  for (const server of [backend, frontend]) server.on('connection', socket => { sockets.add(socket); socket.on('close', () => sockets.delete(socket)) })
  t.after(async () => {
    controller.abort(); relay?.close()
    for (const socket of sockets) socket.destroy()
    await Promise.all([new Promise(resolve => backend.close(resolve)), new Promise(resolve => frontend.close(resolve))])
  })
  const worldOrigin = await listen(backend), frontendOrigin = await listen(frontend)
  backend.on('upgrade', (req, socket) => {
    seen.push({ target: req.url, origin: req.headers.origin, host: req.headers.host, authorization: req.headers.authorization })
    const accept = createHash('sha1').update(req.headers['sec-websocket-key'] + '258EAFA5-E914-47DA-95CA-C5AB0DC85B11').digest('base64')
    socket.write('HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: ' + accept + '\r\n\r\n')
    peers.add(socket); actualBackend = socket
    socket.write(frame(JSON.stringify({ type: 'snapshot', eventId: 'test-event', selfUserId: 'actual-a' })))
  })
  relay = installQa05WorldRelay({ httpServer: frontend, worldOrigin, frontendOrigin, eventId: 'test-event',
    principalIds: ['actual-a', 'actual-b'], createFaultController: createFrameFaultController, decodeServer: decode,
    guard() {}, signal: controller.signal })
  assert.throws(() => relay.arm('actual-a', {}), /ACTUAL_PRINCIPAL_NOT_OBSERVED/)
  const foreign = net.connect(new URL(frontendOrigin).port, '127.0.0.1')
  foreign.on('error', () => {})
  await once(foreign, 'connect', { signal: deadline() })
  const foreignClosed = once(foreign, 'close', { signal: deadline() })
  foreign.write('GET /world-ws HTTP/1.1\r\nHost: original\r\nOrigin: http://foreign.invalid\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\n')
  await foreignClosed
  assert.equal(seen.length, 0)
  assert.equal(relay.summary().acceptedConnections, 0)
  const client = net.connect(new URL(frontendOrigin).port, '127.0.0.1'), received = []
  client.on('data', bytes => received.push(Buffer.from(bytes)))
  client.on('error', () => {})
  await once(client, 'connect', { signal: deadline() })
  client.write('GET /world-ws?original=1 HTTP/1.1\r\nHost: exact-original-host\r\nOrigin: ' + frontendOrigin + '\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nAuthorization: original-test-only\r\n\r\n')
  await awaitCondition(() => relay.summary().observedPrincipals === 1 && received.length > 0)
  assert.equal(peers.size, 1)
  assert.deepEqual(seen, [{ target: '/world-ws?original=1', origin: frontendOrigin, host: 'exact-original-host', authorization: 'original-test-only' }])
  const proof = relay.arm('actual-a', { caseId: 'packet-gap', requestId: 'test-gap', action: 'drop-next-delta', maximumFrames: 1, maximumMs: 500 })
  assert.equal(proof.state, 'ARMED')
  const dropped = frame('{"type":"delta","marker":"first"}'), kept = frame('{"type":"delta","marker":"second"}')
  actualBackend.write(Buffer.concat([dropped, kept]))
  await awaitCondition(() => relay.proof('actual-a').receivedFrames === 3 && Buffer.concat(received).includes(kept))
  assert(!Buffer.concat(received).includes(dropped))
  assert.equal(relay.proof('actual-a').fault.droppedFrames, 1)
  const clientClosed = once(client, 'close', { signal: deadline() })
  controller.abort(); await clientClosed
  assert.deepEqual(relay.summary(), { activePairs: 0, observedPrincipals: 0, acceptedConnections: 1, disposed: true })
})
