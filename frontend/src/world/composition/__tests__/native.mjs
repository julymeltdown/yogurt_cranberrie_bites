// TEST ONLY. WS framing adapted from FE17's reviewed browser fixture at assignment base.
import assert from 'node:assert/strict'
import { createHash, randomUUID } from 'node:crypto'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import { resolve } from 'node:path'
import { execFileSync } from 'node:child_process'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'

const root = fileURLToPath(new URL('../../../../', import.meta.url))
const evidence = process.env.MCR_WORLD_COMPOSITION_EVIDENCE
assert(evidence?.startsWith('/') && evidence.startsWith(resolve(root, 'src/world/composition/__tests__/evidence') + '/'), 'Use a fresh absolute evidence subdirectory inside composition tests')
await mkdir(evidence, { recursive: true })
const paths = execFileSync('rg', ['--files', 'frontend/src', 'contracts/core', 'contracts/realtime', 'contracts/assets', 'contracts/fixtures/core', 'contracts/fixtures/realtime', 'contracts/fixtures/assets'], { cwd: resolve(root, '..'), encoding: 'utf8' }).trim().split('\n')
  .filter(path => !path.includes('/evidence/') && /\.(?:tsx?|mjs|html|css|json)$/.test(path)).sort()
const hashes = Object.fromEntries(await Promise.all(paths.map(async path => [path, createHash('sha256').update(await readFile(resolve(root, '..', path))).digest('hex')])))
const report = { result: 'FAIL', revision: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(), node: process.version,
  scope: 'Actual Phaser 3.90.0, canonical Auth HTTP and native WebSocket with TEST ONLY C03/C05/C06 HTTP/WS/terrain; FE12 art absent; no real backend/physics/IN01 acceptance', hashes, widths: [], errors: [] }
const tickets = new Map(), connections = new Set(), live = new Set(), received = [], serverErrors = []
let issued = 0, peakSockets = 0, data, makeSnapshot, makeTicket, denyNextSwitch = false
const auth = JSON.parse(await readFile(resolve(root, '../contracts/fixtures/core/auth-success.json')))
const caps = JSON.parse(await readFile(resolve(root, '../contracts/fixtures/core/auth-capabilities-disabled.json')))
caps.providers[0] = { ...caps.providers[0], enabled: true, disabled_reason: null }
const reply = (res, status, value) => { res.writeHead(status, { 'Content-Type': 'application/json', 'Cache-Control': 'no-store' }); res.end(value === undefined ? '' : JSON.stringify(value)) }
const dev = await createServer({ root, logLevel: 'error', server: { host: '127.0.0.1', port: 0, strictPort: false }, plugins: [{
  name: 'test-only-world-http', configureServer(server) {
    server.middlewares.use(async (req, res, next) => {
      try {
        if (!req.url?.startsWith('/api/') && !req.url?.startsWith('/world/fe15/')) return next()
        if (req.url.startsWith('/world/fe15/')) {
          const png = req.url.endsWith('.png'); res.writeHead(200, { 'Content-Type': png ? 'image/png' : 'application/json' }); res.end(Buffer.from(png ? data.png : data.tiledBytes)); return
        }
        if (req.url === '/api/auth/capabilities') return reply(res, 200, caps)
        if (req.url === '/api/auth/login') { assert.equal(req.method, 'POST'); assert.equal(req.headers['x-mcr-intent'], 'browser'); return reply(res, 200, auth) }
        if (req.url === '/api/auth/logout') { assert.equal(req.method, 'POST'); return reply(res, 204) }
        assert.equal(req.headers.authorization, `Bearer ${auth.accessToken}`)
        if (req.url.endsWith('/bootstrap')) return reply(res, 200, data.bootstrap)
        if (req.url.endsWith('/rooms')) return reply(res, 200, data.rooms)
        if (req.url.endsWith('/world-tickets')) {
          assert.equal(req.method, 'POST'); assert.equal(req.headers['x-mcr-intent'], 'browser'); assert.equal(req.headers['idempotency-key'], undefined)
          const chunks = []; for await (const chunk of req) chunks.push(chunk)
          const body = JSON.parse(Buffer.concat(chunks).toString()); assert(data.rooms.items.some(room => room.room_id === body.room_id))
          assert.equal(body.map_id, data.map.map_id); assert.equal(body.map_revision, data.map.map_revision)
          assert.equal(body.avatar_manifest_id, data.pin.avatar.manifest_id); assert.equal(body.avatar_manifest_version, data.pin.avatar.manifest_version)
          if (denyNextSwitch && body.room_id !== data.rooms.default_room_id) { denyNextSwitch = false; return reply(res, 409, { code: 'TEST', message: 'Test-only full target.', details: { reason: 'RESOURCE_UNAVAILABLE' } }) }
          const ticket = makeTicket(body, ++issued); tickets.set(ticket.ticket, { body, used: false }); return reply(res, 200, ticket)
        }
        return reply(res, 404, { code: 'TEST', message: '', details: { reason: 'RESOURCE_UNAVAILABLE' } })
      } catch (error) { serverErrors.push(String(error)); reply(res, 500, { code: 'TEST', message: '', details: { reason: 'INTERNAL_ERROR' } }) }
    })
  },
}] })
const fixture = await dev.ssrLoadModule('/src/world/composition/__tests__/fixtures/data.ts')
data = await fixture.testData(); makeSnapshot = fixture.snapshot; makeTicket = fixture.ticket
const wire = value => {
  const payload = Buffer.from(JSON.stringify(value)), head = payload.length < 126 ? Buffer.from([0x81, payload.length]) : Buffer.from([0x81, 126, payload.length >> 8, payload.length & 255])
  return Buffer.concat([head, payload])
}
let browser
try {
  await dev.listen(); const origin = `http://127.0.0.1:${dev.httpServer.address().port}`
  dev.httpServer.on('upgrade', (request, socket, head) => {
    if (request.url !== '/world-ws') return
    assert.equal(request.headers.origin, origin); assert.equal(request.headers['sec-websocket-protocol'], undefined)
    const accept = createHash('sha1').update(request.headers['sec-websocket-key'] + '258EAFA5-E914-47DA-95CA-C5AB0DC85B11').digest('base64')
    socket.write(`HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: ${accept}\r\n\r\n`)
    const connection = { socket, room: null, stream: randomUUID(), seq: 0, tick: 0, expires: new Date(Date.now() + 60000).toISOString(), players: null }
    connections.add(connection); const remove = () => { live.delete(connection); connections.delete(connection) }
    socket.on('close', remove); socket.on('error', remove)
    let buffer = head
    const admit = (message, cause) => {
      const lease = tickets.get(message.ticket); assert(lease && !lease.used); lease.used = true
      assert.equal(lease.body.room_id, message.roomId); assert.equal(message.eventId, data.bootstrap.event.event_id)
      assert.equal(lease.body.map_id, message.mapId); assert.equal(lease.body.map_revision, message.mapRevision)
      assert.equal(lease.body.avatar_manifest_id, message.avatarManifestId); assert.equal(lease.body.avatar_manifest_version, message.avatarManifestVersion)
      connection.room = message.roomId; connection.stream = randomUUID(); connection.seq = connection.tick = 0
      const frame = makeSnapshot(connection.room, connection.stream, { accessExpiresAt: connection.expires, cause, requestId: cause === 'INITIAL' ? null : message.requestId })
      connection.players = frame.players; live.add(connection); peakSockets = Math.max(peakSockets, live.size)
      socket.write(wire(frame)); socket.write(wire({ protocolVersion: '1.0.0', type: 'ping', heartbeatId: randomUUID() }))
    }
    connection.gap = () => socket.write(wire({ protocolVersion: '1.0.0', type: 'delta', streamId: connection.stream,
      roomEpoch: makeSnapshot(connection.room).roomEpoch, streamSeq: 999, serverTick: 999, ackInputSeq: 0, upserts: [], removes: [] }))
    socket.on('data', chunk => {
      try {
        buffer = Buffer.concat([buffer, chunk]); assert(buffer.length < 16384)
        while (buffer.length >= 2) {
          const opcode = buffer[0] & 15, masked = !!(buffer[1] & 128); let length = buffer[1] & 127, offset = 2
          if (length === 126) { if (buffer.length < 4) return; length = buffer.readUInt16BE(2); offset = 4 }
          assert(length <= 4096 && length !== 127 && masked)
          if (buffer.length < offset + 4 + length) return
          const mask = buffer.subarray(offset, offset + 4), payload = Buffer.from(buffer.subarray(offset + 4, offset + 4 + length)); buffer = buffer.subarray(offset + 4 + length)
          for (let i = 0; i < payload.length; i++) payload[i] ^= mask[i % 4]
          if (opcode === 8) { remove(); socket.end(Buffer.from([0x88, 0])); continue }
          assert.equal(opcode, 1); const message = JSON.parse(payload); received.push({ type: message.type, axisX: message.axisX, seq: message.seq })
          if (message.type === 'authenticate') admit(message, 'INITIAL')
          else if (message.type === 'switchRoom') admit(message, 'ROOM_SWITCH')
          else if (message.type === 'resync') {
            connection.stream = randomUUID(); connection.seq = 0; connection.tick++
            const frame = makeSnapshot(connection.room, connection.stream, { players: connection.players.slice(0, 1), accessExpiresAt: connection.expires,
              serverTick: connection.tick, cause: 'RESYNC', requestId: message.requestId })
            connection.players = frame.players; socket.write(wire(frame))
          } else if (message.type === 'input') socket.write(wire({ protocolVersion: '1.0.0', type: 'delta', streamId: connection.stream,
            roomEpoch: makeSnapshot(connection.room).roomEpoch, streamSeq: ++connection.seq, serverTick: ++connection.tick, ackInputSeq: message.seq,
            upserts: connection.players.slice(0, 1), removes: [] }))
        }
      } catch (error) { serverErrors.push(String(error)); remove(); socket.destroy() }
    })
  })
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  report.browser = browser.version()
  for (const [width, height] of [[390, 844], [1280, 900]]) {
    peakSockets = 0
    const context = await browser.newContext({ viewport: { width, height }, reducedMotion: 'reduce' }), page = await context.newPage()
    page.on('pageerror', error => report.errors.push(error.message))
    await page.goto(origin + '/src/world/composition/__tests__/fixtures/native.html')
    const state = () => page.evaluate(() => window.__WORLD_COMPOSITION_TEST_ONLY__.state())
    await page.waitForFunction(() => window.__WORLD_COMPOSITION_TEST_ONLY__?.state().replication?.phase === 'live')
    const before = await state(), canvas = await page.locator('canvas').elementHandle()
    assert.equal(before.games, 1); assert.equal(before.canvases, 1); assert.equal(before.peakGames, 1); assert.equal(live.size, 1)
    await page.screenshot({ path: `${evidence}/world-${width}.png` })
    await page.locator('canvas').focus(); await page.keyboard.down('ArrowRight')
    await expect.poll(() => received.filter(message => message.type === 'input' && message.axisX > 0).length).toBeGreaterThan(0)
    await page.keyboard.up('ArrowRight')
    assert.equal((await state()).replication.spatialPrediction, 'unavailable')
    await page.getByRole('link', { name: 'Gifts', exact: true }).click()
    await page.getByRole('heading', { name: 'Gifts are unavailable', exact: true }).waitFor()
    await page.getByRole('link', { name: 'Meet', exact: true }).click()
    await page.getByRole('link', { name: 'World', exact: true }).click()
    assert(await canvas.evaluate(element => element.isConnected)); assert.equal((await state()).mounts, before.mounts)
    assert.equal(live.size, 1)
    await page.getByRole('link', { name: 'Gifts', exact: true }).click(); await page.getByRole('button', { name: 'Expand panel', exact: true }).click()
    await page.waitForFunction(() => window.__WORLD_COMPOSITION_TEST_ONLY__.state().replication?.phase === 'suspended')
    await expect.poll(() => live.size).toBe(0)
    await page.getByRole('button', { name: 'Reduce panel', exact: true }).click()
    await page.waitForFunction(() => window.__WORLD_COMPOSITION_TEST_ONLY__.state().replication?.phase === 'live')
    assert.equal((await state()).mounts, before.mounts)
    denyNextSwitch = true
    assert.equal(await page.evaluate(room => window.__WORLD_COMPOSITION_TEST_ONLY__.switchRoom(room), data.rooms.items[1].room_id), false)
    assert.equal((await state()).replication.phase, 'live'); assert.equal((await state()).replication.baseline.roomId, data.rooms.default_room_id)
    assert.equal(live.size, 1); assert.equal((await state()).mounts, before.mounts)
    await page.getByRole('link', { name: 'World', exact: true }).click()
    assert.equal((await state()).replication.entities.length, 3)
    ;[...live][0].gap()
    await page.waitForFunction(() => window.__WORLD_COMPOSITION_TEST_ONLY__.state().replication?.entities.length === 1)
    ;[...live][0].socket.destroy()
    await page.waitForFunction(() => window.__WORLD_COMPOSITION_TEST_ONLY__.state().replication?.phase === 'closed')
    assert.equal((await state()).replication.entities.length, 0)
    await page.getByRole('button', { name: 'Reconnect world', exact: true }).click()
    await page.waitForFunction(() => window.__WORLD_COMPOSITION_TEST_ONLY__.state().replication?.phase === 'live')
    assert.equal((await state()).mounts, before.mounts)
    await page.evaluate(room => window.__WORLD_COMPOSITION_TEST_ONLY__.switchRoom(room), data.rooms.items[1].room_id)
    await page.waitForFunction(room => window.__WORLD_COMPOSITION_TEST_ONLY__.state().replication?.baseline?.roomId === room, data.rooms.items[1].room_id)
    assert.equal((await state()).mounts, before.mounts); assert.equal(live.size, 1)
    const revoked = await page.evaluate(() => window.__WORLD_COMPOSITION_TEST_ONLY__.revoke())
    assert.equal(revoked.games, 0); assert.equal(revoked.canvases, 0); assert.equal(revoked.replication, null)
    await expect.poll(() => live.size).toBe(0)
    await page.evaluate(() => window.__WORLD_COMPOSITION_TEST_ONLY__.logout())
    assert(await page.evaluate(() => window.__WORLD_COMPOSITION_TEST_ONLY__.login()))
    await page.waitForFunction(() => window.__WORLD_COMPOSITION_TEST_ONLY__.state().replication?.phase === 'live')
    await page.screenshot({ path: `${evidence}/reentry-${width}.png` })
    const active = await state(); assert.equal(active.peakGames, 1); assert.equal(active.games, 1); assert.equal(active.canvases, 1)
    assert(active.requests.filter(r => r.path.endsWith('/world-tickets')).every(r => r.method === 'POST' && r.cache === 'no-store' && r.intent === 'browser' && r.hasBearer))
    assert.equal(active.world.renderer, 'unavailable'); assert.equal(peakSockets, 1)
    const final = await page.evaluate(() => window.__WORLD_COMPOSITION_TEST_ONLY__.dispose())
    assert.equal(final.games, 0); assert.equal(final.canvases, 0); assert.equal(final.replication, null); assert.equal(final.mounts, final.disposals)
    await expect.poll(() => live.size).toBe(0)
    report.widths.push({ width, height, result: 'PASS', active, final, peakAuthorizedSockets: peakSockets,
      checks: ['actual Phaser and hash-verified HTTP terrain', 'canonical HTTP login/bootstrap/rooms/ticket', 'actual FE15 input to native WS/ack', 'one game/socket across panels',
        'full panel hide evicts roster/disconnects; reveal uses fresh ticket', 'gap resync removes ghosts', 'disconnect/reconnect keeps game', 'room switch keeps game/socket',
        'full target ticket rejection preserves old membership', 'synchronous privacy teardown', 'logout/login re-entry peak one', 'final canvas/game/socket withdrawal', 'no substitute avatar art'] })
    await context.close()
  }
  assert.deepEqual(serverErrors, []); assert.deepEqual(report.errors, []); report.result = 'PASS'
  console.log(`PASS: actual Phaser/canonical HTTP/native WS composition at390/1280; ${Object.keys(hashes).length} source/contract hashes`)
} catch (error) { report.errors.push(String(error)); throw error }
finally {
  await writeFile(`${evidence}/report.json`, JSON.stringify(report, null, 2) + '\n')
  await browser?.close(); for (const connection of connections) connection.socket.destroy(); await dev.close()
}
