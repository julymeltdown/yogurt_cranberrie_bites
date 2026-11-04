import { closeSync, constants, fstatSync, openSync, readFileSync } from 'node:fs'
import { pathToFileURL } from 'node:url'

const uuid = value => typeof value === 'string' && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(value) && !value.startsWith('00000000-')
const record = value => value !== null && typeof value === 'object' && !Array.isArray(value)
const revision = value => typeof value === 'string' && /^[0-9a-f]{40}$/.test(value)
const keys = (value, names) => record(value) && Object.keys(value).length === names.length && names.every(name => Object.hasOwn(value, name))
const roles = ['requester', 'recipient', 'restricted']
const timestamp = value => typeof value === 'string' && /^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:\.\d{1,6})?(?:Z|\+00:00)$/.test(value)
  && Number.isFinite(Date.parse(value)) && new Date(value).toISOString().slice(0, 19) === value.slice(0, 19)

/** Correlated private preflight metadata only; reported 200s never prove current HTTP authority. */
export function admitLocalAuthProducer(config, now = Date.now()) {
  try {
    if (!record(config) || !record(config.source) || !record(config.principals)) return false
    const producer = config.authProducer, runtime = config.runtime
    if (!keys(producer, ['version', 'kind', 'sourceRevision', 'binaryRevision', 'tool', 'mailOwner', 'controlOwner', 'origin', 'eventId', 'issuedAt', 'expiresAt', 'custody', 'actors'])
      || !keys(runtime, ['controlOwner', 'origin', 'eventId', 'expiresAt', 'private'])) return false
    if (producer.version !== 'mcr-local-real-auth/v1' || producer.kind !== 'CANONICAL_GATEWAY_PRIVATE_HTTP_MAIL'
      || producer.tool !== 'world_in01_prepare.py' || producer.custody !== 'PRIVATE_MODE_0600' || runtime.private !== true) return false
    if (!revision(producer.sourceRevision) || !revision(producer.binaryRevision)
      || producer.sourceRevision !== config.source.authProducerRevision || producer.binaryRevision !== config.source.authBinaryRevision) return false
    const origin = new URL(config.origin)
    if (!['http:', 'https:'].includes(origin.protocol) || !['127.0.0.1', 'localhost', '[::1]'].includes(origin.hostname)
      || origin.username || origin.password || origin.pathname !== '/' || origin.search || origin.hash
      || producer.origin !== config.origin || runtime.origin !== config.origin) return false
    if (!uuid(config.eventId) || producer.eventId !== config.eventId || runtime.eventId !== config.eventId) return false
    if (typeof producer.controlOwner !== 'string' || producer.controlOwner.length < 16 || producer.controlOwner.length > 256
      || producer.mailOwner !== producer.controlOwner || runtime.controlOwner !== producer.controlOwner) return false
    if (!timestamp(producer.issuedAt) || !timestamp(producer.expiresAt) || runtime.expiresAt !== producer.expiresAt) return false
    const issued = Date.parse(producer.issuedAt), expires = Date.parse(producer.expiresAt)
    if (!Number.isFinite(now) || issued > now || expires <= now || expires <= issued) return false
    if (!keys(producer.actors, roles) || !keys(config.principals, roles)) return false
    const users = new Set(), emails = new Set()
    for (const role of roles) {
      const actor = producer.actors[role], principal = config.principals[role]
      if (!keys(actor, ['userId', 'eventId', 'email', 'method', 'signupStatus', 'verifyStatus', 'loginStatus', 'bootstrapStatus']) || !keys(principal, ['userId', 'email', 'password'])) return false
      if (!uuid(actor.userId) || actor.userId !== principal.userId || actor.eventId !== config.eventId || actor.email !== principal.email
        || typeof actor.email !== 'string' || !/^[a-zA-Z0-9.!#$%&'*+/=?^_`{|}~-]+@example\.test$/.test(actor.email)
        || /fixture|synthetic|mock/i.test(actor.email) || actor.method !== 'GATEWAY_SIGNUP_HTTP_MAIL_VERIFY_LOGIN'
        || [actor.signupStatus, actor.verifyStatus, actor.loginStatus, actor.bootstrapStatus].some(status => status !== 200)) return false
      users.add(actor.userId); emails.add(actor.email.toLowerCase())
    }
    return users.size === 3 && emails.size === 3
  } catch { return false }
}
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
  const localAuth = admitLocalAuthProducer(config)
  need(localAuth, 'canonical real Auth producer provenance', 'Auth / backend runtime owner')
  for (const role of ['requester', 'recipient', 'restricted']) {
    const p = record(users[role]) ? users[role] : undefined
    need(uuid(p?.userId) && typeof p?.email === 'string' && p.email.includes('@')
      && localAuth && !/fixture|synthetic|\.invalid$/i.test(p.email) && typeof p?.password === 'string' && p.password.length > 0,
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
  let descriptor
  try {
    if (!path) return undefined
    descriptor = openSync(path, constants.O_RDONLY | constants.O_NOFOLLOW)
    const info = fstatSync(descriptor)
    if (!info.isFile() || (info.mode & 0o777) !== 0o600 || info.size > 128 * 1024
      || (typeof process.getuid === 'function' && info.uid !== process.getuid())) throw Error('Private config required')
    return JSON.parse(readFileSync(descriptor, 'utf8'))
  }
  catch { throw new Error('NOT_RUN: private real configuration unreadable') }
  finally { if (descriptor !== undefined) closeSync(descriptor) }
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
    claim: 'private producer preflight only; current HTTP, availability and full IN03 remain unqualified', missing }
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    const result = admitHTTP(readRunConfig(process.env.MCR_C07_REAL_CONFIG))
    process.stdout.write(JSON.stringify(result, null, 2) + '\n')
    process.exitCode = result.status === 'INPUTS_PRESENT' ? 0 : 2
  } catch { process.stderr.write('NOT_RUN: real configuration unreadable; no browser or login started\n'); process.exitCode = 2 }
}
