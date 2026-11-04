// Synthetic component authorities below are NEVER a live App/ticket proof.
import test from 'node:test'
import assert from 'node:assert/strict'
import http from 'node:http'
import { createHash } from 'node:crypto'
import net from 'node:net'
import { once } from 'node:events'
import { createAuthenticationCapture } from './qa05-authentication.mjs'
import { createConsumedTicketProbe, maskedFrame } from './qa05-ticket-probe.mjs'
import { installQa05WorldRelay } from './qa05-relay.mjs'

const auth = { type: 'authenticate', protocolVersion: '1.0.0', eventId: 'event', roomId: 'room', mapId: 'map',
  mapRevision: 4, avatarManifestId: 'avatar', avatarManifestVersion: 2, ticket: 'synthetic-test-only-ticket' }
const snapshot = { ...auth, type: 'snapshot', selfUserId: 'a' }
delete snapshot.ticket
const decode = text => JSON.parse(text) // Fixture decoder only; actual caller MUST inject canonical C05 decode.
function capture(guard = () => {}) { return createAuthenticationCapture({ decodeClient: decode, eventId: 'event', guard }) }
function serverFrame(payload, opcode = 1) {
  const body = Buffer.from(payload)
  assert(body.length < 126)
  return Buffer.concat([Buffer.from([0x80 | opcode, body.length]), body])
}

test('one-shot capture waits for actual bytes and matching consumed snapshot, then retains no second release', () => {
  const owner = capture(), wire = maskedFrame(JSON.stringify(auth))
  for (const byte of wire) owner.feed(Buffer.from([byte]))
  assert.throws(() => owner.take('a'), /CONSUMED_AUTHENTICATION_REQUIRED/)
  assert.throws(() => owner.consumed({ ...snapshot, mapRevision: 5 }))
  owner.consumed(snapshot)
  assert.throws(() => owner.take('b'), /CONSUMED_AUTHENTICATION_REQUIRED/)
  assert.deepEqual(owner.take('a'), auth)
  // Subsequent genuine resync/room snapshots do not repopulate the spent ticket.
  owner.consumed({ ...snapshot, roomId: 'second-room' })
  owner.feed(maskedFrame(JSON.stringify({ ...auth, ticket: 'never-capture-again' })))
  assert.throws(() => owner.take('a'), /CONSUMED_AUTHENTICATION_REQUIRED/)
  owner.dispose(); assert.throws(() => owner.feed(wire), /RETIRED/)
})

test('foreign, unmasked, fragmented, invalid UTF8, oversized, decoder failure and expired capture are refused', () => {
  for (const bytes of [maskedFrame(JSON.stringify({ ...auth, eventId: 'foreign' })),
    serverFrame('{}'), Buffer.from([1, 0x80, 0, 0, 0, 0]), maskedFrame(Buffer.from([0xff])),
    Buffer.from([0x81, 0xff, 0, 0, 0, 0, 0, 1, 0, 1])]) {
    const owner = capture()
    assert.throws(() => owner.feed(bytes))
    assert.throws(() => owner.take('a'), /RETIRED/)
  }
  const expired = capture(() => { throw Error('expired') })
  assert.throws(() => expired.feed(maskedFrame(JSON.stringify(auth))), /expired/)
  const wrongType = capture()
  assert.throws(() => wrongType.feed(maskedFrame(JSON.stringify({ ...auth, type: 'input' }))))
})

const binding = { candidate: 'a'.repeat(40), environmentId: 'environment', eventId: 'event', windowId: 'window', expiresAt: new Date(Date.now() + 60000).toISOString() }
const config = { releaseRevision: binding.candidate, environmentId: binding.environmentId, eventId: binding.eventId,
  windowId: binding.windowId, origin: 'http://127.0.0.1:5555', contractHashes: { domain: 'd'.repeat(64), realtime: 'e'.repeat(64) }, members: [{ userId: 'a' }, { userId: 'b' }] }
const backendBinding = { candidate: binding.candidate, environmentId: binding.environmentId, eventId: binding.eventId,
  windowId: binding.windowId, origin: config.origin, domainHash: config.contractHashes.domain, realtimeHash: config.contractHashes.realtime, members: ['a', 'b'] }
const members = [{ userId: 'a', roomId: 'room', ownerEpoch: 'epoch', disconnected: false },
  { userId: 'b', roomId: 'room', ownerEpoch: 'epoch', disconnected: false }]

async function fixture(t, mode = '4401') {
  const peers = new Set(), seen = [], sockets = [], controller = new AbortController()
  const server = http.createServer()
  server.on('connection', socket => { peers.add(socket); socket.on('close', () => peers.delete(socket)) })
  server.on('upgrade', (req, socket) => {
    seen.push({ path: req.url, origin: req.headers.origin })
    const accept = createHash('sha1').update(req.headers['sec-websocket-key'] + '258EAFA5-E914-47DA-95CA-C5AB0DC85B11').digest('base64')
    socket.write('HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: ' + accept + '\r\n\r\n')
    const consumed = capture()
    let first = true
    socket.on('data', bytes => {
      if (!first) { socket.end(); return }
      try {
        consumed.feed(bytes); consumed.consumed(snapshot); sockets.push(consumed.take('a')); first = false
        if (mode === 'hang') return
        if (mode === 'snapshot') { socket.end(serverFrame('{"type":"snapshot"}')); return }
        const body = Buffer.alloc(2); body.writeUInt16BE(Number(mode))
        socket.write(serverFrame(body, 8))
      } catch { socket.destroy() }
    })
  })
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve))
  t.after(async () => { controller.abort(); for (const peer of peers) peer.destroy(); await new Promise(resolve => server.close(resolve)) })
  let inspections = 0, taken = 0
  const options = { binding, config, relay: { takeConsumedAuthentication(principal) { assert.equal(principal, 'a'); taken++; return structuredClone(auth) } },
    ownerControl: { async inspect() { inspections++; return { state: 'OBSERVED', binding: backendBinding,
      memberships: structuredClone(members) } } }, worldOrigin: 'http://127.0.0.1:' + server.address().port,
    decodeServer: decode, guard() {}, signal: controller.signal }
  return { options, seen, sockets, controller, counts: () => ({ inspections, taken }) }
}
const requestId = 'f'.repeat(32)
const invoke = (probe, signal = new AbortController().signal) => probe({ config, requestId, signal })

test('one actual raw upgrade and masked authentication observes 4401/EOF with unchanged owner membership', async t => {
  const f = await fixture(t), probe = createConsumedTicketProbe(f.options)
  const result = await invoke(probe)
  assert.deepEqual(result, { version: 1, binding, requestId, action: 'CONSUMED_TICKET_REPLAY', connectionAttempts: 1,
    authenticateMessages: 1, snapshots: 0, closeCode: 4401, closed: true,
    membershipBefore: { connected: 2, duplicates: 0 }, membershipAfter: { connected: 2, duplicates: 0 } })
  assert.deepEqual(f.seen, [{ path: '/world-ws', origin: config.origin }]); assert.deepEqual(f.sockets, [auth])
  assert.deepEqual(f.counts(), { inspections: 2, taken: 1 })
  assert(!JSON.stringify(result).includes(auth.ticket))
  await assert.rejects(invoke(probe), /ALREADY_ATTEMPTED/)
})

test('4408 authentication timeout and an application snapshot cannot masquerade as consumed-ticket rejection', async t => {
  for (const mode of ['4408', 'snapshot']) {
    const f = await fixture(t, mode), probe = createConsumedTicketProbe(f.options)
    await assert.rejects(invoke(probe), /REPLAY_UNAVAILABLE/)
    assert.equal(f.seen.length, 1)
    await assert.rejects(invoke(probe), /ALREADY_ATTEMPTED/)
  }
})

test('aborting a pending raw probe destroys the socket and never retries or requests another ticket', async t => {
  const f = await fixture(t, 'hang'), probe = createConsumedTicketProbe(f.options), abort = new AbortController()
  const pending = invoke(probe, abort.signal)
  const deadline = Date.now() + 1500
  while (!f.sockets.length) { assert(Date.now() < deadline); await new Promise(resolve => setTimeout(resolve, 5)) }
  abort.abort(); await assert.rejects(pending, /REPLAY_UNAVAILABLE/)
  assert.equal(f.seen.length, 1); assert.equal(f.counts().taken, 1)
  await assert.rejects(invoke(probe), /ALREADY_ATTEMPTED/)
})

test('foreign owner binding and disconnected membership refuse before custody release or network effect', async t => {
  const f = await fixture(t)
  assert.throws(() => createConsumedTicketProbe({ ...f.options, config: { ...config, accessToken: 'never-custody-a-token' } }))
  for (const bad of [{ state: 'OBSERVED', binding: { ...backendBinding, windowId: 'foreign' }, memberships: members },
    { state: 'OBSERVED', binding: backendBinding, memberships: [{ ...members[0], disconnected: true }, members[1]] }]) {
    const probe = createConsumedTicketProbe({ ...f.options, ownerControl: { async inspect() { return bad } } })
    await assert.rejects(invoke(probe))
  }
  assert.deepEqual(f.counts(), { inspections: 0, taken: 0 }); assert.equal(f.seen.length, 0)
})

test('a successful close followed by membership mutation is rejected rather than reported as acceptance', async t => {
  const f = await fixture(t)
  let calls = 0
  const probe = createConsumedTicketProbe({ ...f.options, ownerControl: { async inspect() {
    const value = structuredClone(members); if (++calls === 2) value[0].ownerEpoch = 'changed'
    return { state: 'OBSERVED', binding: backendBinding, memberships: value }
  } } })
  await assert.rejects(invoke(probe), /MEMBERSHIP_CHANGED/)
  assert.equal(f.seen.length, 1)
})

test('actual relay observes original masked client bytes and releases custody only after forwarded snapshot', async t => {
  const controller = new AbortController(), peers = new Set(), received = []
  const upstream = http.createServer(), frontend = http.createServer()
  for (const server of [upstream, frontend]) server.on('connection', socket => {
    peers.add(socket); socket.on('close', () => peers.delete(socket))
  })
  await new Promise(resolve => upstream.listen(0, '127.0.0.1', resolve))
  await new Promise(resolve => frontend.listen(0, '127.0.0.1', resolve))
  const worldOrigin = 'http://127.0.0.1:' + upstream.address().port, frontendOrigin = 'http://127.0.0.1:' + frontend.address().port
  let relay
  t.after(async () => {
    controller.abort(); relay?.close(); for (const peer of peers) peer.destroy()
    await Promise.all([upstream, frontend].map(server => new Promise(resolve => server.close(resolve))))
  })
  const wire = maskedFrame(JSON.stringify(auth)), response = serverFrame(JSON.stringify({ type: 'snapshot', eventId: 'event', selfUserId: 'a' }))
  // This fixture deliberately keeps snapshots small; decoder supplies a complete
  // synthetic context. Actual attachment uses the canonical decoder unchanged.
  upstream.on('upgrade', (req, socket) => {
    assert.equal(req.headers['sec-websocket-extensions'], 'permessage-deflate; client_max_window_bits')
    const accept = createHash('sha1').update(req.headers['sec-websocket-key'] + '258EAFA5-E914-47DA-95CA-C5AB0DC85B11').digest('base64')
    socket.write('HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: ' + accept + '\r\n\r\n')
    const chunks = []
    socket.on('data', bytes => {
      chunks.push(Buffer.from(bytes))
      if (Buffer.concat(chunks).length >= wire.length) { received.push(Buffer.concat(chunks)); socket.write(response) }
    })
  })
  relay = installQa05WorldRelay({ httpServer: frontend, worldOrigin, frontendOrigin, eventId: 'event', principalIds: ['a', 'b'],
    // Transparent fixture sink only; previously qualified frame faults unchanged.
    createFaultController({ send }) { return { acceptServerFrame(bytes) { send(bytes); return { state: 'IDLE' } }, dispose() {} } },
    decodeClient: decode, decodeServer: () => snapshot, guard() {}, signal: controller.signal })
  const client = net.connect(frontend.address().port, '127.0.0.1'), bytes = []
  client.on('data', chunk => bytes.push(Buffer.from(chunk))); client.on('error', () => {})
  await once(client, 'connect', { signal: AbortSignal.timeout(1000) })
  client.write('GET /world-ws HTTP/1.1\r\nHost: ' + new URL(frontendOrigin).host + '\r\nOrigin: ' + frontendOrigin
    + '\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Extensions: permessage-deflate; client_max_window_bits\r\n\r\n')
  const deadline = Date.now() + 1500
  while (!Buffer.concat(bytes).includes(Buffer.from('101 Switching'))) {
    assert(Date.now() < deadline); await new Promise(resolve => setTimeout(resolve, 5))
  }
  assert.throws(() => relay.takeConsumedAuthentication('a'), /UNAVAILABLE/)
  client.write(wire.subarray(0, 3)); client.write(wire.subarray(3))
  while (relay.summary().observedPrincipals !== 1) {
    assert(Date.now() < deadline); await new Promise(resolve => setTimeout(resolve, 5))
  }
  assert.deepEqual(received, [wire]); assert.deepEqual(relay.takeConsumedAuthentication('a'), auth)
  assert.throws(() => relay.takeConsumedAuthentication('a'), /REQUIRED/)
  assert.deepEqual(relay.summary(), { activePairs: 1, observedPrincipals: 1, acceptedConnections: 1, disposed: false })
})
