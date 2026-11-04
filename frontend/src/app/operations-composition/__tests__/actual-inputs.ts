import assert from 'node:assert/strict'
import { constants } from 'node:fs'
import { open } from 'node:fs/promises'
import { isAbsolute } from 'node:path'
import { createHash } from 'node:crypto'
import { Contracts, loadConfig, type GateConfig } from '../../../../e2e/real/mural-helpers'

// Qualified owner protocol2653376e5; this module observes the existing managed
// runtime. It never launches services, publishes a role, or fabricates a proof.
export const backendObserverSha256 = '7d29fee99b5eab4b3e3320ba5a4513f803d6d2b450da33246fa467767deba6ad'
type FrontendHandoff = { candidate: string; environmentId: string; eventId: string; origin: string; settingsSha256: string; token: string; phaserModulePath: string }
type FrontendObservation = Omit<FrontendHandoff, 'token' | 'phaserModulePath'> & { expiresAt: string; phaserModuleUrl: string }
type BackendObservation = { candidate: string; environmentId: string; eventId: string; domainHash: string; realtimeHash: string; expiresAt: string; actors: { userId: string; role: string }[] }
function need(value: unknown, code: string): asserts value { assert(value, code) }
const hash = (bytes: Uint8Array) => createHash('sha256').update(bytes).digest('hex')
export async function ownedInput(path: string | undefined, privateOnly: boolean, maximum = 32768) {
  need(path && isAbsolute(path), 'ACTUAL_PRIVATE_INPUT_REQUIRED')
  const file = await open(path, constants.O_RDONLY | constants.O_NOFOLLOW | constants.O_NONBLOCK)
  try {
    const before = await file.stat()
    need(before.isFile() && before.uid === process.getuid?.() && !(before.mode & (privateOnly ? 0o077 : 0o022)) && before.size > 0 && before.size <= maximum, 'OWNED_BOUNDED_INPUT_REQUIRED')
    const bytes = await file.readFile(), after = await file.stat()
    need(before.dev === after.dev && before.ino === after.ino && before.size === after.size && before.mtimeMs === after.mtimeMs && bytes.length === before.size, 'IMMUTABLE_INPUT_REQUIRED')
    return bytes
  } finally { await file.close() }
}
export function validateFrontendHandoff(config: GateConfig, value: unknown): FrontendHandoff {
  need(value && typeof value === 'object' && !Array.isArray(value), 'ACTUAL_FRONTEND_HANDOFF_REQUIRED')
  const h = value as FrontendHandoff
  need(Object.keys(h).sort().join(',') === ['candidate','environmentId','eventId','origin','settingsSha256','token','phaserModulePath'].sort().join(','), 'CLOSED_FRONTEND_HANDOFF_REQUIRED')
  const u = new URL(h.origin)
  need(u.origin === h.origin && u.protocol === 'http:' && ['localhost','127.0.0.1','[::1]'].includes(u.hostname) && !!u.port, 'MANAGED_LOOPBACK_FRONTEND_REQUIRED')
  need(h.candidate === config.releaseRevision && h.environmentId === config.environmentId && h.eventId === config.eventId && h.origin === config.origin, 'FRONTEND_SCOPE_MISMATCH')
  need(/^[a-f0-9]{64}$/.test(h.settingsSha256) && /^[a-f0-9]{64}$/.test(h.token) && typeof h.phaserModulePath === 'string' && /^\/@fs\/.*\/deps\/phaser\.js$/.test(h.phaserModulePath), 'ACTUAL_FRONTEND_BINDING_REQUIRED')
  return h
}
export function validateFrontendObservation(h: FrontendHandoff, value: unknown, now = Date.now()) {
  need(value && typeof value === 'object', 'ACTUAL_FRONTEND_OBSERVATION_REQUIRED')
  const live = value as FrontendObservation
  need(['candidate','environmentId','eventId','origin','settingsSha256'].every(k => live[k as keyof FrontendObservation] === h[k as keyof FrontendHandoff]), 'CURRENT_FRONTEND_BINDING_REQUIRED')
  const expiry = Date.parse(live.expiresAt)
  need(expiry > now && expiry <= now + 60000, 'FRESH_FRONTEND_OBSERVATION_REQUIRED')
  const module = new URL(live.phaserModuleUrl)
  need(module.origin === h.origin && module.pathname === h.phaserModulePath && /^\?v=[a-f0-9]+$/.test(module.search), 'SAME_APP_FRONTEND_OBSERVATION_REQUIRED')
}
export function validateBackendObservation(config: GateConfig, value: BackendObservation, now = Date.now()) {
  need(value && value.candidate === config.releaseRevision && value.environmentId === config.environmentId && value.eventId === config.eventId
    && value.domainHash === config.contractHashes.domain && value.realtimeHash === config.contractHashes.realtime, 'CURRENT_BACKEND_SCOPE_REQUIRED')
  const expiry = Date.parse(value.expiresAt)
  need(expiry > now && expiry <= now + 15 * 60000, 'FRESH_BACKEND_OBSERVATION_REQUIRED')
  need(Array.isArray(value.actors) && value.actors.length === 3 && new Set(value.actors.map(a => a.userId)).size === 3, 'CURRENT_THREE_ACTOR_OBSERVATION_REQUIRED')
  need(config.members.every(a => value.actors.some(p => p.userId === a.userId && p.role === 'MEMBER'))
    && value.actors.some(p => p.userId === config.operator.userId && p.role === 'OPERATOR'), 'OBSERVED_ACTOR_BINDING_REQUIRED')
  // The browser's actual OwnAccess response remains the UI authority. Config
  // actor labels and this deployment observer never grant frontend controls.
}
export async function actualInputs() {
  const config = await loadConfig()
  const h = validateFrontendHandoff(config, JSON.parse((await ownedInput(process.env.MCR_ADMIN_FRONTEND_HANDOFF, true)).toString()))
  await ownedInput(process.env.MCR_IN04_BACKEND_DESCRIPTOR, true, 8192)
  const bytes = await ownedInput(process.env.MCR_ADMIN_BACKEND_OBSERVER, false, 1024 * 1024)
  need(hash(bytes) === backendObserverSha256, 'QUALIFIED_OWNER_OBSERVER_HASH_REQUIRED')
  const module = await import('data:text/javascript;base64,' + bytes.toString('base64')) as { runtimeQualification?: { inspect(c: GateConfig): Promise<BackendObservation> } }
  need(module.runtimeQualification && typeof module.runtimeQualification.inspect === 'function', 'ACTUAL_OWNER_OBSERVER_REQUIRED')
  const port = module.runtimeQualification
  const observe = async () => {
    const response = await fetch(h.origin + '/.mcr-in04/frontend', { headers: { 'x-mcr-in04-observer': h.token }, cache: 'no-store', signal: AbortSignal.timeout(12000) })
    need(response.ok, 'ACTUAL_FRONTEND_NOT_SERVING')
    validateFrontendObservation(h, await response.json())
    validateBackendObservation(config, await port.inspect(config))
  }
  await observe()
  return { config, observe }
}

/** The shared decoder predates oneOf. Translate only this verified disjoint
 * null-or-closed-object union; preserve every original schema and closed field. */
export function qualifyAccessContracts(base: Contracts): Contracts {
  const object = (v: unknown): v is Record<string, unknown> => !!v && typeof v === 'object' && !Array.isArray(v)
  const name = 'Domain_admin_OwnOperationsAccessV1', leaseName = 'Domain_admin_OperationsAccessLeaseV1'
  const root = base.schemas[name], lease = base.schemas[leaseName]
  need(object(root) && root.type === 'object' && root.additionalProperties === false && object(root.properties), 'EXACT_OWN_ACCESS_SCHEMA_REQUIRED')
  const access = root.properties.access
  need(object(access) && Object.keys(access).join(',') === 'oneOf' && Array.isArray(access.oneOf) && access.oneOf.length === 2, 'EXACT_NULLABLE_ACCESS_UNION_REQUIRED')
  const [absent, present] = access.oneOf
  need(object(absent) && Object.keys(absent).join(',') === 'type' && absent.type === 'null'
    && object(present) && Object.keys(present).join(',') === '$ref' && present.$ref === '#/components/schemas/' + leaseName
    && object(lease) && lease.type === 'object' && lease.additionalProperties === false, 'DISJOINT_NULL_OR_CLOSED_LEASE_REQUIRED')
  return new Contracts(base.operations, { ...base.schemas, [name]: { ...root, properties: { ...root.properties, access: { anyOf: access.oneOf } } } })
}
