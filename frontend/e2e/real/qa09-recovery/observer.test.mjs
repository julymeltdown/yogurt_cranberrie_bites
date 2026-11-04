import { EventEmitter } from 'node:events'
import { test } from 'node:test'
import assert from 'node:assert/strict'
import { classifyRequest, observeClient } from './observer.mjs'
import { cases, caseDefinition, initialDisposition } from './cases.mjs'

// Explicit synthetic observer-unit inputs. No actual App/service/recovery credit.
const origin = 'http://localhost:5173'
const id = '10000000-0000-4000-8000-000000000001'
const url = `${origin}/api/concert/events/${id}/reservations/${id}/claim`
function request() { return { url: () => url, method: () => 'POST', headers: () => ({ 'idempotency-key': id, authorization: 'PRIVATE_TOKEN' }) } }

test('classifies only exact same-origin known routes, without query/identity export', () => {
  assert.equal(classifyRequest(`${url}?confirmation_code=PRIVATE_CODE`, 'POST', origin), 'pickupClaim')
  assert.equal(classifyRequest(url, 'GET', origin), null)
  assert.equal(classifyRequest(url.replace('localhost', 'elsewhere'), 'POST', origin), null)
  assert.equal(classifyRequest(`${url}/unknown`, 'POST', origin), null)
  assert.equal(classifyRequest('broken', 'POST', origin), null)
})

test('matches original command within one observer but never publishes private values', () => {
  const page = new EventEmitter(), a = observeClient(page, { origin }), b = observeClient(new EventEmitter(), { origin })
  const req = request(); page.emit('request', req); page.emit('response', { request: () => req, status: () => 202 })
  page.emit('request', request())
  const data = a.snapshot(), [first, second, third] = data.events
  assert.equal(first.commandIdentity, second.commandIdentity); assert.equal(first.commandIdentity, third.commandIdentity)
  assert.match(first.commandIdentity, /^[0-9a-f]{64}$/)
  const serialized = JSON.stringify(data)
  for (const secret of [id, origin, 'PRIVATE_TOKEN', 'PRIVATE_CODE', 'authorization', 'headers']) assert.equal(serialized.includes(secret), false)
  a.detach(); b.detach()
})

test('request failure and browser errors are fixed safe categories; no error body retained', () => {
  const page = new EventEmitter(), observer = observeClient(page, { origin }), req = request()
  page.emit('request', req); page.emit('requestfailed', req); page.emit('pageerror', new Error('PRIVATE_ERROR'))
  assert.deepEqual(observer.snapshot().events.map(e => e.kind), ['request', 'requestFailed', 'pageError'])
  assert.equal(JSON.stringify(observer.snapshot()).includes('PRIVATE_ERROR'), false); observer.detach()
})

test('status query binds original command without retaining query or introducing a new intent', () => {
  const page = new EventEmitter(), observer = observeClient(page, { origin }), req = request()
  page.emit('request', req)
  page.emit('request', { url: () => `${origin}/api/concert/events/${id}/commands?operation_id=claimConcertReservation&idempotency_key=${id}`, method: () => 'GET' })
  const [claim, status] = observer.snapshot().events
  assert.equal(claim.commandIdentity, status.commandIdentity)
  assert.equal(status.subjectOperation, 'pickupClaim')
  assert.equal(JSON.stringify(status).includes(id), false)
  observer.detach()
})

test('one report cannot correlate private command identity with another report', () => {
  const a = new EventEmitter(), b = new EventEmitter(), x = observeClient(a, { origin }), y = observeClient(b, { origin })
  a.emit('request', request()); b.emit('request', request())
  assert.notEqual(x.snapshot().events[0].commandIdentity, y.snapshot().events[0].commandIdentity)
  x.detach(); y.detach()
})

test('tracks genuine socket close/reopen and duplicate peak without reading payloads', () => {
  const page = new EventEmitter(), observer = observeClient(page, { origin })
  const socket = () => Object.assign(new EventEmitter(), { url: () => 'ws://localhost:5173/world-ws?PRIVATE_TOKEN' })
  const a = socket(), b = socket(); page.emit('websocket', a); page.emit('websocket', b)
  assert.equal(observer.snapshot().peakSockets, 2)
  a.emit('close'); b.emit('close'); assert.equal(observer.snapshot().activeSockets, 0)
  assert.equal(a.listenerCount('framereceived'), 0); observer.detach()
})

test('bounded events flag overflow and idempotent detach removes every owned listener', () => {
  const page = new EventEmitter(), observer = observeClient(page, { origin, maxEvents: 1 })
  page.emit('request', request()); page.emit('request', request())
  assert.equal(observer.snapshot().events.length, 1); assert.equal(observer.snapshot().overflow, true)
  const ws = Object.assign(new EventEmitter(), { url: () => 'ws://localhost:5173/world-ws' }); page.emit('websocket', ws)
  observer.detach(); observer.detach()
  assert.equal(ws.listenerCount('close'), 0)
  for (const event of ['request', 'response', 'requestfailed', 'websocket', 'pageerror']) assert.equal(page.listenerCount(event), 0)
})

test('all six original clauses remain NOT_RUN and unknown case cannot manufacture coverage', () => {
  assert.equal(Object.keys(cases).length, 6)
  assert.ok(Object.values(initialDisposition()).every(c => c.status === 'NOT_RUN'))
  assert.throws(() => caseDefinition('toString'), /QA09_UNKNOWN_CASE/)
  assert.throws(() => caseDefinition('QA09-FAKE'), /QA09_UNKNOWN_CASE/)
})

test('canonical deletion read is distinguished from the one-way deletion request', () => {
  assert.equal(classifyRequest(`${origin}/api/auth/account`, 'DELETE', origin), 'deletionRequest')
  assert.equal(classifyRequest(`${origin}/api/auth/account/deletion`, 'GET', origin), 'deletionRead')
  assert.equal(classifyRequest(`${origin}/api/auth/account`, 'POST', origin), null)
})
