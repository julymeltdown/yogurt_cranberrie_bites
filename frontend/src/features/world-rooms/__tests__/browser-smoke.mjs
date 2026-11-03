// TEST ONLY. Minimal local WebSocket framing adapted from FE17 at assignment base366cf8b.
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { createServer as createNetServer } from 'node:net'
import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises'
import { resolve, relative } from 'node:path'
import { execFileSync } from 'node:child_process'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
const root = resolve(import.meta.dirname, '../../../../..'), frontend = resolve(root, 'frontend'), evidence = process.env.MCR_FE18_EVIDENCE
assert(evidence?.startsWith('/'), 'Set a new absolute MCR_FE18_EVIDENCE directory'); await mkdir(evidence, { mode: 0o700 })
const hash = value => createHash('sha256').update(value).digest('hex'), sourceHashes = {}
async function collect(dir) { for (const entry of await readdir(dir, { withFileTypes: true })) { const p = resolve(dir, entry.name); if (entry.isDirectory()) await collect(p); else if (/\.(ts|tsx|mjs|html|css|json)$/.test(p)) sourceHashes[relative(root, p)] = hash(await readFile(p)) } }
await collect(resolve(frontend, 'src'))
const contractPaths = ['contracts/realtime/v1/operations.json', 'contracts/realtime/v1/world.schema.json', 'contracts/realtime/v1/policy.json', 'contracts/fixtures/realtime/rooms.json', 'contracts/fixtures/realtime/movement-heartbeat.json', 'contracts/fixtures/realtime/room-switch.json']
const report = { result: 'FAIL', scope: 'FE18 deterministic room HTTP fixtures and actual FE16 native WebSocket/FE17 in FE07 shell; no backend/physical location/art/invitation/whole-task acceptance', revision: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(), sourceHashes,
  contracts: Object.fromEntries(await Promise.all(contractPaths.map(async p => [p, hash(await readFile(resolve(root, p)))]))), widths: [], cleanup: 'NOT_RUN' }
const movement = JSON.parse(await readFile(resolve(root, 'contracts/fixtures/realtime/movement-heartbeat.json'))), initial = movement.messages[1].message
const uid = n => `00000000-0000-4000-8000-${String(n).padStart(12, '0')}`
const reservation = createNetServer(); await new Promise(done => reservation.listen(0, '127.0.0.1', done)); const port = reservation.address().port; await new Promise(done => reservation.close(done))
const dev = await createServer({ root: frontend, logLevel: 'error', server: { host: '127.0.0.1', port, strictPort: true } })
const sockets = new Set(), accepted = [], received = []
let connection = null, serial = 600, peak = 0, browser, stage = 'start'
function wire(value) { const p = Buffer.from(JSON.stringify(value)), h = p.length < 126 ? Buffer.from([0x81, p.length]) : Buffer.from([0x81, 126, p.length >> 8, p.length & 255]); return Buffer.concat([h, p]) }
function send(value) { connection.socket.write(wire(value)) }
function replacement(cause, requestId, roomId, players) {
  connection.stream = uid(++serial); connection.seq = 0; connection.ack = 0; connection.tickBase += 1000
  send({ ...initial, streamId: connection.stream, roomEpoch: connection.epoch, roomId, cause, requestId, players, serverTick: connection.tickBase, ackInputSeq: connection.ack, accessExpiresAt: connection.expiry })
}
function delta(upserts, removes, seq = connection.seq + 1) { connection.seq = seq; send({ protocolVersion: '1.0.0', type: 'delta', streamId: connection.stream, roomEpoch: connection.epoch, streamSeq: seq, serverTick: connection.tickBase + seq * 2, ackInputSeq: connection.ack, upserts, removes }) }
try {
  await dev.listen(); const origin = `http://127.0.0.1:${port}`
  dev.httpServer.on('upgrade', (request, socket, head) => {
    if (request.url !== '/world-ws') return
    assert.equal(request.headers.origin, origin); assert(!request.headers['sec-websocket-protocol']); assert(!request.url.includes('?'))
    sockets.add(socket); accepted.push(socket); peak = Math.max(peak, sockets.size); socket.on('error', () => sockets.delete(socket)); socket.on('close', () => sockets.delete(socket))
    const accept = createHash('sha1').update(request.headers['sec-websocket-key'] + '258EAFA5-E914-47DA-95CA-C5AB0DC85B11').digest('base64')
    socket.write(`HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: ${accept}\r\n\r\n`)
    let buffer = head
    const c = { socket, stream: uid(++serial), epoch: uid(++serial), seq: 0, ack: 0, tickBase: 0, room: initial.roomId, expiry: new Date(Date.now() + 120000).toISOString(), pendingSwitch: null, pendingResync: null }
    connection = c
    socket.on('data', chunk => {
      buffer = Buffer.concat([buffer, chunk]); assert(buffer.length < 8192)
      while (buffer.length >= 2) {
        const opcode = buffer[0] & 15, masked = !!(buffer[1] & 128); let length = buffer[1] & 127, offset = 2
        if (length === 126) { if (buffer.length < 4) return; length = buffer.readUInt16BE(2); offset = 4 }
        assert(length <= 4096 && length !== 127 && masked); if (buffer.length < offset + 4 + length) return
        const mask = buffer.subarray(offset, offset + 4), data = Buffer.from(buffer.subarray(offset + 4, offset + 4 + length)); buffer = buffer.subarray(offset + 4 + length)
        for (let i = 0; i < data.length; i++) data[i] ^= mask[i % 4]
        if (opcode === 8) { sockets.delete(socket); socket.end(Buffer.from([0x88, 0])); continue }
        assert.equal(opcode, 1); const message = JSON.parse(data); const { ticket: _secret, ...safe } = message; received.push(safe)
        if (message.type === 'authenticate') { c.room = message.roomId; socket.write(wire({ ...initial, roomId: c.room, streamId: c.stream, roomEpoch: c.epoch, accessExpiresAt: c.expiry })) }
        else if (message.type === 'switchRoom') c.pendingSwitch = message
        else if (message.type === 'resync') c.pendingResync = message
        else if (message.type === 'input') { c.ack = message.seq; socket.write(wire({ protocolVersion: '1.0.0', type: 'delta', streamId: c.stream, roomEpoch: c.epoch, streamSeq: ++c.seq, serverTick: c.tickBase + c.seq * 2, ackInputSeq: c.ack, upserts: [], removes: [] })) }
      }
    })
  })
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true }); report.browser = browser.version()
  for (const [width, height] of [[390, 844], [1280, 900]]) {
    stage = `start-${width}`; peak = 0; const startConnections = accepted.length
    const context = await browser.newContext({ viewport: { width, height }, reducedMotion: 'reduce' }), page = await context.newPage(), errors = []
    page.on('pageerror', e => errors.push(e.message))
    const state = () => page.evaluate(() => window.__FE18__.state()), feature = async () => (await state()).feature
    const shot = async (name, target) => { await page.locator('.shell-panel-body').evaluate(el => { el.scrollTop = 0 }); if (target) await target.scrollIntoViewIfNeeded(); await page.screenshot({ path: resolve(evidence, `${width}-${name}.png`), fullPage: true }) }
    const switchButton = () => page.getByRole('button', { name: 'Switch to SECOND ROOM', exact: true })
    await page.goto(`${origin}/src/features/world-rooms/__tests__/fixtures/preview.html`)
    await expect.poll(async () => (await feature()).phase).toBe('live'); await expect(switchButton()).toBeEnabled()
    assert.deepEqual((await state()).roomRequests[0], { method: 'GET', path: `/api/concert/events/${initial.eventId}/rooms`, cache: 'no-store' });
    await shot('rooms'); await shot('room-controls', switchButton()); await page.evaluate(() => window.__FE18__.mode('full')); await expect(switchButton()).toBeDisabled(); await expect(page.getByText('This room is full.', { exact: true })).toBeVisible()
    await page.evaluate(() => window.__FE18__.mode('unavailable')); await expect(switchButton()).toBeDisabled(); await page.evaluate(() => window.__FE18__.mode('ok')); await page.evaluate(() => window.__FE18__.mode('outage')); await expect(page.getByRole('alert')).toContainText('temporarily unavailable'); await shot('rooms-error', page.getByRole('alert')); await page.evaluate(() => window.__FE18__.mode('ok')); await expect(switchButton()).toBeEnabled()
    stage = `nearby-${width}`
    await page.getByRole('link', { name: 'Nearby in world', exact: true }).first().focus(); await page.keyboard.press('Enter'); await expect(page.locator('#panel-title')).toBeFocused()
    await expect(page.getByRole('link', { name: 'Ghost', exact: true })).toBeVisible(); await expect(page.getByRole('link', { name: 'Jet', exact: true })).toBeVisible(); await shot('nearby'); await shot('nearby-people', page.getByRole('region', { name: 'Other people in this world' }))
    delta([], [uid(2)]); await expect(page.getByRole('link', { name: 'Ghost', exact: true })).toHaveCount(0); await expect(page.locator('#authorized-world-sink')).not.toContainText('Ghost')
    connection.epoch = uid(++serial); replacement('EPOCH_CHANGED', null, connection.room, [initial.players[0]])
    await expect(page.getByRole('link', { name: 'Jet', exact: true })).toHaveCount(0); await expect(page.locator('#authorized-world-sink')).not.toContainText('Jet'); await expect(page.getByRole('region', { name: 'You in this world' })).toBeVisible(); await shot('nearby-empty')
    await page.goBack(); await expect(page.getByRole('link', { name: 'Nearby in world', exact: true }).first()).toBeFocused()
    stage = `switch-${width}`
    const beforeSwitch = received.filter(r => r.type === 'switchRoom').length
    await switchButton().evaluate(button => { button.click(); button.click() }); await expect.poll(() => received.filter(r => r.type === 'switchRoom').length).toBe(beforeSwitch + 1)
    assert.equal((await feature()).currentRoomId, initial.roomId); assert.equal((await feature()).switch.phase, 'switching'); assert.equal((await state()).replication.baseline.roomId, initial.roomId)
    send({ protocolVersion: '1.0.0', type: 'switchRejected', requestId: connection.pendingSwitch.requestId, streamId: connection.stream, reason: 'ROOM_FULL' }); connection.pendingSwitch = null
    await expect(page.getByRole('alert')).toContainText('confirmed room remains unchanged'); assert.equal((await feature()).currentRoomId, initial.roomId); await shot('rejected', page.getByRole('alert'))
    await switchButton().click(); await expect.poll(() => !!connection.pendingSwitch).toBe(true)
    connection.room = connection.pendingSwitch.roomId; connection.epoch = uid(++serial); connection.ack = 0; replacement('ROOM_SWITCH', connection.pendingSwitch.requestId, connection.room, [initial.players[0]]); connection.pendingSwitch = null
    await expect(page.getByText('Room switch confirmed.', { exact: true })).toBeVisible(); assert.equal((await feature()).currentRoomId, uid(12)); await shot('admitted', page.getByText('Room switch confirmed.', { exact: true }))
    await page.getByRole('link', { name: 'Nearby in world', exact: true }).first().click(); delta([initial.players[1]], []); await expect(page.getByRole('link', { name: 'Ghost' })).toBeVisible()
    stage = `resync-${width}`
    delta([], [], connection.seq + 10); await expect.poll(async () => (await feature()).phase).toBe('resync'); await expect(page.getByRole('link', { name: 'Ghost' })).toHaveCount(0); await expect(page.locator('#authorized-world-sink')).not.toContainText('Ghost'); await expect.poll(() => !!connection.pendingResync).toBe(true)
    connection.ack = 0; replacement('RESYNC', connection.pendingResync.requestId, connection.room, [initial.players[0]]); connection.pendingResync = null
    await expect.poll(async () => (await feature()).phase).toBe('live')
    const mounts = (await state()).mounts; assert.equal(mounts, 1); assert.equal(accepted.length - startConnections, 1)
    stage = `reconnect-${width}`
    await page.evaluate(() => window.__FE18__.pause()); await expect.poll(async () => (await feature()).phase).toBe('suspended'); assert.deepEqual((await feature()).nearby, [])
    await page.evaluate(() => window.__FE18__.resume()); await expect.poll(async () => (await feature()).phase).toBe('live'); assert.equal(accepted.length - startConnections, 2); assert.equal((await state()).mounts, mounts); assert.equal(peak, 1)
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'page overflow'); assert(await page.locator('.world-access').evaluate(el => el.scrollWidth <= el.clientWidth + 1), 'feature overflow')
    for (const el of await page.locator('.world-access button:visible,.world-access a:visible').all()) { const box = await el.boundingBox(); assert(box && box.width >= 44 && box.height >= 44, 'small touch target') }
    assert.equal(await page.locator('.world-access img,.world-access canvas').count(), 0)
    stage = `privacy-${width}`
    assert.deepEqual(await page.evaluate(() => window.__FE18__.revoke()), { nearby: 0, sinkPeople: 0, timers: 0 }); await expect.poll(async () => (await feature()).nearby.length).toBe(0); await expect(page.getByRole('link', { name: 'Ghost' })).toHaveCount(0); await expect.poll(() => page.locator('#authorized-world-sink').count()).toBe(0); await expect.poll(async () => (await state()).timers).toBe(0)
    await page.evaluate(() => window.__FE18__.dispose()); await expect.poll(() => sockets.size).toBe(0); assert.equal((await state()).disposals, 1); assert.deepEqual(errors, [])
    report.widths.push({ width, height, checks: ['core room read', '503/retry', 'full/unavailable', 'keyboard/Back focus', 'authorized self/other DOM', 'delta removal in sink+DOM', 'epoch replacement', 'duplicate switch suppression', 'sent is not admitted', 'rejection preserves membership', 'replacement admission', 'gap/resync clears roster', 'fresh reconnect/single socket', 'no overflow', '44px controls', 'privacy eviction', 'zero timers/complete disposal'], pageErrors: errors }); await context.close()
  }
  report.result = 'PASS'; report.cleanup = 'PASS'
} catch (error) { report.failure = { stage, message: error instanceof Error ? error.message : 'unknown' }; throw error }
finally { await browser?.close(); for (const socket of sockets) socket.destroy(); await dev.close(); await writeFile(resolve(evidence, 'report.json'), JSON.stringify(report, null, 2) + '\n'); console.log(JSON.stringify({ result: report.result, stage, evidence, widths: report.widths, cleanup: report.cleanup })) }
