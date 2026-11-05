import { createHash } from 'node:crypto'
import { open, lstat } from 'node:fs/promises'
import { constants } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { dirname, isAbsolute, relative, resolve } from 'node:path'
import { loadConfig as loadWorldConfig, Prerequisite as WorldPrerequisite, type GateConfig } from './world-gate/config.ts'

export type Status = 'PASS' | 'FAIL' | 'NOT_RUN'
export class Prerequisite extends Error {
  code: string
  constructor(code: string) { super('IN02 prerequisite unavailable'); this.code = code }
}
export class GateFailure extends Error {
  code: string
  constructor(code: string) { super('IN02 assertion failed'); this.code = code }
}
export function verify(value: unknown, code: string): asserts value { if (!value) throw new GateFailure(code) }
export function requirePrerequisite(value: unknown, code: string): asserts value { if (!value) throw new Prerequisite(code) }
export const sha256 = (value: string | Uint8Array) => createHash('sha256').update(value).digest('hex')
export const record = (v: unknown): v is Record<string, unknown> => !!v && typeof v === 'object' && !Array.isArray(v)
export const uuid = (v: unknown): v is string => typeof v === 'string' && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(v)
const revision = (v: unknown): v is number => typeof v === 'number' && Number.isSafeInteger(v) && v > 0
const timestamp = (v: unknown): v is string => typeof v === 'string' && /^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:\.\d+)?Z$/.test(v)
  && Number.isFinite(Date.parse(v)) && new Date(v).toISOString().slice(0, 19) === v.slice(0, 19)
const exact = (v: Record<string, unknown>, keys: string[]) => Object.keys(v).length === keys.length && keys.every(k => Object.hasOwn(v, k))
export interface ReceiptFile { path: string; sha256: string }
export interface InputConfig {
  version: 1; worldConfigPath: string; prerequisiteReceipt: ReceiptFile; partyId: string; giveawayId: string
  place: { id: string; mapRevision: number; placeRevision: number }
  meeting: { instructions: string; identification: string; ttlSeconds: number }
}
export type JourneyWorld = GateConfig | (Omit<GateConfig, 'version' | 'artReview'> & { version: 2 })
export interface Config extends InputConfig { world: JourneyWorld; qualificationExpiresAt: number; prerequisiteHash: string }
/** The original journey needs the actual C03/C05 cohort, not completed IN01 art,
 * theme or customization acceptance. Version 1 remains readable as history.
 */
export function parseJourneyWorld(v: unknown): JourneyWorld {
  requirePrerequisite(record(v) && exact(v, ['version', 'isolated', 'origin', 'eventId', 'releaseRevision', 'accounts', 'serviceReceipt'])
    && v.version === 2 && v.isolated === true && uuid(v.eventId) && typeof v.releaseRevision === 'string' && /^[0-9a-f]{40}$/.test(v.releaseRevision), 'INVALID_JOURNEY_WORLD_CONFIG')
  let url: URL
  try { url = new URL(String(v.origin)) } catch { throw new Prerequisite('INVALID_JOURNEY_ORIGIN') }
  requirePrerequisite(url.origin === v.origin && !url.username && !url.password && (url.protocol === 'https:'
    || url.protocol === 'http:' && ['127.0.0.1', 'localhost', '[::1]'].includes(url.hostname)), 'INVALID_JOURNEY_ORIGIN')
  requirePrerequisite(Array.isArray(v.accounts) && v.accounts.length === 2, 'TWO_PROVISIONED_ACCOUNTS_REQUIRED')
  for (const account of v.accounts) requirePrerequisite(record(account) && exact(account, ['email', 'password', 'userId']) && uuid(account.userId)
    && typeof account.email === 'string' && /^[^\s@]+@[^\s@]+$/.test(account.email) && typeof account.password === 'string' && account.password.length >= 8 && account.password.length <= 512, 'INVALID_JOURNEY_ACCOUNT')
  const [owner, recipient] = v.accounts as GateConfig['accounts']
  requirePrerequisite(owner.userId !== recipient.userId && owner.email.toLowerCase() !== recipient.email.toLowerCase(), 'ACCOUNTS_MUST_BE_INDEPENDENT')
  const receipt = v.serviceReceipt
  requirePrerequisite(record(receipt) && exact(receipt, ['path', 'sha256']) && typeof receipt.path === 'string' && isAbsolute(receipt.path)
    && typeof receipt.sha256 === 'string' && /^[0-9a-f]{64}$/.test(receipt.sha256), 'PINNED_SERVICE_COHORT_REQUIRED')
  return v as unknown as JourneyWorld
}
export function parseInput(v: unknown): InputConfig {
  requirePrerequisite(record(v) && exact(v, ['version', 'worldConfigPath', 'prerequisiteReceipt', 'partyId', 'giveawayId', 'place', 'meeting']), 'INVALID_CONFIG')
  requirePrerequisite(v.version === 1 && typeof v.worldConfigPath === 'string' && isAbsolute(v.worldConfigPath) && uuid(v.partyId) && uuid(v.giveawayId), 'INVALID_CONFIG_BINDING')
  const p = v.place, m = v.meeting, r = v.prerequisiteReceipt
  requirePrerequisite(record(p) && exact(p, ['id', 'mapRevision', 'placeRevision']) && uuid(p.id) && revision(p.mapRevision) && revision(p.placeRevision), 'QUALIFIED_PLACE_REQUIRED')
  requirePrerequisite(record(m) && exact(m, ['instructions', 'identification', 'ttlSeconds']) && typeof m.instructions === 'string' && m.instructions.trim().length > 0
    && Array.from(m.instructions).length <= 1000 && typeof m.identification === 'string' && m.identification.trim().length > 0 && Array.from(m.identification).length <= 240
    && Number.isInteger(m.ttlSeconds) && Number(m.ttlSeconds) >= 120 && Number(m.ttlSeconds) <= 900, 'REVIEWED_MEETING_REQUIRED')
  requirePrerequisite(record(r) && exact(r, ['path', 'sha256']) && typeof r.path === 'string' && isAbsolute(r.path) && typeof r.sha256 === 'string' && /^[0-9a-f]{64}$/.test(r.sha256), 'PREREQUISITE_RECEIPT_REQUIRED')
  return v as unknown as InputConfig
}
async function privateBytes(path: string, max = 32768) {
  requirePrerequisite(isAbsolute(path), 'PRIVATE_ABSOLUTE_CONFIG_REQUIRED')
  try {
    const handle = await open(path, constants.O_RDONLY | constants.O_NOFOLLOW)
    try {
      const s = await handle.stat()
      requirePrerequisite(s.isFile() && s.uid === process.getuid?.() && !(s.mode & 0o077) && s.size <= max, 'PRIVATE_OWNED_REGULAR_FILE_REQUIRED')
      const bytes = await handle.readFile()
      requirePrerequisite(bytes.length <= max, 'PRIVATE_FILE_TOO_LARGE')
      return bytes
    } finally { await handle.close() }
  } catch (e) { if (e instanceof Prerequisite) throw e; throw new Prerequisite('PRIVATE_FILE_UNREADABLE_OR_INVALID') }
}
function parseJson(bytes: Uint8Array): unknown {
  try { return JSON.parse(Buffer.from(bytes).toString('utf8')) as unknown } catch { throw new Prerequisite('PRIVATE_FILE_UNREADABLE_OR_INVALID') }
}
async function pinnedJson(receipt: ReceiptFile) {
  const bytes = await privateBytes(receipt.path, 4 * 1024 * 1024)
  requirePrerequisite(sha256(bytes) === receipt.sha256, 'RECEIPT_HASH_MISMATCH')
  return parseJson(bytes)
}
export function validateQualification(input: InputConfig, world: JourneyWorld, value: unknown, in01: unknown, now: number) {
  const scoped = record(value) && value.kind === 'mcr-in02-prerequisites-v2'
  requirePrerequisite(record(value) && exact(value, ['kind', 'status', 'isolated', 'origin', 'releaseRevision', 'eventId', 'expiresAt', 'dependencies', 'resources', scoped ? 'serviceReceipt' : 'in01'])
    && (scoped ? value.status === 'READY' : value.kind === 'mcr-in02-prerequisites-v1' && value.status === 'PASS') && value.isolated === true
    && value.origin === world.origin && value.eventId === world.eventId && value.releaseRevision === world.releaseRevision
    && timestamp(value.expiresAt) && Date.parse(value.expiresAt) > now, 'CURRENT_CANDIDATE_QUALIFICATION_REQUIRED')
  const d = value.dependencies, r = value.resources
  requirePrerequisite(record(d) && exact(d, ['BE19', 'BE23', 'BE24', 'FE23', 'FE25', 'FE26']) && Object.values(d).every(v => v === (scoped ? 'READY' : 'PASS')), 'IN02_DEPENDENCIES_NOT_QUALIFIED')
  requirePrerequisite(record(r) && exact(r, ['partyId', 'giveawayId', 'placeId', 'mapRevision', 'placeRevision', 'meetingSha256'])
    && r.partyId === input.partyId && r.giveawayId === input.giveawayId && r.placeId === input.place.id && r.mapRevision === input.place.mapRevision
    && r.placeRevision === input.place.placeRevision && r.meetingSha256 === sha256(JSON.stringify(input.meeting)), 'ISOLATED_RESOURCE_QUALIFICATION_REQUIRED')
  if (scoped) {
    const receipt = value.serviceReceipt
    requirePrerequisite(world.version === 2 && record(receipt) && exact(receipt, ['path', 'sha256'])
      && receipt.path === world.serviceReceipt.path && receipt.sha256 === world.serviceReceipt.sha256, 'SAME_SERVICE_COHORT_REQUIRED')
    return Date.parse(value.expiresAt)
  }
  requirePrerequisite(record(in01) && in01.kind === 'mcr-in01-world-gate-v1' && in01.status === 'PASS' && in01.declaredRelease === world.releaseRevision
    && record(in01.checks) && ['configuration', 'services', 'independentAccounts', 'approvedLiveWorld', 'bidirectionalMovement', 'panels', 'theme', 'customization', 'reconnect', 'singleGame', 'actualArtVisual', 'teardown']
      .every(key => record(in01.checks) && record(in01.checks[key]) && in01.checks[key].status === 'PASS'), 'SAME_CANDIDATE_IN01_PASS_REQUIRED')
  return Date.parse(value.expiresAt)
}
export async function loadConfig(path = process.env.MCR_IN02_CONFIG): Promise<Config> {
  requirePrerequisite(path && isAbsolute(path), 'MCR_IN02_CONFIG_REQUIRED')
  const input = parseInput(parseJson(await privateBytes(path)))
  let world: JourneyWorld
  const raw = parseJson(await privateBytes(input.worldConfigPath))
  try {
    if (record(raw) && raw.version === 2) {
      world = parseJourneyWorld(raw)
      // Pin the descriptor bytes; do not manufacture its contents or interpret
      // this preflight READY declaration as an IN02 runtime result.
      await pinnedJson(world.serviceReceipt)
    } else world = await loadWorldConfig(input.worldConfigPath)
  }
  catch (e) { if (e instanceof WorldPrerequisite) throw new Prerequisite(`WORLD_${e.code}`); throw new Prerequisite('WORLD_CONFIG_UNAVAILABLE') }
  const receipt = await pinnedJson(input.prerequisiteReceipt)
  let in01: unknown
  if (record(receipt) && receipt.kind === 'mcr-in02-prerequisites-v1') {
    requirePrerequisite(record(receipt.in01) && exact(receipt.in01, ['path', 'sha256']) && typeof receipt.in01.path === 'string'
      && typeof receipt.in01.sha256 === 'string' && /^[0-9a-f]{64}$/.test(receipt.in01.sha256), 'PINNED_IN01_REPORT_REQUIRED')
    in01 = await pinnedJson(receipt.in01 as unknown as ReceiptFile)
  }
  return { ...input, world, qualificationExpiresAt: validateQualification(input, world, receipt, in01, Date.now()), prerequisiteHash: input.prerequisiteReceipt.sha256 }
}

/** Conservative harness observation only; never supplies the product clock/profile.
 * server_time was generated somewhere inside the full request interval. Anchoring
 * expiry at response arrival would incorrectly grant the network duration again.
 */
export function serverResponseClock(serverTime: unknown, requestStarted: number, responseReceived: number) {
  verify(timestamp(serverTime) && Number.isFinite(requestStarted) && requestStarted >= 0
    && Number.isFinite(responseReceived) && responseReceived >= requestStarted, 'SERVER_REQUEST_CLOCK_INTERVAL_REQUIRED')
  const generated = Date.parse(serverTime)
  const quantum = 10 ** (3 - Math.min(3, serverTime.match(/\.(\d+)Z$/)?.[1]?.length ?? 0))
  let previous = responseReceived
  return (observed: number) => {
    verify(Number.isFinite(observed) && observed >= previous, 'HARNESS_MONOTONIC_CLOCK_WITHDRAWN')
    previous = observed
    const upper = Math.ceil(generated + quantum + observed - requestStarted)
    verify(Number.isSafeInteger(upper), 'SERVER_REQUEST_CLOCK_OVERFLOW')
    return upper
  }
}

export interface Binding { eventId: string; actorId: string; operation: string; key: string }
function boundReceipt(v: unknown, b: Binding, now: number): asserts v is Record<string, unknown> {
  verify(record(v) && v.event_id === b.eventId && v.actor_id === b.actorId && v.operation_id === b.operation && v.idempotency_key === b.key
    && uuid(v.receipt_id) && timestamp(v.created_at) && timestamp(v.expires_at) && Date.parse(v.created_at) < Date.parse(v.expires_at) && Date.parse(v.expires_at) > now, 'RECEIPT_BINDING_OR_EXPIRY')
}
export function rejectedPickupCodeReceipt(v: unknown, b: Binding, now: number): boolean {
  boundReceipt(v, b, now)
  if (v.state === 'PENDING') { verify(v.resource_id === null && v.resource_revision === null && v.result_status === null && v.failure_reason === null, 'INVALID_PENDING_RECEIPT'); return false }
  verify(v.state === 'FAILED' && v.resource_id === null && v.resource_revision === null && v.result_status === 403 && v.failure_reason === 'PICKUP_CODE_INVALID', 'INVALID_CODE_FAILURE_NOT_DURABLE')
  return true
}
export function receiptResource(v: unknown, b: Binding, now: number): { id: string; revision: number } | null {
  boundReceipt(v, b, now)
  if (v.state === 'PENDING') { verify(v.resource_id === null && v.resource_revision === null && v.result_status === null && v.failure_reason === null, 'INVALID_PENDING_RECEIPT'); return null }
  verify(v.state === 'SUCCEEDED' && uuid(v.resource_id) && revision(v.resource_revision) && v.result_status === 200 && v.failure_reason === null, 'ORIGINAL_COMMAND_NOT_SUCCEEDED')
  return { id: v.resource_id, revision: v.resource_revision }
}
export interface Inventory { total: number; available: number; reserved: number; claimed: number }
export function inventory(v: unknown, eventId: string, giveawayId: string, ownerId: string): Inventory {
  verify(record(v) && record(v.giveaway) && v.giveaway.event_id === eventId && v.giveaway.giveaway_id === giveawayId && record(v.giveaway.owner)
    && v.giveaway.owner.user_id === ownerId && Array.isArray(v.reservations) && v.next_cursor === null, 'INVENTORY_BINDING_OR_COMPLETE_PAGE')
  const i = { total: v.giveaway.quantity_total, available: v.giveaway.available, reserved: v.reserved_unclaimed, claimed: v.quantity_claimed }
  verify(Object.values(i).every(n => typeof n === 'number' && Number.isSafeInteger(n) && n >= 0)
    && i.total === Number(i.available) + Number(i.reserved) + Number(i.claimed), 'STOCK_CONSERVATION')
  return i as Inventory
}
export function compareInventory(before: Inventory, after: Inventory, action: 'reserve' | 'claim' | 'duplicate') {
  verify(after.total === before.total && after.available >= 0 && after.reserved >= 0 && after.claimed >= 0, 'INVENTORY_NONNEGATIVE')
  const expected = action === 'reserve' ? { total: before.total, available: before.available - 1, reserved: before.reserved + 1, claimed: before.claimed }
    : action === 'claim' ? { total: before.total, available: before.available, reserved: before.reserved - 1, claimed: before.claimed + 1 } : before
  verify(JSON.stringify(after) === JSON.stringify(expected), 'ATOMIC_INVENTORY_TRANSITION')
}
export function rallySignature(v: unknown, eventId: string, partyId: string) {
  verify(record(v) && v.event_id === eventId && v.party_id === partyId && revision(v.meeting_revision) && (v.beacon_id === null || uuid(v.beacon_id)) && Array.isArray(v.entries), 'RALLY_BINDING')
  const entries = v.entries.map(e => {
    verify(record(e) && uuid(e.user_id) && revision(e.revision) && revision(e.meeting_revision)
      && ['ON_MY_WAY', 'HERE', 'MET', 'NEED_HELP', 'LEFT'].includes(String(e.state)) && timestamp(e.self_reported_at), 'RALLY_ENTRY')
    return { user: e.user_id, state: e.state, revision: e.revision, meeting: e.meeting_revision, at: e.self_reported_at }
  }).sort((a, b) => a.user.localeCompare(b.user))
  verify(new Set(entries.map(e => e.user)).size === entries.length, 'DUPLICATE_RALLY_ACTOR')
  return JSON.stringify({ meeting: v.meeting_revision, beacon: v.beacon_id, entries })
}
export function validateBeacon(v: unknown, c: Config, now: number) {
  verify(record(v) && uuid(v.beacon_id) && v.event_id === c.world.eventId && v.owner_id === c.world.accounts[0].userId && v.state === 'ACTIVE'
    && revision(v.revision) && record(v.meeting) && v.meeting.place_id === c.place.id && v.meeting.place_revision === c.place.placeRevision
    && v.meeting.map_revision === c.place.mapRevision && v.meeting.instructions === c.meeting.instructions && v.meeting.identification === c.meeting.identification
    && record(v.meeting.audience) && exact(v.meeting.audience, ['kind', 'party_id']) && v.meeting.audience.kind === 'PARTY_MEMBERS' && v.meeting.audience.party_id === c.partyId
    && timestamp(v.updated_at) && timestamp(v.meeting.expires_at) && Date.parse(v.meeting.expires_at) > now
    && Date.parse(v.meeting.expires_at) - Date.parse(v.updated_at) <= c.meeting.ttlSeconds * 1000 + 2000, 'BEACON_AUDIENCE_REVISION_EXPIRY')
  return { id: v.beacon_id, revision: v.revision, expiresAt: Date.parse(v.meeting.expires_at) }
}
export function validateReservation(v: unknown, c: Config, id?: string) {
  verify(record(v) && uuid(v.reservation_id) && (!id || v.reservation_id === id) && v.event_id === c.world.eventId && v.giveaway_id === c.giveawayId
    && v.recipient_id === c.world.accounts[1].userId && revision(v.revision) && timestamp(v.expires_at)
    && ['HELD', 'READY', 'CLAIMED', 'CANCELLED', 'EXPIRED'].includes(String(v.state)), 'RESERVATION_ACTOR_RESOURCE_BINDING')
  verify(v.state === 'CLAIMED' ? uuid(v.claim_receipt_id) : v.claim_receipt_id === null, 'CLAIM_RECEIPT_STATE_BINDING')
  return { id: v.reservation_id, revision: v.revision, state: String(v.state), expiresAt: Date.parse(v.expires_at), receiptId: v.claim_receipt_id }
}
export function validateJournal(v: unknown, b: Binding, giveawayId: string) {
  verify(record(v) && exact(v, ['version', 'actor', 'event', 'operation', 'key', 'giveawayId', 'reservationId', 'expectedRevision'])
    && v.version === 1 && v.actor === b.actorId && v.event === b.eventId && v.operation === b.operation && v.key === b.key
    && v.giveawayId === giveawayId && v.reservationId === null && revision(v.expectedRevision), 'SAFE_ORIGINAL_KEY_JOURNAL_REQUIRED')
}
export const steps = ['configuration', 'servicesAndIndependentAccounts', 'rootAndResourcePrerequisites', 'partyApproval', 'beaconAudienceRevisionExpiry',
  'virtualMovementNoArrival', 'explicitPhysicalRally', 'atomicReservation', 'reloadOriginalKeyRecovery', 'privatePickup', 'oneTimeCollection', 'privacyWithdrawal', 'teardown'] as const
export type Step = typeof steps[number]
export type Checks = Record<Step, { status: Status; code?: string }>
export function initialChecks(): Checks { return Object.fromEntries(steps.map(k => [k, { status: 'NOT_RUN', code: 'PRIOR_STEP_NOT_PASS' }])) as Checks }
export const result = (checks: Checks): Status => Object.values(checks).some(c => c.status === 'FAIL') ? 'FAIL' : Object.values(checks).every(c => c.status === 'PASS') ? 'PASS' : 'NOT_RUN'
export function failure(error: unknown): { status: 'FAIL' | 'NOT_RUN'; code: string } {
  const known = error instanceof Prerequisite || error instanceof GateFailure
  return { status: error instanceof Prerequisite ? 'NOT_RUN' : 'FAIL', code: known && /^[A-Z][A-Z0-9_]{0,100}$/.test(error.code) ? error.code : 'ASSERTION_OR_RUNTIME_FAILURE' }
}

/** Both existing Node-only contract loaders stay inside the allocated private
 * scratch. Never let Vite write its default cache through borrowed node_modules.
 */
async function contractLoader() {
  const cache = process.env.MCR_IN02_PRIVATE_CACHE, scratch = process.env.TMPDIR
  requirePrerequisite(cache && scratch && isAbsolute(cache) && isAbsolute(scratch), 'ALLOCATED_PRIVATE_CONTRACT_CACHE_REQUIRED')
  const parent = await lstat(scratch), within = relative(resolve(scratch), resolve(cache))
  requirePrerequisite(parent.isDirectory() && !parent.isSymbolicLink() && parent.uid === process.getuid?.() && !(parent.mode & 0o077)
    && within !== '' && within !== '..' && !within.startsWith('../') && !isAbsolute(within), 'ALLOCATED_PRIVATE_CONTRACT_CACHE_REQUIRED')
  for (let path = resolve(cache); path !== resolve(scratch); path = dirname(path)) {
    try {
      const info = await lstat(path)
      requirePrerequisite(info.isDirectory() && !info.isSymbolicLink() && info.uid === process.getuid?.() && !(info.mode & 0o077), 'PRIVATE_CONTRACT_CACHE_ESCAPE_REFUSED')
    } catch (error) {
      if (error instanceof Prerequisite) throw error
      requirePrerequisite(record(error) && error.code === 'ENOENT', 'PRIVATE_CONTRACT_CACHE_UNAVAILABLE')
    }
  }
  const { createServer } = await import('vite')
  return createServer({ configFile: false, root: fileURLToPath(new URL('../../', import.meta.url)), cacheDir: cache,
    optimizeDeps: { noDiscovery: true, include: [] },
    server: { middlewareMode: true, watch: null, ws: false }, logLevel: 'silent' })
}

/** Read-only use of the existing committed C05 decoder; no IN01 file is edited. */
export async function loadJourneyWorldDecoder() {
  const loader = await contractLoader()
  try { return (await loader.ssrLoadModule('/src/world/network/transport/contract.ts')).decode as import('./world-gate/observer.ts').Decoder }
  finally { await loader.close() }
}

/** The spec uses the committed feature validator in Node only, never imports mocks into the product. */
export async function domainValidator() {
  const loader = await contractLoader()
  try {
    const general = (await loader.ssrLoadModule('/src/features/party-authoring/model/contract.ts')).contractValue as (name: string, value: unknown) => unknown
    const beacon = await loader.ssrLoadModule('/src/features/meet-beacon/model/contract.ts')
    // The party interpreter deliberately rejects oneOf. Beacon audiences use FE20's strict union validator.
    return (name: string, value: unknown): unknown => {
      if (name === 'Domain_beacon_MeetBeacon') return beacon.beaconSchema.parse(value)
      if (name === 'Domain_beacon_CreateBeacon') {
        verify(record(value) && Object.keys(value).length === 2 && Object.hasOwn(value, 'meeting')
          && value.physical_presence_acknowledged === true, 'EXPLICIT_BEACON_REQUEST_CONTRACT')
        beacon.meetingSchema.parse(value.meeting); return value
      }
      return general(name, value)
    }
  }
  finally { await loader.close() }
}

/** Offline parser/invariant regressions only. No network, browser fixtures or real-gate fallback. */
export async function selfTest() {
  const a: typeof import('node:assert').strict = (await import('node:assert')).strict
  let checks = 0
  const check = (work: () => void) => { work(); checks++ }
  const id = (n: number) => `00000000-0000-4000-8000-${String(n).padStart(12, '0')}`
  const input = (): InputConfig => ({ version: 1, worldConfigPath: '/tmp/offline-world', prerequisiteReceipt: { path: '/tmp/offline-receipt', sha256: 'b'.repeat(64) },
    partyId: id(1), giveawayId: id(2), place: { id: id(3), mapRevision: 1, placeRevision: 2 }, meeting: { instructions: 'OFFLINE PRIVATE TEXT', identification: 'OFFLINE PRIVATE DETAIL', ttlSeconds: 900 } })
  check(() => a.equal(parseInput(input()).version, 1))
  for (const v of [null, {}, { ...input(), extra: true }, { ...input(), worldConfigPath: 'relative' }, { ...input(), partyId: '../x' },
    { ...input(), place: { ...input().place, placeRevision: 0 } }, { ...input(), meeting: { ...input().meeting, ttlSeconds: 901 } },
    { ...input(), meeting: { ...input().meeting, instructions: '' } }]) check(() => a.throws(() => parseInput(v), Prerequisite))
  check(() => a.deepEqual(failure(new Error('secret-token-123456')), { status: 'FAIL', code: 'ASSERTION_OR_RUNTIME_FAILURE' }))
  check(() => a.deepEqual(failure(new Prerequisite('MISSING')), { status: 'NOT_RUN', code: 'MISSING' }))
  check(() => a.equal(result(initialChecks()), 'NOT_RUN'))
  check(() => { const c = initialChecks(); c.partyApproval = { status: 'FAIL' }; a.equal(result(c), 'FAIL') })
  check(() => { const c = initialChecks(); steps.forEach(k => { c[k] = { status: 'PASS' } }); a.equal(result(c), 'PASS') })
  const b = { eventId: id(4), actorId: id(5), operation: 'reserveConcertGiveaway', key: id(6) }
  const r = { event_id: b.eventId, actor_id: b.actorId, operation_id: b.operation, idempotency_key: b.key, receipt_id: id(7),
    created_at: '2026-11-04T00:00:00Z', expires_at: '2026-11-04T01:00:00Z', state: 'SUCCEEDED', resource_id: id(8), resource_revision: 1, failure_reason: null, result_status: 200 }
  const now = Date.parse('2026-11-04T00:01:00Z')
  check(() => a.deepEqual(receiptResource(r, b, now), { id: id(8), revision: 1 }))
  for (const bad of [{ ...r, actor_id: id(9) }, { ...r, event_id: id(9) }, { ...r, idempotency_key: id(9) }, { ...r, state: 'RETIRED' },
    { ...r, resource_revision: 0 }, { ...r, expires_at: '2026-11-04T00:00:01Z' }]) check(() => a.throws(() => receiptResource(bad, b, now), GateFailure))
  check(() => a.equal(receiptResource({ ...r, state: 'PENDING', resource_id: null, resource_revision: null, result_status: null }, b, now), null))
  const invalidCode = { ...r, state: 'FAILED', resource_id: null, resource_revision: null, result_status: 403, failure_reason: 'PICKUP_CODE_INVALID' }
  check(() => a.equal(rejectedPickupCodeReceipt(invalidCode, b, now), true))
  check(() => a.equal(rejectedPickupCodeReceipt({ ...r, state: 'PENDING', resource_id: null, resource_revision: null, result_status: null }, b, now), false))
  for (const bad of [{ ...invalidCode, actor_id: id(9) }, { ...invalidCode, idempotency_key: id(9) }, { ...invalidCode, failure_reason: 'RATE_LIMITED' },
    { ...invalidCode, result_status: 503 }, { ...invalidCode, resource_id: id(9) }]) check(() => a.throws(() => rejectedPickupCodeReceipt(bad, b, now), GateFailure))
  const anchored = serverResponseClock('2026-11-04T00:00:00.000Z', 100, 5100)
  check(() => a.equal(anchored(5100), Date.parse('2026-11-04T00:00:05.001Z')))
  check(() => a.equal(anchored(6100), Date.parse('2026-11-04T00:00:06.001Z')))
  check(() => a.throws(() => anchored(6099), GateFailure))
  check(() => a.throws(() => serverResponseClock('not-a-time', 100, 200), GateFailure))
  check(() => a.throws(() => serverResponseClock(r.created_at, 200, 100), GateFailure))
  check(() => a.throws(() => serverResponseClock(r.created_at, -1, 100), GateFailure))
  check(() => a.equal(serverResponseClock(r.created_at, 100, 200)(200), Date.parse(r.created_at) + 1100))
  const before: Inventory = { total: 1, available: 1, reserved: 0, claimed: 0 }, reserved = { total: 1, available: 0, reserved: 1, claimed: 0 }, claimed = { total: 1, available: 0, reserved: 0, claimed: 1 }
  check(() => compareInventory(before, reserved, 'reserve')); check(() => compareInventory(reserved, claimed, 'claim')); check(() => compareInventory(claimed, claimed, 'duplicate'))
  check(() => a.throws(() => compareInventory(claimed, { ...claimed, claimed: 2 }, 'duplicate'), GateFailure))
  const rally = { event_id: id(1), party_id: id(2), meeting_revision: 1, beacon_id: id(3), entries: [{ user_id: id(5), state: 'HERE', meeting_revision: 1, revision: 1, self_reported_at: r.created_at }] }
  check(() => a.notEqual(rallySignature(rally, id(1), id(2)), rallySignature({ ...rally, entries: [] }, id(1), id(2))))
  check(() => a.throws(() => rallySignature({ ...rally, entries: [...rally.entries, ...rally.entries] }, id(1), id(2)), GateFailure))
  const j = { version: 1, actor: b.actorId, event: b.eventId, operation: b.operation, key: b.key, giveawayId: id(2), reservationId: null, expectedRevision: 1 }
  check(() => validateJournal(j, b, id(2))); check(() => a.throws(() => validateJournal({ ...j, confirmation_code: '123456' }, b, id(2)), GateFailure))
  check(() => a.throws(() => validateJournal({ ...j, key: id(99) }, b, id(2)), GateFailure))
  try { await loadConfig(''); a.fail('expected missing config') } catch (e) { a.deepEqual(failure(e), { status: 'NOT_RUN', code: 'MCR_IN02_CONFIG_REQUIRED' }); checks++ }
  // Qualifications are isolated synthetic input to the parser, never passed to a browser/run.
  const world = { eventId: id(4), origin: 'http://127.0.0.1:1', releaseRevision: 'a'.repeat(40) } as GateConfig
  const in01 = { kind: 'mcr-in01-world-gate-v1', status: 'PASS', declaredRelease: world.releaseRevision,
    checks: Object.fromEntries(['configuration', 'services', 'independentAccounts', 'approvedLiveWorld', 'bidirectionalMovement', 'panels', 'theme', 'customization', 'reconnect', 'singleGame', 'actualArtVisual', 'teardown'].map(k => [k, { status: 'PASS' }])) }
  const q = { kind: 'mcr-in02-prerequisites-v1', status: 'PASS', isolated: true, origin: world.origin, eventId: world.eventId,
    releaseRevision: world.releaseRevision, expiresAt: r.expires_at, dependencies: { BE19: 'PASS', BE23: 'PASS', BE24: 'PASS', FE23: 'PASS', FE25: 'PASS', FE26: 'PASS' },
    resources: { partyId: input().partyId, giveawayId: input().giveawayId, placeId: input().place.id, mapRevision: 1, placeRevision: 2, meetingSha256: sha256(JSON.stringify(input().meeting)) },
    in01: { path: '/tmp/offline-in01', sha256: 'b'.repeat(64) } }
  check(() => a.equal(validateQualification(input(), world, q, in01, now), Date.parse(r.expires_at)))
  for (const bad of [{ ...q, isolated: false }, { ...q, origin: 'https://another.invalid' }, { ...q, eventId: id(90) },
    { ...q, releaseRevision: 'c'.repeat(40) }, { ...q, expiresAt: r.created_at }, { ...q, dependencies: { ...q.dependencies, BE24: 'NOT_RUN' } },
    { ...q, resources: { ...q.resources, placeRevision: 3 } }, { ...q, resources: { ...q.resources, meetingSha256: 'c'.repeat(64) } }]) {
    check(() => a.throws(() => validateQualification(input(), world, bad, in01, now), Prerequisite))
  }
  check(() => a.throws(() => validateQualification(input(), world, q, { ...in01, checks: { ...in01.checks, actualArtVisual: { status: 'NOT_RUN' } } }, now), Prerequisite))
  check(() => a.throws(() => validateQualification(input(), world, q, { ...in01, declaredRelease: 'c'.repeat(40) }, now), Prerequisite))
  const journey = { version: 2, isolated: true, eventId: world.eventId, origin: world.origin, releaseRevision: world.releaseRevision, accounts: [
    { userId: id(5), email: 'owner@offline.invalid', password: 'synthetic-owner-only' },
    { userId: id(6), email: 'recipient@offline.invalid', password: 'synthetic-recipient-only' }],
    serviceReceipt: { path: '/tmp/offline-cohort', sha256: 'd'.repeat(64) } }
  check(() => a.equal(parseJourneyWorld(journey).version, 2))
  for (const bad of [{ ...journey, artReview: {} }, { ...journey, isolated: false }, { ...journey, origin: 'http://unsafe.invalid' },
    { ...journey, accounts: [journey.accounts[0], journey.accounts[0]] }, { ...journey, serviceReceipt: { ...journey.serviceReceipt, sha256: 'not-a-digest' } }])
    check(() => a.throws(() => parseJourneyWorld(bad), Prerequisite))
  const scoped = { kind: 'mcr-in02-prerequisites-v2', status: 'READY', isolated: q.isolated, origin: q.origin, eventId: q.eventId,
    releaseRevision: q.releaseRevision, expiresAt: q.expiresAt, resources: q.resources, serviceReceipt: journey.serviceReceipt,
    dependencies: Object.fromEntries(Object.keys(q.dependencies).map(key => [key, 'READY'])) }
  check(() => a.equal(validateQualification(input(), parseJourneyWorld(journey), scoped, undefined, now), Date.parse(r.expires_at)))
  for (const bad of [{ ...scoped, status: 'PASS' }, { ...scoped, in01 }, { ...scoped, serviceReceipt: { ...journey.serviceReceipt, sha256: 'e'.repeat(64) } },
    { ...scoped, dependencies: { ...scoped.dependencies, BE24: 'NOT_RUN' } }, { ...scoped, expiresAt: r.created_at }])
    check(() => a.throws(() => validateQualification(input(), parseJourneyWorld(journey), bad, undefined, now), Prerequisite))
  const { mkdtemp, writeFile, chmod, symlink, rm } = await import('node:fs/promises')
  const { tmpdir } = await import('node:os'), { join } = await import('node:path')
  const dir = await mkdtemp(join(tmpdir(), 'mcr-in02-offline-')); await chmod(dir, 0o700)
  const file = join(dir, 'private.json'), link = join(dir, 'link.json')
  try {
    await writeFile(file, '{}', { mode: 0o600 })
    a.equal((await privateBytes(file)).toString(), '{}'); checks++
    a.deepEqual(await pinnedJson({ path: file, sha256: sha256('{}') }), {}); checks++
    await a.rejects(pinnedJson({ path: file, sha256: '0'.repeat(64) }), Prerequisite); checks++
    await a.rejects(privateBytes(file, 1), Prerequisite); checks++
    await chmod(file, 0o644); await a.rejects(privateBytes(file), Prerequisite); checks++
    await chmod(file, 0o600); await symlink(file, link); await a.rejects(privateBytes(link), Prerequisite); checks++
    await writeFile(file, 'not JSON'); await a.rejects(loadConfig(file), Prerequisite); checks++
    await a.rejects(privateBytes(dir), Prerequisite); checks++
  } finally { await rm(dir, { recursive: true, force: true }) }
  const decode = await domainValidator()
  const meeting = { place_id: id(3), map_revision: 1, place_revision: 2, instructions: 'Offline', identification: 'Offline',
    audience: { kind: 'PARTY_MEMBERS', party_id: id(1) }, expires_at: r.expires_at }
  const beacon = { beacon_id: id(10), event_id: id(4), owner_id: id(5), meeting, created_at: r.created_at, updated_at: r.created_at, state: 'ACTIVE', revision: 1 }
  check(() => a.deepEqual(decode('Domain_beacon_MeetBeacon', beacon), beacon))
  check(() => a.throws(() => decode('Domain_beacon_MeetBeacon', { ...beacon, meeting: { ...meeting, audience: { kind: 'EXPLICIT_USERS', user_ids: [id(5), id(5)] } } })))
  check(() => a.throws(() => decode('Domain_beacon_MeetBeacon', { ...beacon, extra: true })))
  check(() => a.throws(() => decode('Domain_beacon_CreateBeacon', { meeting, physical_presence_acknowledged: false })))
  check(() => a.throws(() => decode('Domain_beacon_CreateBeacon', { meeting, physical_presence_acknowledged: true, extra: true })))
  check(() => a.deepEqual(decode('Domain_beacon_CreateBeacon', { meeting, physical_presence_acknowledged: true }), { meeting, physical_presence_acknowledged: true }))
  process.stdout.write(JSON.stringify({ kind: 'mcr-in02-helper-check-v1', status: 'PASS', checks, realGate: 'NOT_RUN', scope: 'Offline invariant checks only' }) + '\n')
}
if (process.argv.includes('--self-test')) await selfTest()
