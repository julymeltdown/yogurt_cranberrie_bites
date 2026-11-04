// One real raw WebSocket attempt with an already consumed App authentication.
// Never acquires a ticket, retries, creates a member, or serializes credentials.
import assert from 'node:assert/strict'
import http from 'node:http'
import { createHash, randomBytes } from 'node:crypto'
import { isDeepStrictEqual as equal } from 'node:util'
import { createServerFrameReader } from './qa05-relay.mjs'

export function maskedFrame(payload, opcode = 1) {
  const bytes = Buffer.from(payload), mask = randomBytes(4)
  assert(bytes.length <= 65536)
  const prefix = bytes.length < 126 ? Buffer.from([0x80 | opcode, 0x80 | bytes.length]) : Buffer.alloc(bytes.length <= 65535 ? 4 : 10)
  if (prefix.length > 2) {
    prefix[0] = 0x80 | opcode; prefix[1] = 0x80 | (prefix.length === 4 ? 126 : 127)
    if (prefix.length === 4) prefix.writeUInt16BE(bytes.length, 2)
    else prefix.writeBigUInt64BE(BigInt(bytes.length), 2)
  }
  const masked = Buffer.alloc(bytes.length)
  for (let n = 0; n < bytes.length; n++) masked[n] = bytes[n] ^ mask[n % 4]
  return Buffer.concat([prefix, mask, masked])
}

export function createConsumedTicketProbe({ binding, config, relay, ownerControl, worldOrigin,
  decodeServer, guard, signal, now = Date.now }) {
  const keys = (value, expected) => equal(Object.keys(value).sort(), [...expected].sort())
  assert(keys(binding, ['candidate', 'environmentId', 'eventId', 'windowId', 'expiresAt']))
  assert(keys(config, ['releaseRevision', 'environmentId', 'eventId', 'windowId', 'origin', 'contractHashes', 'members']))
  assert(keys(config.contractHashes, ['domain', 'realtime']) && config.members.every(member => keys(member, ['userId'])))
  config = structuredClone(config)
  const origin = new URL(worldOrigin), frontend = new URL(config.origin)
  for (const url of [origin, frontend]) assert(url.protocol === 'http:' && ['127.0.0.1', 'localhost', '[::1]'].includes(url.hostname)
    && !url.username && !url.password && url.pathname === '/' && !url.search && !url.hash)
  assert(signal instanceof AbortSignal && !signal.aborted)
  const fixed = Object.freeze({ ...binding }), principals = config.members.map(member => member.userId)
  assert(principals.length === 2 && new Set(principals).size === 2)
  const backendBinding = { candidate: fixed.candidate, environmentId: fixed.environmentId, eventId: fixed.eventId,
    windowId: fixed.windowId, origin: config.origin, domainHash: config.contractHashes.domain,
    realtimeHash: config.contractHashes.realtime, members: principals }
  for (const [key, value] of Object.entries({ releaseRevision: fixed.candidate, environmentId: fixed.environmentId,
    eventId: fixed.eventId, windowId: fixed.windowId })) assert.equal(config[key], value)
  let attempted = false
  let lastNow = -Infinity
  function live(callSignal) {
    const at = now(); assert(Number.isFinite(at) && at >= lastNow); lastNow = at
    assert(callSignal instanceof AbortSignal && !signal.aborted && !callSignal.aborted && Date.parse(fixed.expiresAt) - at > 3000)
    const result = guard(); assert(result !== false && !result?.then)
  }
  async function memberships(callSignal) {
    live(callSignal)
    const value = await ownerControl.inspect(config)
    live(callSignal)
    assert(value.state === 'OBSERVED' && equal(value.binding, backendBinding))
    assert(Array.isArray(value.memberships) && value.memberships.length === 2)
    assert(new Set(value.memberships.map(member => member.userId)).size === 2)
    assert(value.memberships.every(member => principals.includes(member.userId) && member.disconnected === false))
    assert(value.memberships[0].roomId === value.memberships[1].roomId)
    return structuredClone(value.memberships).sort((a, b) => a.userId.localeCompare(b.userId))
  }
  return async function probeConsumedTicket({ config: supplied, requestId, signal: callSignal }) {
    assert(!attempted, 'QA05_TICKET_PROBE_ALREADY_ATTEMPTED'); attempted = true
    assert(equal(supplied, config) && /^[a-f0-9]{32}$/.test(requestId))
    const before = await memberships(callSignal)
    let authentication = relay.takeConsumedAuthentication(principals[0])
    assert(authentication.type === 'authenticate' && authentication.eventId === fixed.eventId)
    let message = Buffer.from(JSON.stringify(authentication)); authentication = null
    const wire = maskedFrame(message); message.fill(0); message = null
    const result = await new Promise((resolve, reject) => {
      let socket, reader, done = false, sent = 0, snapshots = 0, closeCode = null, sentClose = false
      const fail = () => finish(false)
      const finish = ok => {
        if (done) return
        done = true; clearTimeout(timer); signal.removeEventListener('abort', fail); callSignal.removeEventListener('abort', fail)
        reader?.dispose(); request.destroy(); socket?.destroy(); wire.fill(0)
        if (ok) resolve({ authenticateMessages: sent, snapshots, closeCode, closed: true })
        else reject(new Error('QA05_CONSUMED_TICKET_REPLAY_UNAVAILABLE'))
      }
      const key = randomBytes(16).toString('base64')
      const request = http.request({ hostname: origin.hostname === '[::1]' ? '::1' : origin.hostname,
        port: origin.port || 80, method: 'GET', path: '/world-ws', agent: false,
        headers: { Host: frontend.host, Origin: config.origin, Upgrade: 'websocket', Connection: 'Upgrade',
          'Sec-WebSocket-Version': '13', 'Sec-WebSocket-Key': key } })
      const timer = setTimeout(fail, 2800)
      signal.addEventListener('abort', fail, { once: true }); callSignal.addEventListener('abort', fail, { once: true })
      request.on('error', fail); request.on('response', response => { response.destroy(); fail() })
      request.on('upgrade', (response, opened, head) => {
        socket = opened
        try {
          live(callSignal)
          assert(response.statusCode === 101 && !response.headers['sec-websocket-extensions'] && !response.headers['sec-websocket-protocol'])
          assert.equal(response.headers['sec-websocket-accept'], createHash('sha1').update(key + '258EAFA5-E914-47DA-95CA-C5AB0DC85B11').digest('base64'))
          socket.on('error', fail)
          socket.on('close', () => finish(closeCode === 4401 && sent === 1 && snapshots === 0))
          reader = createServerFrameReader({ decodeServer, guard: () => live(callSignal),
            application(_bytes, frame) { if (frame.type === 'snapshot') snapshots++; throw new Error('QA05_REPLAY_APPLICATION_FRAME') },
            control(bytes) {
              const opcode = bytes[0] & 15, size = bytes[1] & 127, payload = bytes.subarray(2)
              if (opcode === 8) {
                assert(!sentClose && size >= 2); closeCode = payload.readUInt16BE(); assert.equal(closeCode, 4401)
                sentClose = true; assert(socket.write(maskedFrame(payload, 8)))
              } else if (opcode === 9) assert(socket.write(maskedFrame(payload, 10)))
            } })
          socket.on('data', bytes => { try { reader.feed(bytes) } catch { fail() } })
          live(callSignal); sent++; assert(socket.write(wire, error => { wire.fill(0); if (error) fail() }))
          if (head.length) reader.feed(head)
        } catch { fail() }
      })
      try { live(callSignal); request.end() } catch { fail() }
    })
    const after = await memberships(callSignal)
    assert(equal(before, after), 'QA05_REPLAY_MEMBERSHIP_CHANGED')
    return { version: 1, binding: { ...fixed }, requestId, action: 'CONSUMED_TICKET_REPLAY', connectionAttempts: 1,
      ...result, membershipBefore: { connected: 2, duplicates: 0 }, membershipAfter: { connected: 2, duplicates: 0 } }
  }
}
