import { readFile, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'
import { root } from './runtime.mjs'
import { schemaAccepts } from './schema.mjs'
export const roles = ['owner', 'member', 'stranger', 'blocked', 'suspended', 'expired', 'admin']
export const domains = ['handle', 'claim', 'evidence', 'beacon', 'recipient']
const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/
function requireSetup(condition) { if (!condition) throw new Error('QA04 setup unavailable or invalid; no request dispatched') }
/** A seed-owner-authored plan is input data, not an invented application endpoint/role policy. */
export function qualifyPlan(plan, env, spec) {
  requireSetup(plan?.version === 1 && plan.isolatedEnvironment === true)
  const origin = new URL(plan.origin)
  requireSetup(origin.origin === plan.origin && !origin.username && !origin.password && (origin.protocol === 'https:' || origin.protocol === 'http:' && ['localhost', '127.0.0.1', '[::1]'].includes(origin.hostname)))
  for (const key of ['environmentId', 'seedRevision', 'serviceRevision', 'principalHandoff']) requireSetup(typeof plan[key] === 'string' && plan[key].length > 0)
  const principals = plan.principals
  requireSetup(principals && roles.every(role => uuid.test(principals[role]?.userId) && uuid.test(principals[role]?.eventId)
    && /^[A-Z][A-Z0-9_]+$/.test(principals[role]?.tokenEnv ?? '') && /^[A-Za-z0-9._~+/-]+=*$/.test(env[principals[role].tokenEnv] ?? '')))
  requireSetup(new Set(roles.map(role => principals[role].userId)).size === roles.length)
  requireSetup(Array.isArray(plan.httpCells) && plan.httpCells.length === 21)
  const known = new Map()
  for (const [path, methods] of Object.entries(spec.paths)) for (const [method, operation] of Object.entries(methods)) {
    if (operation.operationId) known.set(operation.operationId, { ...operation, path, method: method.toUpperCase() })
  }
  const keys = new Set()
  for (const cell of plan.httpCells) {
    requireSetup(roles.includes(cell.role) && ['list', 'detail', 'mutation'].includes(cell.operation))
    const key = cell.role + ':' + cell.operation
    requireSetup(!keys.has(key)); keys.add(key)
    requireSetup(Array.isArray(cell.probes) && cell.probes.length > 0)
    requireSetup(domains.every(domain => cell.probes.some(probe => probe.domains?.includes(domain))))
    for (const probe of cell.probes) {
      const operation = known.get(probe.operationId)
      requireSetup(operation && !!operation.responses[String(probe.expectedStatus)])
      requireSetup(cell.operation === 'mutation' ? !['GET', 'HEAD'].includes(operation.method) : operation.method === 'GET')
      requireSetup(Object.keys(probe.parameters ?? {}).every(name => operation.path.includes('{' + name + '}')))
      let path = operation.path.replace(/\{([^}]+)\}/g, (_, name) => {
        const value = probe.parameters?.[name]
        requireSetup(typeof value === 'string' && uuid.test(value))
        if (name === 'eventId') requireSetup(value === principals[cell.role].eventId)
        return value
      })
      const query = new URLSearchParams()
      const parameters = operation.parameters ?? []
      for (const [name, value] of Object.entries(probe.query ?? {})) {
        const parameter = parameters.find(parameter => parameter.in === 'query' && parameter.name === name)
        requireSetup(parameter && schemaAccepts(spec, parameter.schema, value))
        query.set(name, String(value))
      }
      requireSetup(parameters.filter(parameter => parameter.in === 'query' && parameter.required).every(parameter => Object.hasOwn(probe.query ?? {}, parameter.name)))
      if (query.size) path += '?' + query
      const responseSchema = operation.responses[String(probe.expectedStatus)].content?.['application/json']?.schema
      requireSetup(responseSchema)
      if (operation.requestBody?.required) requireSetup(probe.body !== undefined)
      if (probe.body !== undefined) requireSetup(operation.requestBody && schemaAccepts(spec, operation.requestBody.content['application/json'].schema, probe.body))
      requireSetup(Array.isArray(probe.forbiddenValues) && probe.forbiddenValues.length >= domains.length && probe.forbiddenValues.every(x => typeof x === 'string' && x.length >= 8))
      requireSetup(Array.isArray(probe.requiredValues) && probe.requiredValues.every(value => typeof value === 'string' && value.length >= 8) && (probe.expectedStatus >= 400 || probe.requiredValues.length > 0))
      if (operation.method !== 'GET') requireSetup(uuid.test(probe.idempotencyKey) && plan.allowSeedMutations === true)
      // No arbitrary URLs, fetch options, Authorization override or supplied WS messages accepted.
      probe.qualified = { path, method: operation.method, responseSchema }
    }
  }
  return plan
}

export async function executeHttp(plan, env, spec, fetchImpl = fetch) {
  const cells = []
  for (const cell of plan.httpCells) {
    for (const probe of cell.probes) {
      const headers = { Authorization: 'Bearer ' + env[plan.principals[cell.role].tokenEnv], Accept: 'application/json' }
      if (probe.qualified.method !== 'GET') Object.assign(headers, { 'Content-Type': 'application/json', 'Idempotency-Key': probe.idempotencyKey, 'X-MCR-Intent': 'browser', Origin: plan.origin })
      const response = await fetchImpl(plan.origin + probe.qualified.path, { method: probe.qualified.method, headers, cache: 'no-store', redirect: 'error',
        signal: AbortSignal.timeout(15000), ...(probe.body === undefined ? {} : { body: JSON.stringify(probe.body) }) })
      if (response.status !== probe.expectedStatus) throw new Error('QA04 HTTP status mismatch; payload withheld')
      if (!/(?:^|,)\s*no-store\s*(?:,|$)/i.test(response.headers.get('cache-control') ?? '')) throw new Error('QA04 private response lacks no-store')
      const reader = response.body?.getReader()
      if (!reader) throw new Error('QA04 missing response body')
      const chunks = []; let bytes = 0
      try {
        while (true) {
          const next = await reader.read()
          if (next.done) break
          bytes += next.value.byteLength
          if (bytes > 1048576) { await reader.cancel(); throw new Error('QA04 response exceeds qualification bound') }
          chunks.push(next.value)
        }
      } finally { reader.releaseLock() }
      const text = Buffer.concat(chunks).toString('utf8')
      const value = JSON.parse(text)
      if (!schemaAccepts(spec, probe.qualified.responseSchema, value)) throw new Error('QA04 frozen response schema mismatch')
      const decoded = JSON.stringify(value)
      if (probe.forbiddenValues.some(marker => decoded.includes(marker)) || !probe.requiredValues.every(marker => decoded.includes(marker))) throw new Error('QA04 private disclosure/content assertion failed; payload withheld')
    }
    cells.push({ role: cell.role, operation: cell.operation, result: 'PASS', probes: cell.probes.length, provenance: fetchImpl === fetch ? 'REAL_HTTP' : 'SYNTHETIC_GATE_SELFTEST' })
  }
  return cells
}

async function main() {
  const out = resolve(root, 'docs/mcr/evidence/privacy-frontend/live-report.json')
  const report = { task: 'QA04', result: 'NOT_RUN', originalTask: 'IN_PROGRESS', HTTP: 'NOT_RUN', WS: 'NOT_RUN', cachedProductUI: 'NOT_RUN', credentialOrPayloadLogging: false }
  try {
    if (!process.env.MCR_QA04_REAL_PLAN) throw new Error('Missing MCR_QA04_REAL_PLAN and qualified principals/seed/public services')
    const spec = JSON.parse(await readFile(resolve(root, 'contracts/generated/mcr-domain.openapi.json'), 'utf8'))
    const plan = qualifyPlan(JSON.parse(await readFile(process.env.MCR_QA04_REAL_PLAN, 'utf8')), process.env, spec)
    // Full gate refuses unsupported WS/cache setup BEFORE mutating any seed.
    if (!process.argv.includes('--http-only')) throw new Error('Full QA04 WS/revocation/cache public-service handoff unavailable; HTTP-only execution must be explicit')
    report.HTTP = 'IN_PROGRESS'
    report.cells = await executeHttp(plan, process.env, spec)
    report.HTTP = 'PASS'; report.result = 'PARTIAL'; report.serviceRevision = plan.serviceRevision; report.seedRevision = plan.seedRevision
    report.reason = 'Real HTTP only; original QA04 WS/cache requirements remain OPEN'
    process.exitCode = 2
  } catch (error) {
    report.reason = error.message.startsWith('Missing ') || error.message.startsWith('Full QA04 ') ? error.message : 'Setup refused or real HTTP assertion failed; private detail withheld'
    report.result = report.HTTP === 'NOT_RUN' ? 'NOT_RUN' : 'FAIL'
    if (report.HTTP === 'IN_PROGRESS') report.HTTP = 'FAIL'
    process.exitCode = 2
  } finally {
    await writeFile(out, JSON.stringify(report, null, 2) + '\n')
    console.log(JSON.stringify(report))
  }
}
if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) await main()
