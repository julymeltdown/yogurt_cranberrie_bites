// SYNTHETIC SOURCE TESTS ONLY. No managed producer/browser acceptance.
import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { createWorldObservation, assertPairConverged } from '../../../../../e2e/qa05-functional-recovery/observations.mjs'
import { createCheckpoints } from '../../../../../e2e/qa05-functional-recovery/checkpoints.mjs'
import { runRecoveryCases } from '../../../../../e2e/qa05-functional-recovery/driver.mjs'

const fixture = JSON.parse(readFileSync(new URL('../../../../../../contracts/fixtures/realtime/movement-heartbeat.json', import.meta.url)))
const template = fixture.messages.find(row => row.message.type === 'snapshot').message
const input = fixture.messages.find(row => row.message.type === 'input').message
const delta = fixture.messages.find(row => row.message.type === 'delta').message
const copy = value => structuredClone(value)
function pair() {
  const snapshots = [0, 1].map(index => ({ ...copy(template), selfUserId: template.players[index].userId,
    streamId: `00000000-0000-4000-8000-00000000002${index}`, players: copy(template.players.slice(0, 2)) }))
  const observers = snapshots.map(snapshot => {
    const observer = createWorldObservation({ eventId: snapshot.eventId, actorId: snapshot.selfUserId, maximumPlayers: 2 })
    observer.open('current'); observer.message('current', 'server', snapshot); return observer
  })
  const renderings = snapshots.map(snapshot => ({ games: 1, canvases: 1, sprites: snapshot.players.map(p => ({ ...p.position })) }))
  return { snapshots, observers, renderings }
}

test('complete current pair plus actual matching renderer converges; summary omits identity and protected values', () => {
  const p = pair(); assert.equal(assertPairConverged(p.observers, p.renderings).ghostSprites, 0)
  p.observers[0].message('current', 'client', { type: 'authenticate', ticket: 'NEVER-PERSIST-TICKET' })
  const summary = JSON.stringify(p.observers[0].summary())
  assert(!summary.includes(template.eventId) && !summary.includes(template.selfUserId) && !summary.includes('NEVER-PERSIST'))
})
test('an actual delivered stream gap withdraws the roster until a full correlated fresh baseline', () => {
  const p = pair(), observer = p.observers[0]
  observer.message('current', 'server', { ...copy(delta), streamId: p.snapshots[0].streamId, roomEpoch: template.roomEpoch, streamSeq: 2 })
  assert.equal(observer.read().baseline, null); assert.equal(observer.read().players.size, 0)
  assert.throws(() => assertPairConverged(p.observers, p.renderings))
  observer.message('current', 'server', { ...copy(p.snapshots[0]), streamId: '00000000-0000-4000-8000-000000000099', cause: 'RESYNC' })
  assert.equal(observer.summary().gaps, 1); assertPairConverged(p.observers, p.renderings)
})
test('late messages and close callbacks from an obsolete socket cannot replace the new baseline', () => {
  const p = pair(), observer = p.observers[0]
  observer.close('current', 4409); observer.open('replacement')
  observer.message('replacement', 'server', { ...copy(p.snapshots[0]), streamId: 'new-stream' })
  observer.message('current', 'server', p.snapshots[0]); observer.close('current', 4500)
  assert.equal(observer.read().baseline.streamId, 'new-stream'); assert.equal(observer.read().activeSockets, 1)
})
test('bare 4409 is not a version reason; a typed BACKPRESSURE reason stays distinct', () => {
  const p = pair(), observer = p.observers[0]
  observer.close('current', 4409); assert.equal(observer.summary().terminalReason, null)
  observer.open('next'); observer.message('next', 'server', { type: 'resyncRequired', reason: 'BACKPRESSURE' })
  assert.equal(observer.summary().terminalReason, 'BACKPRESSURE'); assert.equal(observer.read().baseline, null)
})
test('foreign identity or event never becomes the current evidence baseline', () => {
  for (const key of ['eventId', 'selfUserId']) {
    const p = pair(); p.observers[0].message('current', 'server', { ...copy(p.snapshots[0]), [key]: 'foreign' })
    assert.equal(p.observers[0].read().baseline, null); assert.equal(p.observers[0].summary().foreignSnapshots, 1)
    assert.throws(() => assertPairConverged(p.observers, p.renderings))
  }
})
test('complete motion anchors update self even without an upsert; wire ACK bookkeeping is bounded', () => {
  const p = pair(), observer = p.observers[0]
  for (let seq = 1; seq <= 65; seq++) observer.message('current', 'client', { ...copy(input), seq })
  assert.equal(observer.read().pending, 64); assert.equal(observer.summary().pendingOverflow, 1)
  observer.message('current', 'server', { ...copy(delta), streamId: p.snapshots[0].streamId, roomEpoch: template.roomEpoch,
    streamSeq: 1, ackInputSeq: 65, upserts: [], removes: [], selfMotion: { position: { x: 40, y: 60 } } })
  assert.equal(observer.read().pending, 0); assert.deepEqual(observer.read().players.get(template.selfUserId), { x: 40, y: 60 })
})
test('same counts do not conceal renderer ghosts, wrong coordinates or duplicate games', () => {
  for (const mutate of [r => r[0].sprites.push({ x: 0, y: 0 }), r => { r[0].sprites[0].x++ }, r => { r[0].games = 2 }]) {
    const p = pair(); mutate(p.renderings); assert.throws(() => assertPairConverged(p.observers, p.renderings))
  }
})
test('disposal withdraws cached observations and ignores later frames', () => {
  const p = pair(), observer = p.observers[0]; observer.dispose(); observer.open('late'); observer.message('late', 'server', p.snapshots[0])
  assert.equal(observer.read().baseline, null); assert.equal(observer.read().players.size, 0); assert.equal(observer.read().activeSockets, 0)
})

const binding = () => ({ candidate: 'a'.repeat(40), environmentId: '00000000-0000-4000-8000-000000000001',
  eventId: template.eventId, windowId: '00000000-0000-4000-8000-000000000002', expiresAt: new Date(Date.now() + 120000).toISOString() })
function reply(request) {
  return { ...request, action: request.request.action, status: request.request.action === 'observe-only' ? 'OBSERVED'
    : request.step === 'frontend-ready' ? 'ARMED' : 'APPLIED', proof: { syntheticTestOnly: true } }
}
test('checkpoint arming and executed after-observation are distinct, awaitable, correlated stages', async () => {
  const seen = [], c = createCheckpoints({ binding: binding(), invoke: async request => { seen.push(request); return reply(request) } })
  await c.checkpoint('packet-gap', 'frontend-ready', {}); await c.checkpoint('packet-gap', 'after-observation', {})
  assert.deepEqual(seen.map(r => [r.sequence, r.step, r.request.action]), [[1, 'frontend-ready', 'drop-next-delta'], [2, 'after-observation', 'drop-next-delta']])
  await assert.rejects(c.checkpoint('packet-gap', 'frontend-ready', {}), /order/)
})
test('read-only IN04 observation cannot masquerade as a QA05 fault controller', async () => {
  const c = createCheckpoints({ binding: binding(), invoke: async request => ({ ...reply(request), status: 'OBSERVED' }) })
  await assert.rejects(c.checkpoint('packet-gap', 'frontend-ready', {}), /executed fault/)
})
test('a supplied binding/configuration is not an owner execution proof', async () => {
  const c = createCheckpoints({ binding: binding(), invoke: async request => ({ ...reply(request), proof: undefined }) })
  await assert.rejects(c.checkpoint('packet-gap', 'frontend-ready', {}), /proof required/)
})
for (const field of ['candidate', 'environmentId', 'eventId', 'windowId', 'expiresAt']) test(`foreign ${field} rejects the owner checkpoint and cannot be retried`, async () => {
  let calls = 0
  const c = createCheckpoints({ binding: binding(), invoke: async request => { calls++; return { ...reply(request), binding: { ...request.binding, [field]: 'foreign' } } } })
  await assert.rejects(c.checkpoint('packet-gap', 'frontend-ready', {}), /Foreign/)
  await assert.rejects(c.checkpoint('packet-gap', 'frontend-ready', {}), /UNAVAILABLE/); assert.equal(calls, 1)
})
test('expired, missing, unknown and private-expanded interfaces fail before invoking owner actions', async () => {
  assert.throws(() => createCheckpoints({ binding: binding() }), /NOT_BOUND/)
  assert.throws(() => createCheckpoints({ binding: { ...binding(), token: 'private' }, invoke() {} }), /public/)
  let calls = 0; const b = binding(), c = createCheckpoints({ binding: b, now: () => Date.parse(b.expiresAt), invoke() { calls++ } })
  await assert.rejects(c.checkpoint('world-restart', 'frontend-ready', {}), /EXPIRED/)
  await assert.rejects(c.checkpoint('invented-http-endpoint', 'frontend-ready', {}), /Unknown/); assert.equal(calls, 0)
})
test('an uncertain action never silently retries or overlaps another owner mutation', async () => {
  let release, calls = 0; const wait = new Promise(resolve => { release = resolve })
  const c = createCheckpoints({ binding: binding(), invoke: async () => { calls++; await wait; throw Error('Uncertain actual outcome') } })
  const first = c.checkpoint('world-restart', 'frontend-ready', {})
  await assert.rejects(c.checkpoint('world-restart', 'frontend-ready', {}), /Concurrent/)
  release(); await assert.rejects(first, /UNCERTAIN/)
  await assert.rejects(c.checkpoint('world-restart', 'frontend-ready', {}), /UNAVAILABLE/); assert.equal(calls, 1)
})
test('restart cannot skip producer-ready or run without the actual owner time reserve', async () => {
  const c = createCheckpoints({ binding: binding(), invoke: async request => reply(request) })
  await c.checkpoint('world-restart', 'frontend-ready', {})
  await assert.rejects(c.checkpoint('world-restart', 'after-observation', {}), /order/)
  await c.checkpoint('world-restart', 'producer-ready', {})
  await c.checkpoint('world-restart', 'after-observation', {})
  let invoked = false
  const short = createCheckpoints({ binding: { ...binding(), expiresAt: new Date(Date.now() + 60000).toISOString() }, invoke: () => { invoked = true } })
  await assert.rejects(short.checkpoint('world-restart', 'frontend-ready', {}), /RESERVE/)
  assert.equal(invoked, false)
})
test('driver awaits genuine producer-ready before one reconnect and compares captured old stream/epoch values', async () => {
  const p = pair(), seen = []; let complete
  const ready = new Promise(resolve => { complete = resolve })
  const session = { kind: 'QA05_ACTUAL_DEFAULT_APP_PAIR', sample: async () => ({ observations: p.observers, renderings: p.renderings }),
    reconnect: async () => {
      seen.push('reconnect')
      for (let i = 0; i < 2; i++) {
        p.observers[i].close('current', null); p.observers[i].open('next')
        p.observers[i].message('next', 'server', { ...p.snapshots[i], streamId: `new-stream-${i}`, roomEpoch: 'new-epoch' })
      }
    }, awaitConvergence: async ({ beforeState }) => {
      assert.equal(beforeState[0].baseline.streamId, p.snapshots[0].streamId)
      seen.push('convergence'); return { observations: p.observers, renderings: p.renderings }
    } }
  const running = runRecoveryCases({ caseIds: ['world-restart'], binding: binding(), session, checkpoint: async request => {
    seen.push(request.step)
    if (request.step === 'producer-ready') await ready
    return reply(request)
  } })
  // Yield to the readiness checkpoint, which remains unresolved.
  await new Promise(resolve => setImmediate(resolve))
  assert.deepEqual(seen, ['frontend-ready', 'producer-ready'])
  complete(); const results = await running
  assert.equal(results[0].status, 'PASS')
  assert.deepEqual(seen, ['frontend-ready', 'producer-ready', 'reconnect', 'convergence', 'after-observation'])
})
test('failed producer readiness aborts the combined driver without reconnecting or claiming PASS', async () => {
  const p = pair(); let reconnects = 0
  const session = { kind: 'QA05_ACTUAL_DEFAULT_APP_PAIR', sample: async () => ({ observations: p.observers, renderings: p.renderings }), reconnect: async () => { reconnects++ } }
  await assert.rejects(runRecoveryCases({ caseIds: ['world-restart'], binding: binding(), session,
    checkpoint: async request => ({ ...reply(request), ...(request.step === 'producer-ready' ? { status: 'UNAVAILABLE' } : {}) }) }), /UNAVAILABLE/)
  assert.equal(reconnects, 0)
})
