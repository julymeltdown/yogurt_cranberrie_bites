import { readFileSync } from 'node:fs'
import { pathToFileURL } from 'node:url'

const uuid = value => typeof value === 'string' && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(value) && !value.startsWith('00000000-')
const record = value => value !== null && typeof value === 'object' && !Array.isArray(value)
/** No credentials or config values appear in diagnostics. There is no fixture fallback. */
function admitInputs(config, resourceKeys) {
  config = record(config) ? config : {}
  const missing = []
  const need = (ok, input, owner) => { if (!ok) missing.push({ input, owner }) }
  need(config?.mode === 'real', 'explicit real mode', 'IN03 runtime owner')
  let origin
  try { origin = new URL(config?.origin); if (!['https:', 'http:'].includes(origin.protocol) || origin.username || origin.password || origin.pathname !== '/' || origin.search || origin.hash) origin = undefined } catch { /* Report the field, never its value. */ }
  need(Boolean(origin), 'same-origin deployment URL', 'Root153 / infrastructure')
  need(uuid(config?.eventId), 'non-fixture event UUID', 'backend social owner')
  need(typeof config?.availabilityModule === 'string' && /^\/src\/[^?#]+\.tsx?$/.test(config.availabilityModule)
    && !/fixture|__tests__|mock|\.\./i.test(config.availabilityModule), 'reviewed production availability module', 'Root153 frontend composition')
  need(typeof config?.availabilityRevision === 'string' && /^[0-9a-f]{40}$/.test(config.availabilityRevision), 'availability source revision', 'Root153 frontend composition')
  const users = record(config.principals) ? config.principals : {}
  for (const role of ['requester', 'recipient', 'restricted']) {
    const p = record(users[role]) ? users[role] : undefined
    need(uuid(p?.userId) && typeof p?.email === 'string' && p.email.includes('@')
      && !/fixture|synthetic|example\.|\.invalid$/i.test(p.email) && typeof p?.password === 'string' && p.password.length > 0,
    `${role} provisioned login`, 'Auth / backend social owner')
    need(!p || (!('accessToken' in p) && !('token' in p)), `${role} cookie-login-only credentials`, 'Auth owner')
  }
  need(new Set(['requester', 'recipient', 'restricted'].map(role => users[role]?.userId)).size === 3, 'three distinct principals', 'backend social owner')
  for (const key of resourceKeys) need(uuid(config?.resources?.[key]), `admitted ${key}`, 'backend social owner')
  need(config?.qualifiedRefresh === undefined || config.qualifiedRefresh === false, 'qualifiedRefresh=false for this cohort', 'Auth owner')
  return { status: missing.length ? 'NOT_RUN' : 'INPUTS_PRESENT', claim: 'HTTP controller preparation only', missing,
    IN03: 'NOT_RUN', unavailable: [
      { input: 'admitted social block/revoke WS producer and public message contract', owner: 'Root153 / C05 backend owner' },
      { input: 'reviewed age-provider restricted-principal provenance', owner: 'age provider / backend social owner' },
    ] }
}
export function admitHTTP(config) { return admitInputs(config, ['introId', 'postId', 'approvedRequestId']) }
export function readRunConfig(path) {
  try { return path ? JSON.parse(readFileSync(path, 'utf8')) : undefined }
  catch { throw new Error('NOT_RUN: private real configuration unreadable') }
}
/** Admission is not qualification. Original WS/cache cells require actual producer evidence. */
export function admitOriginal(config) {
  // Original journey creates its own consent request; an unrelated pre-approved request is unnecessary.
  const previous = admitInputs(config, ['introId', 'postId']), missing = [...previous.missing]
  for (const key of ['frontendRevision', 'backendRevision', 'contractRevision']) {
    if (typeof config?.source?.[key] !== 'string' || !/^[0-9a-f]{40}$/.test(config.source[key]))
      missing.push({ input: `actual ${key}`, owner: 'named publishing/runtime owner' })
  }
  if (!uuid(config?.resources?.expiredRequestId)) missing.push({ input: 'server-expired request identity', owner: 'backend Social owner' })
  for (const key of ['unrelated', 'expired', 'revoked']) {
    const statuses = config?.denials?.[key]
    if (!Array.isArray(statuses) || !statuses.length || !statuses.every(value => [403, 404, 410].includes(value)))
      missing.push({ input: `contract-qualified ${key} denial statuses`, owner: 'backend scenario authority' })
  }
  return { ...previous, status: missing.length ? 'NOT_RUN' : 'INPUTS_PRESENT',
    claim: 'actual public controller segments; full original IN03 remains unqualified', missing }
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    const result = admitHTTP(readRunConfig(process.env.MCR_C07_REAL_CONFIG))
    process.stdout.write(JSON.stringify(result, null, 2) + '\n')
    process.exitCode = result.status === 'INPUTS_PRESENT' ? 0 : 2
  } catch { process.stderr.write('NOT_RUN: real configuration unreadable; no browser or login started\n'); process.exitCode = 2 }
}
