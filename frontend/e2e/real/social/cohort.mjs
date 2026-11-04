import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { resolve } from 'node:path'
import assert from 'node:assert/strict'

export const base = '9e7952fcd89360f7b4fe43c69f22060b9aa55c74'
export const root = fileURLToPath(new URL('../../../../', import.meta.url))
export const readJSON = path => JSON.parse(readFileSync(resolve(root, path), 'utf8'))
export const operationIds = Object.freeze([
  'listConcertIntros', 'getConcertIntro',
  'listConcertFeed', 'getConcertPost', 'listConcertComments', 'createConcertComment',
  'listOwnConcertHandles', 'getConcertSharingOffer', 'createConcertSocialRequest',
  'listConcertSocialRequests', 'getConcertSocialRequest', 'decideConcertSocialRequest',
  'cancelConcertSocialRequest', 'revokeConcertSocialRequest', 'revealConcertSocialHandle',
  'listConcertNotifications', 'getConcertNotificationUnreadCount', 'markConcertNotification',
  'markAllConcertNotifications', 'listConcertBlocks', 'blockConcertUser', 'unblockConcertUser',
])

/** Public source comparison only. Never a generator, response decoder or route authority. */
export function assertOperations(metadata, operations = readJSON('contracts/domain/v1/operations.json'),
  openapi = readJSON('contracts/generated/mcr-domain.openapi.json')) {
  for (const id of operationIds) {
    const source = operations.find(row => row.operationId === id)
    assert.ok(source, `C07 missing public operation ${id}`)
    const actual = metadata(id)
    assert.ok(actual, `C07 missing retained metadata ${id}`)
    const prefix = `C07 metadata drift ${id}`
    assert.equal(actual.method, source.method.toUpperCase(), `${prefix}: method`)
    assert.equal(actual.path, source.path, `${prefix}: path`)
    assert.equal(actual.query, source.query_schema, `${prefix}: query`)
    assert.equal(actual.idempotency, source.idempotency_key, `${prefix}: idempotency`)
    assert.deepEqual(actual.errors, source.errors, `${prefix}: errors`)
    assert.deepEqual(actual.success, [source.success_status, ...(source.pending_status ? [source.pending_status] : [])], `${prefix}: status`)
    assert.deepEqual(Object.keys(actual.parameters).sort(), Object.keys(source.path_parameters).sort(), `${prefix}: path parameters`)
    for (const [name, ref] of Object.entries(source.path_parameters)) {
      const [url, pointer] = ref.$ref.split('#')
      const file = 'contracts/' + new URL(url).pathname.split('/').slice(1).join('/')
      let schema = readJSON(file)
      for (const part of pointer.split('/').slice(1)) schema = schema[part]
      assert.deepEqual(actual.parameters[name], schema, `${prefix}: scalar ${name}`)
    }
    const wire = openapi.paths[source.path]?.[source.method]
    assert.equal(wire?.operationId, id, `${prefix}: OpenAPI operation`)
    const response = wire.responses[String(source.success_status)]?.content?.['application/json']?.schema
    assert.equal(response?.$ref, '#/components/schemas/Domain_' + source.response.replaceAll('.', '_'), `${prefix}: response`)
  }
}

/** Exact indexed fixtures and public contract inputs; hashes do not certify all their semantics. */
export function inputManifest() {
  const files = new Set([
    'contracts/generated/mcr-core.openapi.json', 'contracts/generated/mcr-domain.openapi.json',
    'contracts/core/v1/core.schema.json', 'contracts/domain/v1/operations.json',
    'contracts/fixtures/core/index.json', 'contracts/fixtures/domain/index.json',
    'contracts/realtime/v1/policy.json', 'contracts/realtime/v1/world.schema.json', 'contracts/realtime/v1/operations.json',
    'frontend/src/shared/api/domain-operations.generated.ts', 'frontend/pnpm-lock.yaml',
  ])
  const visit = path => {
    if (files.has(path) && !path.endsWith('operations.json')) return
    files.add(path)
    const walk = value => {
      if (!value || typeof value !== 'object') return
      if (typeof value.$ref === 'string' && value.$ref.startsWith('https://contracts.mcr.invalid/')) {
        const next = 'contracts' + new URL(value.$ref).pathname
        if (!files.has(next)) visit(next)
      }
      for (const child of Object.values(value)) walk(child)
    }
    walk(readJSON(path))
  }
  // Selected operation request/response/query schemas and recursively referenced public definitions.
  for (const op of readJSON('contracts/domain/v1/operations.json').filter(row => operationIds.includes(row.operationId))) {
    for (const key of ['request', 'response', 'query_schema', 'pending_response']) {
      if (op[key]) visit(`contracts/domain/v1/${op[key].split('.')[0]}.schema.json`)
    }
  }
  for (const lane of ['core', 'domain']) for (const row of readJSON(`contracts/fixtures/${lane}/index.json`)) files.add(`contracts/fixtures/${lane}/${row.file}`)
  return { base, status: { C07: 'UNRECONCILED', IN03: 'NOT_RUN' }, inputs: [...files].sort().map(path => ({ path,
    sha256: createHash('sha256').update(readFileSync(resolve(root, path))).digest('hex') })) }
}
