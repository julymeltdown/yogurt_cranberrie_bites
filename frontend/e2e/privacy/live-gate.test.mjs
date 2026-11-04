import assert from 'node:assert/strict'
import test from 'node:test'
import { readFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { root } from './runtime.mjs'
import { roles, domains, qualifyPlan, executeHttp } from './live-gate.mjs'
import { schemaAccepts } from './schema.mjs'

const spec = JSON.parse(await readFile(resolve(root, 'contracts/generated/mcr-domain.openapi.json'), 'utf8'))
const uuid = n => `00000000-0000-4000-8000-${String(n).padStart(12, '0')}`
const env = Object.fromEntries(roles.map(role => ['QA04_' + role.toUpperCase(), 'synthetic-test-token']))
// These identities qualify ONLY the gate's own tests, never a service seed handoff.
function fixturePlan() {
  return { version: 1, isolatedEnvironment: true, origin: 'https://qa04.invalid', environmentId: 'synthetic-gate-test', seedRevision: 'synthetic', serviceRevision: 'synthetic', principalHandoff: 'synthetic-selftest-only', allowSeedMutations: true,
    principals: Object.fromEntries(roles.map((role, index) => [role, { userId: uuid(index + 10), eventId: uuid(1), tokenEnv: 'QA04_' + role.toUpperCase() }])),
    httpCells: roles.flatMap(role => ['list', 'detail', 'mutation'].map(operation => ({ role, operation, probes: [{
      operationId: operation === 'list' ? 'listConcertSocialRequests' : operation === 'detail' ? 'getConcertSocialRequest' : 'revokeConcertSocialRequest',
      parameters: { eventId: uuid(1), ...(operation === 'list' ? {} : { requestId: uuid(29) }) },
      ...(operation === 'mutation' ? { body: { expected_revision: 2 }, idempotencyKey: uuid(900 + roles.indexOf(role)) } : {}),
      expectedStatus: 404, domains, forbiddenValues: domains.map(domain => 'gate-test-private-' + domain), requiredValues: [],
    }] }))) }
}
const response = (extra = {}) => Response.json({ code: 'NOT_FOUND', message: 'Resource unavailable.', details: { reason: 'RESOURCE_UNAVAILABLE' }, ...extra }, { status: 404, headers: { 'Cache-Control': 'no-store' } })
test('unavailable setup is rejected before any transport work', () => {
  for (const input of [null, {}, { version: 1 }, { ...fixturePlan(), principals: {} }]) assert.throws(() => qualifyPlan(input, env, spec))
  assert.throws(() => qualifyPlan(fixturePlan(), {}, spec))
})
test('all seven roles and 21 HTTP cells are required, without duplicates or role bypass', () => {
  const plan = fixturePlan(); plan.httpCells.pop(); assert.throws(() => qualifyPlan(plan, env, spec))
  const duplicate = fixturePlan(); duplicate.httpCells[0] = duplicate.httpCells[1]; assert.throws(() => qualifyPlan(duplicate, env, spec))
  const admin = fixturePlan(); admin.principals.admin.userId = admin.principals.owner.userId; assert.throws(() => qualifyPlan(admin, env, spec))
})
test('unregistered endpoints, methods, queries, cross-event IDs and malformed bodies refuse setup', () => {
  const changes = [
    probe => { probe.operationId = 'inventedSocialWsEvent' },
    probe => { probe.query = { secret_bypass: true } },
    probe => { probe.parameters.eventId = uuid(222) },
    probe => { probe.body = { expected_revision: -1 } },
    probe => { probe.parameters.requestId = '../../other-user' },
  ]
  for (const change of changes) {
    const plan = fixturePlan(); change(plan.httpCells[2].probes[0]); assert.throws(() => qualifyPlan(plan, env, spec))
  }
})
test('non-isolated setup, URL credentials and unqualified mutations refuse setup', () => {
  for (const patch of [{ isolatedEnvironment: false }, { origin: 'https://user:password@qa04.invalid' }, { origin: 'http://unqualified.invalid' }, { allowSeedMutations: false }]) assert.throws(() => qualifyPlan({ ...fixturePlan(), ...patch }, env, spec))
})
test('gate selftest consumes published schema/status and labels mocked fetch as synthetic', async () => {
  let calls = 0
  const rows = await executeHttp(qualifyPlan(fixturePlan(), env, spec), env, spec, async (url, options) => {
    assert(url.startsWith('https://qa04.invalid/api/concert/events/'))
    assert.equal(options.redirect, 'error'); assert.equal(options.cache, 'no-store')
    assert.equal(options.headers.Authorization, 'Bearer synthetic-test-token'); calls++
    return response()
  })
  assert.equal(calls, 21); assert.equal(rows.length, 21)
  assert(rows.every(row => row.provenance === 'SYNTHETIC_GATE_SELFTEST'))
})
test('schema-valid private markers, unknown private fields and missing no-store fail rather than produce passing receipts', async () => {
  const plan = qualifyPlan(fixturePlan(), env, spec)
  const bad = [
    () => response({ message: 'gate-test-private-handle' }),
    () => response({ handle: 'gate-test-private-handle' }),
    () => new Response(JSON.stringify({ code: 'NOT_FOUND', message: 'Resource unavailable.', details: { reason: 'RESOURCE_UNAVAILABLE' } }), { status: 404 }),
    () => Response.json({ code: 'NOT_FOUND', message: 'Resource unavailable.', details: { reason: 'PERMISSION_DENIED' } }, { status: 404, headers: { 'Cache-Control': 'no-store' } }),
    () => Response.json({}, { status: 200, headers: { 'Cache-Control': 'no-store' } }),
    () => new Response('{"code":"NOT_FOUND","message":"gate-test-private-\\u0068andle","details":{"reason":"RESOURCE_UNAVAILABLE"}}', { status: 404, headers: { 'Cache-Control': 'no-store' } }),
  ]
  for (const make of bad) await assert.rejects(executeHttp(plan, env, spec, async () => make()))
})
test('oversized private responses stop at the bounded reader', async () => {
  const plan = qualifyPlan(fixturePlan(), env, spec)
  await assert.rejects(executeHttp(plan, env, spec, async () => new Response('x'.repeat(1048577), { status: 404, headers: { 'Cache-Control': 'no-store' } })), /qualification bound/)
})
test('positive-content assertions stop a vacuous empty successful response', async () => {
  const plan = fixturePlan(); const probe = plan.httpCells[0].probes[0]
  probe.expectedStatus = 200; probe.requiredValues = ['expected-seeded-resource']
  await assert.rejects(executeHttp(qualifyPlan(plan, env, spec), env, spec,
    async () => Response.json({ items: [], next_cursor: null }, { headers: { 'Cache-Control': 'no-store' } })))
})
test('frozen public projection fixtures reject handle/claim/recipient leaks while authorized shapes remain separate', async () => {
  const selected = ['public-intro', 'intro-age-leak', 'public-post', 'feed-private-handle', 'party-full', 'party-private-leak', 'private-beacon', 'beacon-world-coordinate', 'private-age-claim', 'notification-safe', 'notification-raw-payload']
  const index = JSON.parse(await readFile(resolve(root, 'contracts/fixtures/domain/index.json'), 'utf8'))
  for (const item of index.filter(item => selected.includes(item.file.replace('.json', '')))) {
    const value = JSON.parse(await readFile(resolve(root, 'contracts/fixtures/domain', item.file), 'utf8'))
    const name = 'Domain_' + item.schema.replace('.', '_')
    assert.equal(schemaAccepts(spec, { $ref: '#/components/schemas/' + name }, value), item.expected === 'accept', item.file)
  }
  assert.equal(schemaAccepts(spec, { type: 'object', unknownAuthorizationKeyword: true }, {}), false)
})
