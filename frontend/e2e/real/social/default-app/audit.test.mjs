import assert from 'node:assert/strict'
import test from 'node:test'
import { createAudit, policyMetadata, publicPath } from './audit.mjs'
const baseline = { protocolVersion: '1.0.0', type: 'policy.baseline', eventId: 'unit-event', streamId: 'unit-stream', scope: 'SOCIAL', policyRevision: 1 }
test('policy observations require the actual event/stream and never expose an authentication frame', () => {
  const inspect = value => policyMetadata(JSON.stringify(value), 'unit-event', 'unit-stream', 'unit-user')
  assert.equal(inspect({ type: 'authenticate', ticket: 'unit-secret-never-recorded' }), null)
  assert.equal(inspect({ ...baseline, ticket: 'unit-secret-never-recorded' }), null)
  assert.equal(inspect({ ...baseline, eventId: 'foreign' }), null)
  assert.equal(inspect({ ...baseline, streamId: 'foreign' }).matches, false)
  assert.equal(inspect(baseline).matches, true)
  const snapshot = { protocolVersion: '1.0.0', type: 'snapshot', eventId: 'unit-event', streamId: 'unit-stream', selfUserId: 'unit-user', streamSeq: 0, players: [{ private: 'never-copied' }] }
  assert.deepEqual(inspect(snapshot), { type: 'snapshot', streamId: 'unit-stream', matches: true })
  assert.equal(inspect({ ...snapshot, selfUserId: 'foreign' }), null)
  assert(!JSON.stringify(inspect(snapshot)).includes('never-copied'))
})
test('malformed, oversized and unsupported policy frames do not become baseline evidence', () => {
  for (const payload of ['not-json', 'x'.repeat(1024 * 1024 + 1), JSON.stringify({ ...baseline, scope: 'OTHER' }),
    JSON.stringify({ ...baseline, policyRevision: -1 }), JSON.stringify({ ...baseline, policyRevision: 0 }), JSON.stringify({ ...baseline, type: 'policy.invalidate', reason: 'MADE_UP' })]) {
    assert.equal(policyMetadata(payload, 'unit-event', 'unit-stream', 'unit-user'), null)
  }
  assert.equal(policyMetadata(JSON.stringify(baseline), 'unit-event', null, 'unit-user').matches, false)
})
test('HTTP projection removes query/identities and audit refuses private fields with bounded storage', () => {
  assert.equal(publicPath('http://127.0.0.1:5173/api/concert/events/12345678-1234-1234-1234-123456789abc/bootstrap?token=unit-secret'), '/api/concert/events/{id}/bootstrap')
  const audit = createAudit(1)
  for (const key of ['body', 'ticket', 'cookie', 'handle', 'payload', 'email']) assert.throws(() => audit.add({ kind: 'http', [key]: 'unit-private' }))
  audit.add({ kind: 'http', status: 200 }); audit.add({ kind: 'http', status: 403 })
  assert.equal(audit.read().rows.length, 1); assert.equal(audit.read().overflow, true)
})
