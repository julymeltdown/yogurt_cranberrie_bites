// Root-owned transport attachment. A qualified managed window and the actual
// canonical C05 decoder must be supplied by its caller; this creates no actors.
import assert from 'node:assert/strict'
import http from 'node:http'
import { createHash } from 'node:crypto'
import { TextDecoder } from 'node:util'

const MAX_PAYLOAD = 65536
const MAX_BUFFER = 262144
const decoder = new TextDecoder('utf-8', { fatal: true })

// Keep original RFC6455 bytes, including control frames. Fragmented or compressed
// application messages are refused: a partial message cannot be classified C05.
export function createServerFrameReader({ application, control, decodeServer, guard }) {
  for (const fn of [application, control, decodeServer, guard]) assert.equal(typeof fn, 'function')
  let pending = Buffer.alloc(0), retired = false
  function feed(chunk) {
    assert(!retired, 'QA05_READER_RETIRED')
    try {
      guard()
      assert(chunk instanceof Uint8Array, 'QA05_BYTES_REQUIRED')
      let offset = 0
      while (offset < chunk.length || pending.length >= 2) {
        if (pending.length < 2) {
          const take = Math.min(2 - pending.length, chunk.length - offset)
          pending = Buffer.concat([pending, chunk.subarray(offset, offset + take)]); offset += take
          if (pending.length < 2) break
        }
        const first = pending[0], second = pending[1], opcode = first & 15
        assert((first & 0x70) === 0 && !(second & 0x80), 'QA05_SERVER_FRAME_ENCODING')
        const fin = (first & 0x80) !== 0, code = second & 127
        const header = code < 126 ? 2 : code === 126 ? 4 : 10
        if (pending.length < header) {
          const take = Math.min(header - pending.length, chunk.length - offset)
          pending = Buffer.concat([pending, chunk.subarray(offset, offset + take)]); offset += take
          if (pending.length < header) break
        }
        const length = code < 126 ? code : code === 126 ? pending.readUInt16BE(2) : Number(pending.readBigUInt64BE(2))
        assert(Number.isSafeInteger(length) && length <= MAX_PAYLOAD, 'QA05_FRAME_BOUND')
        assert(code < 126 || length >= (code === 126 ? 126 : 65536), 'QA05_NONCANONICAL_LENGTH')
        const isControl = opcode >= 8
        assert(fin && (isControl ? [8, 9, 10].includes(opcode) && length <= 125 : opcode === 1), 'QA05_UNSUPPORTED_FRAME')
        const total = header + length
        if (pending.length < total) {
          const take = Math.min(total - pending.length, chunk.length - offset)
          pending = Buffer.concat([pending, chunk.subarray(offset, offset + take)]); offset += take
          if (pending.length < total) break
        }
        const original = pending; pending = Buffer.alloc(0)
        guard()
        if (isControl) control(original)
        else {
          const message = decodeServer(decoder.decode(original.subarray(header)))
          application(original, message)
        }
        if (offset === chunk.length && !pending.length) break
      }
    } catch {
      retired = true; pending = Buffer.alloc(0)
      throw new Error('QA05_SERVER_FRAME_REFUSED')
    }
  }
  return Object.freeze({ feed, dispose() { retired = true; pending = Buffer.alloc(0) }, bufferedBytes: () => pending.length })
}

function loopbackOrigin(value) {
  const url = new URL(value)
  assert(url.protocol === 'http:' && ['127.0.0.1', '[::1]', 'localhost'].includes(url.hostname))
  assert(!url.username && !url.password && url.pathname === '/' && !url.search && !url.hash)
  return url
}

// Install on the SAME Vite httpServer, with Vite's /world-ws proxy omitted.
// HTTP /api remains Vite's normal proxy. There is no public fault-control route.
export function installQa05WorldRelay({ httpServer, worldOrigin, frontendOrigin, eventId,
  principalIds, createFaultController, decodeServer, guard, signal, now = () => performance.now() }) {
  assert(httpServer && typeof httpServer.on === 'function')
  const upstreamOrigin = loopbackOrigin(worldOrigin), origin = loopbackOrigin(frontendOrigin)
  assert.equal(typeof eventId, 'string')
  assert(Array.isArray(principalIds) && principalIds.length === 2 && new Set(principalIds).size === 2)
  assert(principalIds.every(id => typeof id === 'string' && id.length > 0))
  for (const fn of [createFaultController, decodeServer, guard, now]) assert.equal(typeof fn, 'function')
  assert(signal instanceof AbortSignal && !signal.aborted)
  const pairs = new Set(), byPrincipal = new Map()
  let disposed = false, accepted = 0
  function live() { assert(!disposed && !signal.aborted, 'QA05_RELAY_RETIRED'); guard() }
  function upgrade(req, downstream, initial) {
    let path
    try { path = new URL(req.url, origin).pathname } catch { downstream.destroy(); return }
    if (path !== '/world-ws') return
    let pair
    try {
      live()
      assert(req.method === 'GET' && req.headers.origin === origin.origin, 'QA05_UPGRADE_ORIGIN')
      assert(!req.headers['sec-websocket-extensions'], 'QA05_COMPRESSED_UPGRADE_REFUSED')
      assert(pairs.size < 2 && accepted < 4, 'QA05_CONNECTION_BOUND')
      assert(initial.length <= MAX_BUFFER, 'QA05_PREFETCH_BOUND')
      downstream.pause()
      pair = { downstream, upstream: null, request: null, identity: null, reader: null, faults: null,
        closed: false, receivedFrames: 0, forwardedBytes: 0, openedAtMs: now(), failure: null }
      pairs.add(pair); accepted++
      function close(reason = 'QA05_PAIR_CLOSED') {
        if (pair.closed) return
        pair.closed = true; pair.failure = reason
        if (byPrincipal.get(pair.identity) === pair) byPrincipal.delete(pair.identity)
        pairs.delete(pair)
        pair.reader?.dispose(); pair.faults?.dispose()
        pair.request?.destroy(); pair.upstream?.destroy(); downstream.destroy()
      }
      pair.close = close
      function write(socket, bytes) {
        live()
        assert(!pair.closed && socket && !socket.destroyed && socket.writable, 'QA05_SOCKET_UNAVAILABLE')
        assert(socket.writableLength + bytes.length <= MAX_BUFFER, 'QA05_SOCKET_BUFFER_BOUND')
        const ok = socket.write(bytes, error => { if (error) close('QA05_ASYNC_WRITE_FAILED') })
        // Never retry an uncertain write, even when its bytes entered the queue.
        if (!ok) { close('QA05_SOCKET_BACKPRESSURE'); return false }
        pair.forwardedBytes += bytes.length
        return true
      }
      pair.faults = createFaultController({ send: bytes => write(downstream, bytes),
        pauseUpstream() { live(); assert(pair.upstream && !pair.closed); pair.upstream.pause() },
        resumeUpstream() { live(); assert(pair.upstream && !pair.closed); pair.upstream.resume() },
        close: () => close('QA05_FAULT_RETIRED'), now })
      pair.reader = createServerFrameReader({ guard: live, decodeServer,
        control(bytes) { assert(write(downstream, bytes), 'QA05_CONTROL_WRITE_UNCERTAIN') },
        application(bytes, message) {
          if (message.type === 'snapshot') {
            assert(message.eventId === eventId && principalIds.includes(message.selfUserId), 'QA05_FOREIGN_SNAPSHOT')
            assert(pair.identity === null || pair.identity === message.selfUserId, 'QA05_PRINCIPAL_CHANGED')
            assert(!byPrincipal.has(message.selfUserId) || byPrincipal.get(message.selfUserId) === pair, 'QA05_DUPLICATE_PRINCIPAL')
            pair.identity = message.selfUserId; byPrincipal.set(pair.identity, pair)
          }
          pair.receivedFrames++
          const proof = pair.faults.acceptServerFrame(bytes, { isBinary: false, isDelta: message.type === 'delta' })
          assert(proof.state !== 'RETIRED', 'QA05_FAULT_UNCERTAIN')
        } })
      downstream.on('error', () => close('QA05_DOWNSTREAM_ERROR'))
      downstream.on('close', () => close())
      downstream.on('end', () => close())
      const headers = []
      for (let i = 0; i < req.rawHeaders.length; i += 2) headers.push(req.rawHeaders[i], req.rawHeaders[i + 1])
      // Header values, Origin, Host, target/query and authentication remain the
      // actual request. Node's request parser supplies the genuine 101 response.
      const request = http.request({ hostname: upstreamOrigin.hostname === '[::1]' ? '::1' : upstreamOrigin.hostname,
        port: upstreamOrigin.port || 80, method: 'GET', path: req.url, headers, agent: false, timeout: 3000 })
      pair.request = request
      request.on('timeout', () => close('QA05_UPGRADE_TIMEOUT'))
      request.on('error', () => close('QA05_UPSTREAM_ERROR'))
      request.on('response', response => { response.destroy(); close('QA05_UPSTREAM_UPGRADE_REFUSED') })
      request.on('upgrade', (response, upstream, head) => {
        try {
          live(); assert(!pair.closed)
          pair.upstream = upstream
          const expected = createHash('sha1').update(String(req.headers['sec-websocket-key']) + '258EAFA5-E914-47DA-95CA-C5AB0DC85B11').digest('base64')
          assert(response.statusCode === 101 && response.headers['sec-websocket-accept'] === expected)
          assert(!response.headers['sec-websocket-extensions'])
          upstream.setTimeout(0)
          upstream.on('error', () => close('QA05_UPSTREAM_ERROR'))
          upstream.on('close', () => close())
          upstream.on('end', () => close())
          const raw = [`HTTP/${response.httpVersion} ${response.statusCode} ${response.statusMessage}`]
          for (let i = 0; i < response.rawHeaders.length; i += 2) raw.push(`${response.rawHeaders[i]}: ${response.rawHeaders[i + 1]}`)
          assert(write(downstream, Buffer.from(raw.join('\r\n') + '\r\n\r\n')))
          if (initial.length) assert(write(upstream, initial))
          downstream.on('data', bytes => { try { assert(write(upstream, bytes)) } catch { close('QA05_CLIENT_WRITE_FAILED') } })
          upstream.on('data', bytes => { try { pair.reader.feed(bytes) } catch { close('QA05_SERVER_FRAME_REFUSED') } })
          if (head.length) pair.reader.feed(head)
          downstream.resume()
        } catch { close('QA05_UPGRADE_REFUSED') }
      })
      request.end()
    } catch { pair?.close?.('QA05_UPGRADE_REFUSED'); downstream.destroy() }
  }
  function close() {
    if (disposed) return
    disposed = true; httpServer.off('upgrade', upgrade); signal.removeEventListener('abort', close)
    for (const pair of pairs) pair.close('QA05_WINDOW_RETIRED')
  }
  live(); httpServer.on('upgrade', upgrade); signal.addEventListener('abort', close, { once: true })
  return Object.freeze({ close,
    arm(principalId, request) {
      live()
      const pair = byPrincipal.get(principalId)
      assert(pair && !pair.closed, 'QA05_ACTUAL_PRINCIPAL_NOT_OBSERVED')
      const proof = pair.faults.arm(request)
      assert(proof.state === 'ARMED', 'QA05_FAULT_NOT_ARMED')
      return proof
    },
    proof(principalId) {
      live()
      const pair = byPrincipal.get(principalId)
      assert(pair && !pair.closed, 'QA05_ACTUAL_PAIR_UNAVAILABLE')
      return { connected: true, receivedFrames: pair.receivedFrames, forwardedBytes: pair.forwardedBytes,
        fault: pair.faults.readProof(), qualification: 'Actual owned transport sink handling; peer convergence requires the App observer.' }
    },
    summary() { return { activePairs: pairs.size, observedPrincipals: byPrincipal.size, acceptedConnections: accepted, disposed } },
  })
}
