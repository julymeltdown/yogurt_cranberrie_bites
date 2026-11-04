import assert from 'node:assert/strict'
import { test } from 'node:test'
import { readFileSync } from 'node:fs'
import { pathToFileURL } from 'node:url'
import path from 'node:path'
const api = await import(pathToFileURL(path.join(process.env.C07_COMPILED_ROOT, 'index.js')))
const contract = (await import(pathToFileURL(path.join(process.env.C07_COMPILED_ROOT, 'document.js')))).document
const fixtures = name => JSON.parse(readFileSync(path.join(process.env.C07_REPO_ROOT, 'contracts/fixtures/c07', name), 'utf8'))
const rows = fixtures('responses.json').cases
const compatibility = fixtures('compatibility.json').cases

test('service policy preserves closed decisions, canonical query names, operation errors and write idempotency', async () => {
  const readId = 'getConcertServicePolicy', writeId = 'setConcertServicePolicy'
  const readRow = rows.find(row => row.operation === readId && row.status === 200)
  const writeRow = rows.find(row => row.operation === writeId && row.status === 200)
  const pendingRow = rows.find(row => row.operation === writeId && row.status === 202)
  assert.ok(readRow && writeRow && pendingRow)
  assert.equal(contract.operations[readId].method, 'GET')
  assert.equal(contract.operations[readId].requestBody, undefined)
  const normal = { event_revision: 1, policy_revision: 1, kind: 'NORMAL', minimum_client_version: '0.1.0',
    server_time: '2030-01-01T00:00:00Z', valid_until: '2030-01-01T00:01:00Z' }
  const unavailable = { ...normal, kind: 'UNAVAILABLE', policy_revision: null, minimum_client_version: null }
  const decision = api.schemas.Domain_admin_ServicePolicyDecision
  for (const kind of ['NORMAL', 'MAINTENANCE', 'CLIENT_VERSION_REQUIRED']) {
    assert.equal(decision.safeParse({ ...normal, kind }).success, true)
    assert.equal(decision.safeParse({ ...normal, kind, policy_revision: null }).success, false)
    assert.equal(decision.safeParse({ ...normal, kind, minimum_client_version: null }).success, false)
  }
  assert.equal(decision.safeParse(unavailable).success, true)
  for (const invalid of [{ ...normal, kind: '503' }, { ...normal, kind: 'GAP' }, { ...normal, kind: '4409' },
    { ...normal, minimumClientVersion: normal.minimum_client_version }, { ...normal, validUntil: normal.valid_until },
    { ...normal, event_revision: 0 }, { ...normal, minimum_client_version: '65536.0.0' },
    { ...normal, minimum_client_version: '0.1.0\n' }]) {
    assert.equal(decision.safeParse(invalid).success, false)
  }
  let calls = 0, observed
  const responding = (body, status = 200, fixture = readRow) => api.createClient({ baseUrl: 'https://fixture.invalid',
    fetch: async (url, init) => {
      calls++; observed = { url: new URL(url), init }
      return new Response(JSON.stringify(body), { status, headers: {
        ...Object.fromEntries(Object.entries(fixture.headers).map(([key, value]) => [key, String(value)])),
        'content-type': 'application/json',
      } })
    } })
  const request = { path: structuredClone(readRow.request.path) }
  const read = responding(unavailable)
  // Omitted claims are syntactically allowed; this mock is no catalog/admission proof.
  for (const query of [undefined, {}, { client_version: '0.1.0' }, { client_release: 'a'.repeat(64) },
    { client_release: 'a'.repeat(64), client_version: '65535.65535.65535' }]) {
    await read.call(readId, { ...request, ...(query === undefined ? {} : { query }) })
    assert.equal(observed.init.method, 'GET'); assert.equal(observed.init.body, undefined)
    assert.deepEqual([...observed.url.searchParams.keys()], ['client_version', 'client_release'].filter(key => query?.[key] !== undefined))
    for (const [key, value] of Object.entries(query ?? {})) assert.equal(observed.url.searchParams.get(key), value)
  }
  for (const query of [{ clientVersion: '0.1.0' }, { clientRelease: 'a'.repeat(64) },
    { client_version: '' }, { client_version: '00.1.0' }, { client_version: '65536.0.0' },
    { client_version: '１.0.0' }, ...['\n', '\r', '\u2028', '\u2029', ' '].map(suffix => ({ client_version: '0.1.0' + suffix })),
    { client_version: ['0.1.0'] },
    { client_release: 'A'.repeat(64) }, { client_release: 'a'.repeat(63) }, { client_release: 'a'.repeat(64) + '\n' }]) {
    const before = calls
    await assert.rejects(read.call(readId, { ...request, query }), api.ContractError)
    assert.equal(calls, before)
  }
  for (const extra of [{ body: {} }, { mediaType: 'application/json' }, { headers: { 'Idempotency-Key': 'a' } }]) {
    const before = calls
    await assert.rejects(read.call(readId, { ...request, ...extra }), api.ContractError)
    assert.equal(calls, before)
  }
  await assert.rejects(responding(unavailable, 503).call(readId, request), api.ContractError)
  const errorRow = rows.find(row => row.operation === readId && row.status === 400)
  for (const reason of ['UNKNOWN_SERVICE_POLICY', 'EVENT_READ_ONLY']) {
    const body = structuredClone(errorRow.body); body.details.reason = reason
    await assert.rejects(responding(body, 400, errorRow).call(readId, request), api.ContractError)
  }
  const command = api.schemas.Domain_admin_ServicePolicyInput
  const publish = { expected_policy_revision: 0, reason: 'Policy publication', change: {
    action: 'PUBLISH', mode: 'NORMAL', minimum_client_version: '0.1.0', expires_at: '2030-01-01T00:01:00Z' } }
  const withdraw = { ...publish, change: { action: 'WITHDRAW' } }
  assert.equal(command.safeParse(publish).success, true); assert.equal(command.safeParse(withdraw).success, true)
  for (const invalid of [{ ...publish, expectedPolicyRevision: 0 }, { ...publish, expected_policy_revision: -1 },
    { ...publish, reason: ' ' }, { ...publish, change: { ...publish.change, minimum_client_version: '01.0.0' } },
    { ...publish, change: { ...publish.change, minimum_client_version: '0.1.0\n' } },
    { ...withdraw, change: { action: 'WITHDRAW', mode: 'NORMAL' } }]) assert.equal(command.safeParse(invalid).success, false)
  const pending = await responding(pendingRow.body, 202, pendingRow).call(writeId, { ...structuredClone(pendingRow.request), body: publish })
  assert.equal(pending.status, 202)
  const write = responding(writeRow.body, 200, writeRow)
  for (const change of [publish, withdraw]) {
    const input = { ...structuredClone(writeRow.request), body: change }
    await write.call(writeId, input)
    assert.equal(observed.init.method, 'PUT')
    assert.equal(observed.init.headers.get('Idempotency-Key'), input.headers['Idempotency-Key'])
    delete input.headers['Idempotency-Key']
    const before = calls
    await assert.rejects(write.call(writeId, input), api.ContractError)
    assert.equal(calls, before)
  }
})

test('every registered operation/status/media witness runs through the real generated client', async () => {
  assert.ok(rows.length > 100)
  assert.equal(new Set(rows.map(r => r.operation)).size, Object.keys(contract.operations).length)
  for (const row of rows) {
    let calls = 0
    const input = structuredClone(row.request)
    if (input.mediaType?.startsWith('image/')) input.body = new Blob(['C07 binary fixture'], { type: input.mediaType })
    const client = api.createClient({ baseUrl: 'https://fixture.invalid', getAccessToken: () => 'test-only-access-token', fetch: async (url, init) => {
      calls++
      const spec = contract.operations[row.operation]
      assert.equal(init.method, spec.method)
      assert.equal(init.redirect, 'manual')
      assert.equal(init.credentials, 'same-origin')
      assert.equal(init.headers.get('Authorization'), spec.security?.some(s => Object.hasOwn(s, 'BearerAuth')) ? 'Bearer test-only-access-token' : null)
      let expected = spec.path
      for (const p of spec.parameters.filter(p => p.in === 'path')) expected = expected.replace(`{${p.name}}`, encodeURIComponent(String(input.path[p.name])))
      assert.equal(new URL(url).pathname, expected)
      for (const p of spec.parameters.filter(p => p.in === 'query')) {
        const value = input.query[p.name]
        assert.deepEqual(new URL(url).searchParams.getAll(p.name), (Array.isArray(value) ? value : [value]).map(String))
      }
      if (input.mediaType === 'application/json') assert.deepEqual(JSON.parse(init.body), input.body)
      if (input.mediaType === 'application/x-www-form-urlencoded') assert.deepEqual(Object.fromEntries(init.body), input.body)
      if (input.mediaType?.startsWith('image/')) assert.ok(init.body instanceof Blob)
      const headers = new Headers(Object.fromEntries(Object.entries(row.headers).map(([k, v]) => [k, String(v)])))
      if (row.mediaType) headers.set('content-type', row.mediaType + (row.mediaType === 'application/json' ? '; charset=utf-8' : ''))
      const body = row.mediaType === null ? null : row.mediaType.startsWith('image/') ? new Blob(['bytes'], { type: row.mediaType }) : row.mediaType === 'application/json' ? JSON.stringify(row.body) : row.body
      return new Response(body, { status: row.status, headers })
    } })
    const result = await client.operations[row.operation](input)
    assert.equal(calls, 1)
    assert.equal(result.status, row.status)
    assert.equal(result.mediaType, row.mediaType)
    if (row.mediaType?.startsWith('image/')) assert.equal(await result.body.text(), 'bytes')
    else if (row.mediaType === null) assert.equal(result.body, undefined)
    else assert.deepEqual(result.body, row.body)
    assert.equal(Object.hasOwn(result.headers, 'Set-Cookie'), false)
  }
})

test('Zod agrees with retained independent Python JSON Schema decisions for every differential case', () => {
  assert.ok(compatibility.length > 500)
  const named = new Set(compatibility.filter(r => r.schema.$ref?.startsWith('#/components/schemas/')).map(r => r.schema.$ref.split('/').at(-1)))
  assert.deepEqual([...named].sort(), Object.keys(contract.components.schemas).sort())
  let accepted = 0, rejected = 0
  for (const row of compatibility) {
    assert.equal(api.accepts(row.schema, row.value), row.accept, JSON.stringify(row.schema) + ' / ' + JSON.stringify(row.value).slice(0, 120))
    if (row.schema.$ref?.startsWith('#/components/schemas/')) assert.equal(api.schemas[row.schema.$ref.split('/').at(-1)].safeParse(row.value).success, row.accept)
    row.accept ? accepted++ : rejected++
  }
  assert.ok(accepted > 0 && rejected > 0)
})

test('conditionals, exclusive branches, sibling assertions, UTF-8 bytes and JSON edge cases fail closed', () => {
  for (const [rule, value, expected] of [
    [{ oneOf: [{ type: 'integer' }, { type: 'number' }] }, 1, false],
    [{ type: 'object', properties: { state: { type: 'string' }, value: { type: 'integer' } }, required: ['state'], if: { properties: { state: { const: 'done' } } }, then: { required: ['value'] } }, { state: 'done' }, false],
    [{ type: 'array', uniqueItems: true }, [{ a: 1, b: 2 }, { b: 2, a: 1 }], false],
    [{ type: 'string', 'x-mcr-utf8-bytes': { minimum: 8, maximum: 128 } }, 'a'.repeat(7), false],
    [{ type: 'string', 'x-mcr-utf8-bytes': { minimum: 8, maximum: 128 } }, '가'.repeat(43), false],
    [{ type: 'string', 'x-mcr-utf8-bytes': { minimum: 8, maximum: 128 } }, '가'.repeat(3), true],
    [{ type: 'string', 'x-mcr-utf8-bytes': { minimum: 8, maximum: 128 } }, '\ud800'.repeat(8), false],
    [{ type: 'string', format: 'date-time' }, '2026-02-30T00:00:00Z', false],
    [{ type: 'string', format: 'date' }, '0000-01-01', false],
    [{ type: 'string', format: 'date' }, '0001-01-01', true],
    [{ type: 'string', format: 'uuid' }, '00000000-0000-0000-0000-000000000000\n', false],
    [{ type: 'string', format: 'uri' }, 'http://', true],
    [{ type: 'string', format: 'uri' }, 'https://한글.example/', false],
    [{ type: 'string', format: 'uri' }, 'https://example.test/[x]', false],
    [{ type: 'string', format: 'uri' }, 'http://[::1]/', true],
    [{ type: 'string', format: 'date-time' }, '2026-02-28T23:59:59.123456789+09:00', true],
    [{ type: 'number' }, NaN, false], [{ type: 'object' }, new Date(), false],
    [{ type: 'integer', minimum: 1, format: 'int64' }, 9007199254740992, true],
    [{ type: 'integer' }, 0.5, false],
    [{ type: 'object' }, { hidden: undefined }, false],
  ]) assert.equal(api.accepts(rule, value), expected)
  const schema = Object.keys(contract.components.schemas).find(n => contract.components.schemas[n].type === 'object' && contract.components.schemas[n].additionalProperties === false)
  assert.equal(api.schema(schema).safeParse({ __c07_unknown: true }).success, false)
  assert.throws(() => api.schema('__UNKNOWN__'))
})

test('request/response drift, malformed media, aborts and unexpected mock calls are observable failures', async () => {
  const row = rows.find(r => r.mediaType === 'application/json')
  let calls = 0
  const client = api.createClient({ baseUrl: 'https://fixture.invalid', fetch: async () => { calls++; return new Response('{', { status: 200, headers: { 'content-type': 'application/json' } }) } })
  await assert.rejects(() => client.call(row.operation, { ...row.request, unknown: true }), api.ContractError)
  assert.equal(calls, 0)
  const parameterRow = rows.find(r => contract.operations[r.operation].parameters.some(p => p.in === 'path' && p.required))
  const missingPath = structuredClone(parameterRow.request)
  const parameter = contract.operations[parameterRow.operation].parameters.find(p => p.in === 'path' && p.required)
  delete missingPath.path[parameter.name]
  await assert.rejects(() => client.call(parameterRow.operation, missingPath), api.ContractError)
  const bodyRow = rows.find(r => contract.operations[r.operation].requestBody?.required && r.request.mediaType === 'application/json')
  const missingBody = structuredClone(bodyRow.request); delete missingBody.body
  await assert.rejects(() => client.call(bodyRow.operation, missingBody), api.ContractError)
  assert.equal(calls, 0)
  await assert.rejects(() => client.call(row.operation, row.request), api.ContractError)
  await assert.rejects(() => client.call('__UNKNOWN__', {}), api.ContractError)
  for (const spec of [ { status: 599, body: '{}' }, { status: 200, body: '{}', headers: { 'content-type': 'text/plain' } } ]) {
    const wrong = api.createClient({ baseUrl: 'https://fixture.invalid', fetch: async () => new Response(spec.body, spec) })
    await assert.rejects(() => wrong.call(row.operation, row.request), api.ContractError)
  }
  const aborted = new AbortController(); aborted.abort(new Error('fixture abort'))
  const mock = api.createMock([])
  const canceled = api.createClient({ baseUrl: 'https://fixture.invalid', fetch: mock.fetch })
  await assert.rejects(() => canceled.call(row.operation, row.request, { signal: aborted.signal }), /fixture abort/)
  assert.equal(mock.requests.length, 0)
  await assert.rejects(() => canceled.call(row.operation, row.request), api.ContractError)
  assert.throws(() => api.createMock([{ method: 'GET', path: '/' }]).assertComplete())
})

test('clock and seed drive reproducible mock IDs, timestamps, expiry and scripted failures', async () => {
  function sequence(seed) { const clock = new api.FixtureClock('2030-01-01T00:00:00.000Z', seed); return [clock.now(), clock.id(), clock.id(), (clock.advance(1000), clock.now())] }
  assert.deepEqual(sequence(424242), sequence(424242))
  assert.notDeepEqual(sequence(424242), sequence(424243))
  const clock = new api.FixtureClock(), expires = Date.parse(clock.now()) + 1000
  clock.advance(1000); assert.equal(Date.parse(clock.now()), expires)
  assert.throws(() => clock.advance(-1)); assert.throws(() => new api.FixtureClock('invalid'))
  const mock = api.createMock([{ method: 'GET', path: '/fixture', status: 409, mediaType: 'application/json', body: { conflict: true }, advanceMs: 200 }, { method: 'GET', path: '/fixture', failure: 'network' }])
  assert.equal((await mock.fetch('https://fixture.invalid/fixture')).status, 409)
  await assert.rejects(() => mock.fetch('https://fixture.invalid/fixture'), /network failure/)
  assert.equal(mock.requests[0].at, '2030-01-01T00:00:00.000Z'); assert.equal(mock.requests[1].at, '2030-01-01T00:00:00.200Z')
  mock.assertComplete()
})

test('Enrollment negation excludes nil UUIDs and bounded traversal cannot invert into acceptance', () => {
  for (const [rule, value, expected] of [
    [{ not: true }, 0, false], [{ not: false }, 0, true],
    [{ not: { const: 0 } }, 0, false], [{ not: { const: 0 } }, 1, true],
    [{ type: 'string', not: { const: 'forbidden' } }, 'allowed', true],
    [{ type: 'string', not: { const: 'forbidden' } }, 'forbidden', false],
    [{ type: 'string', not: { const: 'forbidden' } }, 1, false],
    [{ type: 'object', not: { required: ['secret'] } }, {}, true],
    [{ type: 'object', not: { required: ['secret'] } }, { secret: null }, false],
    [{ anyOf: [{ not: { type: 'number' } }, { const: 1 }] }, 1, true],
    [{ anyOf: [{ not: { type: 'number' } }, { const: 1 }] }, 2, false],
    [{ if: { type: 'number' }, then: { not: { const: 0 } }, else: { not: { const: '' } } }, 0, false],
    [{ if: { type: 'number' }, then: { not: { const: 0 } }, else: { not: { const: '' } } }, 'x', true],
  ]) assert.equal(api.accepts(rule, value), expected)
  const nil = '00000000-0000-0000-0000-000000000000'
  const uuid = '00000000-0000-4000-8000-000000000001'
  const begin = { policyRevision: uuid }
  assert.equal(api.schemas.EnrollmentBeginRequest.safeParse(begin).success, true)
  assert.equal(api.schemas.EnrollmentBeginRequest.safeParse({ policyRevision: nil }).success, false)
  const ref = { not: { $ref: '#/components/schemas/EnrollmentBeginRequest' } }
  assert.equal(api.accepts(ref, begin), false)
  assert.equal(api.accepts(ref, { policyRevision: nil }), true)
  let exclusions = 0
  function walk(rule) {
    if (!rule || typeof rule !== 'object') return
    if (rule.not?.const === nil) {
      exclusions++
      assert.equal(api.accepts(rule, uuid), true)
      assert.equal(api.accepts(rule, nil), false)
    }
    for (const child of Object.values(rule)) walk(child)
  }
  for (const [name, rule] of Object.entries(contract.components.schemas)) if (name.startsWith('Enrollment')) walk(rule)
  assert.equal(exclusions, 16)
  let nested = { const: 0 }
  for (let index = 0; index < 40; index++) nested = { not: nested }
  assert.equal(api.accepts(nested, 0), true)
  for (let index = 0; index < 60; index++) nested = { not: nested }
  assert.equal(api.accepts(nested, 1), false)
  assert.equal(api.accepts({ not: { allOf: Array(100001).fill(true) } }, 0), false)
  const recursive = {}; recursive.not = recursive
  assert.equal(api.accepts(recursive, 0), false)
})

test('published C05 motion speed is positive and snapshot motion fields are paired own properties', () => {
  for (const invalid of [null, true, false, '0', [], {}, NaN, Infinity, -Infinity]) {
    assert.equal(api.accepts({ exclusiveMinimum: invalid }, 1), false)
  }
  for (const invalid of [null, [], true, { a: 'b' }, { a: [1] }, { a: ['b', 'b'] }, { a: [{}] }]) {
    assert.equal(api.accepts({ dependentRequired: invalid }, {}), false)
  }
  for (const [rule, value, expected] of [
    [{ type: 'number', exclusiveMinimum: 0, maximum: 1 }, 0, false],
    [{ type: 'number', exclusiveMinimum: 0, maximum: 1 }, -1, false],
    [{ type: 'number', exclusiveMinimum: 0, maximum: 1 }, Number.MIN_VALUE, true],
    [{ type: 'number', exclusiveMinimum: 0, maximum: 1 }, 1, true],
    [{ type: 'number', exclusiveMinimum: 0, maximum: 1 }, 1.000000001, false],
    [{ type: 'number', exclusiveMinimum: 0 }, true, false],
    [{ type: 'number', exclusiveMinimum: 0 }, '1', false],
    [{ type: 'number', exclusiveMinimum: 0 }, Infinity, false],
    [{ exclusiveMinimum: 0 }, 'numeric keyword alone does not require a number', true],
    [{ dependentRequired: { a: ['b'] } }, {}, true],
    [{ dependentRequired: { a: ['b'] } }, { a: null }, false],
    [{ dependentRequired: { a: ['b'] } }, { a: null, b: false }, true],
    [{ dependentRequired: { a: ['b'] } }, { b: 1 }, true],
    [{ dependentRequired: { constructor: ['companion'] } }, {}, true],
    [{ dependentRequired: { trigger: ['constructor'] } }, { trigger: 1 }, false],
    [{ dependentRequired: { trigger: ['constructor'] } }, { trigger: 1, constructor: 0 }, true],
  ]) assert.equal(api.accepts(rule, value), expected)
  const fixture = JSON.parse(readFileSync(path.join(process.env.C07_REPO_ROOT,
    'contracts/fixtures/realtime/motion-v1/negotiated.json'), 'utf8'))
  const snapshot = fixture.messages.find(frame => frame.message.type === 'snapshot').message
  const validator = api.schemas.Realtime_world_Snapshot
  assert.equal(validator.safeParse(snapshot).success, true)
  for (const missing of ['motionProfile', 'selfMotion']) {
    const unpaired = structuredClone(snapshot); delete unpaired[missing]
    assert.equal(validator.safeParse(unpaired).success, false)
  }
  const legacy = structuredClone(snapshot); delete legacy.motionProfile; delete legacy.selfMotion
  assert.equal(validator.safeParse(legacy).success, true)
  for (const speed of [0, -1, 1001, '1', Infinity]) {
    const invalid = structuredClone(snapshot); invalid.motionProfile.speedPxPerSecond = speed
    assert.equal(validator.safeParse(invalid).success, false)
  }
  const tiny = structuredClone(snapshot); tiny.motionProfile.speedPxPerSecond = Number.MIN_VALUE
  assert.equal(validator.safeParse(tiny).success, true)
})
