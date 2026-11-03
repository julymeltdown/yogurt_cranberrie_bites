// TEST ONLY. Minimal WS framing adapted from FE16 browser-smoke.mjs at assignment base.
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { createServer as createHttpServer } from 'node:http'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import { execFileSync } from 'node:child_process'
import { createServer } from 'vite'
import { chromium } from '@playwright/test'
const evidence = process.env.MCR_FE17_EVIDENCE
assert(evidence?.startsWith('/'), 'Set a new absolute MCR_FE17_EVIDENCE directory')
await mkdir(evidence)
const root = fileURLToPath(new URL('../../../../', import.meta.url))
const movement = JSON.parse(await readFile(new URL('../../../../../contracts/fixtures/realtime/movement-heartbeat.json', import.meta.url)))
const initial = movement.messages[1].message
const sourcePaths = ['src/world/network/replication.ts', 'src/world/entities/replicated.ts', 'src/world/network/__tests__/fixtures/replication-browser.ts', 'src/world/network/__tests__/fixtures/replication-browser.html', 'src/world/network/__tests__/replication-browser.mjs', 'src/world/network/WorldTransport.ts', 'src/world/network/websocket.ts', 'src/world/network/transport/engine.ts', 'src/world/network/transport/contract.ts', 'src/world/network/transport/wire-types.ts', 'src/shared/api/http.ts']
const hash = b => createHash('sha256').update(b).digest('hex')
const sourceHashes = Object.fromEntries(await Promise.all(sourcePaths.map(async p => [p, hash(await readFile(root + p))])))
const report = { result: 'FAIL', revision: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(), sourceHashes,
  scope: 'FE17 headless synthetic C05 projection and native WS lifecycle only; no renderer/art/backend acceptance', checks: [], errors: [] }
const reserve = createHttpServer(); await new Promise(r => reserve.listen(0, '127.0.0.1', r)); const port = reserve.address().port; await new Promise(r => reserve.close(r))
const dev = await createServer({ root, logLevel: 'error', server: { host: '127.0.0.1', port, strictPort: true } })
const sockets = new Set(), accepted = [], received = []
function wire(data) {
  const payload = Buffer.from(JSON.stringify(data)), head = payload.length < 126 ? Buffer.from([0x81, payload.length]) : Buffer.from([0x81, 126, payload.length >> 8, payload.length & 255])
  return Buffer.concat([head, payload])
}
let browser
try {
  await dev.listen(); const origin = `http://127.0.0.1:${port}`
  dev.httpServer.on('upgrade', (request, socket, head) => {
    if (request.url !== '/world-ws') return
    assert.equal(request.headers.origin, origin); assert.equal(request.headers['sec-websocket-protocol'], undefined)
    accepted.push(socket); sockets.add(socket); socket.on('error', () => sockets.delete(socket)); socket.on('close', () => sockets.delete(socket))
    const connectionNumber = accepted.length
    const accept = createHash('sha1').update(request.headers['sec-websocket-key'] + '258EAFA5-E914-47DA-95CA-C5AB0DC85B11').digest('base64')
    socket.write(`HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: ${accept}\r\n\r\n`)
    let buffer = head, stream = initial.streamId, streamSeq = 0, tickBase = 0, accessExpiresAt = new Date(Date.now() + 60000).toISOString()
    socket.on('data', chunk => {
      buffer = Buffer.concat([buffer, chunk]); assert(buffer.length < 8192)
      while (buffer.length >= 2) {
        const opcode = buffer[0] & 15, masked = !!(buffer[1] & 128)
        let length = buffer[1] & 127, offset = 2
        if (length === 126) { if (buffer.length < 4) return; length = buffer.readUInt16BE(2); offset = 4 }
        assert(length <= 4096 && length !== 127 && masked)
        if (buffer.length < offset + 4 + length) return
        const mask = buffer.subarray(offset, offset + 4), data = Buffer.from(buffer.subarray(offset + 4, offset + 4 + length)); buffer = buffer.subarray(offset + 4 + length)
        for (let i = 0; i < data.length; i++) data[i] ^= mask[i % 4]
        if (opcode === 8) { socket.end(Buffer.from([0x88, 0])); continue }
        assert.equal(opcode, 1); const message = JSON.parse(data); received.push({ ...message, connectionNumber })
        if (message.type === 'authenticate') { socket.write(wire({ ...initial, accessExpiresAt })); socket.write(wire(movement.messages[4].message)) }
        else if (message.type === 'input') socket.write(wire({ ...movement.messages[3].message, streamId: stream, streamSeq: ++streamSeq, serverTick: tickBase + streamSeq * 2, ackInputSeq: message.seq }))
        else if (message.type === 'resync') {
          stream = '00000000-0000-4000-8000-000000000150'; streamSeq = 0; tickBase = 20
          socket.write(wire({ ...initial, streamId: stream, cause: 'RESYNC', requestId: message.requestId, accessExpiresAt, players: [initial.players[0]], serverTick: 20 }))
        }
      }
    })
  })
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  report.browser = browser.version()
  const page = await browser.newPage({ viewport: { width: 390, height: 844 } }); page.on('pageerror', error => report.errors.push(String(error)))
  await page.goto(origin + '/src/world/network/__tests__/fixtures/replication-browser.html')
  await page.waitForFunction(() => window.__FE17_TEST_ONLY__?.view().entities.length === 3)
  assert.equal(accepted.length, 1); assert(received.some(m => m.type === 'authenticate'))
  await page.waitForFunction(() => window.__FE17_TEST_ONLY__.view().ackInputSeq > 0 && window.__FE17_TEST_ONLY__.view().pendingInputs === 0)
  assert(received.some(m => m.type === 'pong')); report.checks.push('Native socket admission, public roster projection, ping response and input acknowledgement')
  accepted[0].write(wire({ ...movement.messages[3].message, streamSeq: 900 }))
  await page.waitForFunction(() => window.__FE17_TEST_ONLY__.view().entities.length === 1)
  assert.equal(received.filter(m => m.type === 'resync').length, 1); report.checks.push('Gap drives one actual resync; replacement snapshot removes ghosts')
  await page.evaluate(() => { window.__FE17_TEST_ONLY__.intent({ axisX: 1, axisY: 0, buttons: ['interact'] }); window.__FE17_TEST_ONLY__.visible(false) })
  assert.equal(await page.evaluate(() => window.__FE17_TEST_ONLY__.counts().timers), 0)
  assert.equal(await page.evaluate(() => window.__FE17_TEST_ONLY__.view().entities.length), 0)
  await page.evaluate(() => window.__FE17_TEST_ONLY__.visible(true))
  await page.waitForFunction(() => window.__FE17_TEST_ONLY__.counts().tickets === 2 && window.__FE17_TEST_ONLY__.view().entities.length === 3)
  assert.equal(accepted.length, 2)
  await page.waitForFunction(() => window.__FE17_TEST_ONLY__.view().ackInputSeq > 0)
  const freshInputs = received.filter(m => m.type === 'input' && m.connectionNumber === 2)
  assert(freshInputs.length > 0); assert.equal(freshInputs[0].seq, 1)
  assert(freshInputs.every(m => m.axisX === 0 && m.axisY === 0 && m.buttons.length === 0))
  const admissions = received.filter(m => m.type === 'authenticate')
  assert.notEqual(admissions[0].ticket, admissions[1].ticket)
  report.checks.push('Visibility boundary gets one fresh ticket/connection, with no old interaction or movement replay')
  await page.evaluate(() => window.__FE17_TEST_ONLY__.replaceOwner())
  await page.waitForFunction(() => window.__FE17_TEST_ONLY__.counts().timers === 0 && window.__FE17_TEST_ONLY__.view().entities.length === 0)
  await page.evaluate(() => window.__FE17_TEST_ONLY__.dispose())
  const before = await page.evaluate(() => window.__FE17_TEST_ONLY__.counts())
  await page.waitForTimeout(150); assert.deepEqual(await page.evaluate(() => window.__FE17_TEST_ONLY__.counts()), before)
  assert.equal(accepted.length, 2); assert.deepEqual(report.errors, []); report.checks.push('Canonical owner cancellation and disposal leave no timers, retries or later sink commits')
  report.result = 'PASS'
} catch (error) { report.failure = { name: error.name, message: error.message }; process.exitCode = 1 }
finally { await browser?.close(); for (const socket of sockets) socket.destroy(); await dev.close(); report.cleanup = 'PASS'; await writeFile(evidence + '/report.json', JSON.stringify(report, null, 2) + '\n') }
console.log(JSON.stringify({ result: report.result, checks: report.checks.length, failure: report.failure }))
