import assert from 'node:assert/strict'

// SYNTHETIC C04 peer. No backend imports, socket admission or production permissions.
export function muralPeer(eventId, actorId) {
  const uuid = n => `00000000-0000-4000-8000-${String(n).padStart(12, '0')}`
  const palette = ['#FAF6ED','#201D1D','#A22635','#605952','#E5DFD4','#713B70','#244C69','#42736B','#7E8245','#BD8E46','#CF6845','#D4A2AC','#938EB0','#8EBDD0','#ACCB96','#FFFFFF']
  const time = () => new Date().toISOString()
  let state = { snapshot_id: uuid(3), event_id: eventId, revision: 10, state: 'EDITABLE', width: 128, height: 128,
    palette, pixels: Array(16384).fill(0), created_at: time(), server_time: time(), cooldown_ms: 0, next_allowed_at: null }
  const operations = []
  let mode = 'ack', generation = 1, serial = 2000, resetRevision = 10
  const commands = new Map(), pending = new Map(), holds = new Set(), log = []
  const counters = { active: 0, peak: 0, heldPeak: 0, posts: 0, replayedPosts: 0, aborted: 0 }
  const snapshot = () => structuredClone({ ...state, server_time: time() })
  function paint(count = 1) {
    const batch = []
    for (let n = 0; n < count; n++) {
      const revision = state.revision + 1, p = { operation_id: uuid(++serial), event_id: eventId, revision,
        x: revision % 128, y: Math.floor(revision / 128) % 128, color_index: revision % 15 + 1, committed_at: time() }
      state.pixels[p.y * 128 + p.x] = p.color_index; state.revision = revision; operations.push(p); batch.push(p)
    }
    return structuredClone(batch)
  }
  const delta = (from, ops, reset = false, to = from + ops.length) => ({ event_id: eventId, from_revision: from, to_revision: to, reset_required: reset, operations: structuredClone(ops) })
  const error = (reason, status) => ({ status, body: { code: `MCR_${status}`, message: 'Synthetic fault response.', details: { reason } } })
  function response(method, url, headers, body) {
    if (url.pathname.endsWith('/snapshot')) return { status: 200, body: snapshot() }
    if (url.pathname.endsWith('/operations')) {
      const after = Number(url.searchParams.get('after_revision')), found = operations.filter(p => p.revision > after)
      return { status: 200, body: after < resetRevision || after > state.revision || found.length > 512 || found.length !== state.revision - after
        ? delta(after, [], true, state.revision) : delta(after, found) }
    }
    if (url.pathname.endsWith('/pixels') && method === 'POST') {
      counters.posts++
      const key = headers['idempotency-key']
      if (commands.has(key)) { counters.replayedPosts++; return error('VERSION_CONFLICT', 409) }
      if (mode === 'rejected') return error('INVALID_INPUT', 400)
      if (state.state !== 'EDITABLE') return error('RESOURCE_UNAVAILABLE', 409)
      if (body.expected_revision !== state.revision) return error('VERSION_CONFLICT', 409)
      assert.ok(Number.isInteger(body.x) && body.x >= 0 && body.x < 128 && Number.isInteger(body.y) && body.y >= 0 && body.y < 128)
      assert.ok(Number.isInteger(body.color_index) && body.color_index >= 0 && body.color_index < 16)
      const p = { operation_id: uuid(++serial), event_id: eventId, revision: ++state.revision, x: body.x, y: body.y, color_index: body.color_index, committed_at: time() }
      state.pixels[p.y * 128 + p.x] = p.color_index; operations.push(p)
      const own = { ...p, next_allowed_at: p.committed_at }
      commands.set(key, { own, body: structuredClone(body), receipt_id: uuid(++serial), created_at: time() })
      return mode === 'lost' ? { status: 503, body: { code: 'MCR_503', message: 'Synthetic lost acknowledgement.' } } : { status: 200, body: own }
    }
    if (url.pathname.endsWith('/commands')) {
      const key = url.searchParams.get('idempotency_key'), c = commands.get(key)
      return c ? { status: 200, body: { receipt_id: c.receipt_id, event_id: eventId, actor_id: actorId,
        operation_id: 'placeConcertPixel', idempotency_key: key, state: 'SUCCEEDED', resource_id: eventId, resource_revision: c.own.revision,
        failure_reason: null, result_status: 200, created_at: c.created_at, expires_at: new Date(Date.now() + 3600000).toISOString() } } : error('RESOURCE_UNAVAILABLE', 404)
    }
    if (url.pathname.endsWith('/result')) {
      const key = url.pathname.split('/').at(-2), c = commands.get(key)
      return c ? { status: 200, body: { receipt_id: c.receipt_id, idempotency_key: key, result: c.own } } : error('RESOURCE_UNAVAILABLE', 404)
    }
    return error('RESOURCE_UNAVAILABLE', 404)
  }
  async function middleware(req, res, next) {
    const url = new URL(req.url, 'http://fixture.invalid')
    if (!url.pathname.startsWith('/api/concert/') || !url.pathname.includes('/mural') && !url.pathname.endsWith('/commands')) return next()
    const client = req.headers['x-qa05-client']
    if (!['a','b','visibility'].includes(client)) { res.statusCode = 403; res.end(); return }
    const kind = url.pathname.endsWith('/snapshot') ? 'snapshot' : url.pathname.endsWith('/operations') ? 'delta' : 'command'
    let bytes = ''; for await (const chunk of req) { bytes += chunk; if (bytes.length > 8192) throw Error('Fixture request too large') }
    const result = response(req.method, url, req.headers, bytes ? JSON.parse(bytes) : null)
    const record = { client, kind, method: req.method, path: url.pathname, generation, status: result.status, revision: state.revision, aborted: false }
    log.push(record); counters.active++; counters.peak = Math.max(counters.peak, counters.active)
    let finished = false
    const finish = aborted => { if (finished) return; finished = true; counters.active--; record.aborted = aborted; if (aborted) counters.aborted++; pending.delete(client + ':' + kind) }
    res.once('close', () => finish(!res.writableEnded))
    const send = () => { if (finished) return; res.writeHead(result.status, { 'Content-Type': 'application/json', 'Cache-Control': 'no-store' }); res.end(JSON.stringify(result.body)); finish(false) }
    const key = client + ':' + kind
    if (holds.delete(key)) {
      assert.ok(!pending.has(key), 'Only one held read per client/kind')
      pending.set(key, { send, res }); counters.heldPeak = Math.max(counters.heldPeak, pending.size)
    } else send()
  }
  return { middleware, snapshot, paint, delta,
    mode(value) { assert.ok(['ack','lost','rejected'].includes(value)); mode = value },
    hold(client, kind) { const key = client + ':' + kind; assert.ok(!holds.has(key) && !pending.has(key)); holds.add(key) },
    release(client, kind) { pending.get(client + ':' + kind)?.send() },
    pause() { assert.equal(state.state, 'EDITABLE'); state = { ...snapshot(), snapshot_id: uuid(++serial), revision: state.revision + 1, state: 'PAUSED' }; resetRevision = state.revision; return snapshot() },
    restore() { assert.equal(state.state, 'PAUSED'); state = { ...snapshot(), snapshot_id: uuid(++serial), revision: state.revision + 1, pixels: Array(16384).fill(2) }; resetRevision = state.revision; return snapshot() },
    resume() { assert.equal(state.state, 'PAUSED'); state = { ...snapshot(), snapshot_id: uuid(++serial), revision: state.revision + 1, state: 'EDITABLE' }; resetRevision = state.revision },
    restart() { generation++; for (const p of pending.values()) p.res.destroy(); pending.clear(); holds.clear(); return generation },
    stats: () => ({ ...counters, pending: pending.size, holds: holds.size, generation, revision: state.revision, log: structuredClone(log) }),
    dispose() { for (const p of pending.values()) p.res.destroy(); pending.clear(); holds.clear() },
  }
}
