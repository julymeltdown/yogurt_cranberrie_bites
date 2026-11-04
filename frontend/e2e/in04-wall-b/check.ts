import assert from 'node:assert/strict'
import { readFile, mkdtemp, writeFile, chmod, symlink, rm } from 'node:fs/promises'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { Contracts, loadContracts, hash, parseConfig } from '../real/mural-helpers.ts'
import type { Snapshot } from '../real/mural-helpers.ts'
import { qualifyContracts, loadWallContracts, bindAction, bindTools, connectedRegion, expectedEffect, recover, requireInputs } from './helpers.ts'
import type { Command, OwnAction, Tools } from './helpers.ts'
import { loadRuntimeAdapter, qualifiedPhaserModuleUrl } from './runtime.ts'

const contracts = await loadWallContracts(), id = (n: number) => `00000000-0000-4000-8000-${String(n).padStart(12, '0')}`
const config = parseConfig({ version: 1, isolated: true, allowDestructiveFixtureEndState: true, origin: 'http://127.0.0.1:4000',
  eventId: id(1), environmentId: id(8), releaseRevision: 'a'.repeat(40),
  members: [{ userId: id(2), email: 'a@example.invalid', password: 'self-check-only' }, { userId: id(3), email: 'b@example.invalid', password: 'self-check-only' }],
  operator: { userId: id(4), email: 'o@example.invalid', password: 'self-check-only' }, contractHashes: { domain: 'b'.repeat(64), realtime: 'c'.repeat(64) },
  restoreSnapshotId: id(9), maxCooldownMs: 10000, restartObservationTimeoutMs: 10000 })
const snapshot: Snapshot & { created_at: string } = { snapshot_id: id(9), event_id: config.eventId, revision: 1, state: 'EDITABLE', width: 128, height: 128,
  palette: Array.from({ length: 16 }, (_, n) => '#' + n.toString(16).repeat(6).toUpperCase()), pixels: Array.from({ length: 16384 }, () => 0),
  created_at: '2026-11-05T00:00:00Z', cooldown_ms: 5000, server_time: '2026-11-05T00:00:01Z', next_allowed_at: null }
snapshot.pixels[0] = 1; snapshot.pixels[1] = 1; snapshot.pixels[129] = 1; snapshot.pixels[258] = 1
const command: Command = { operation_id: 'fillConcertMuralRegion', key: id(6), body: { expected_revision: 1, x: 0, y: 0, color_index: 2 } }
const action: OwnAction = { action_id: id(5), receipt_id: id(7), event_id: config.eventId, actor_id: config.members[0].userId,
  command: { operation_id: command.operation_id, body: command.body }, idempotency_key: command.key, kind: 'FILL', revision: 2,
  changed_cells: 3, committed_at: '2026-11-05T00:00:00Z', next_allowed_at: '2026-11-05T00:00:05Z', undo_expires_at: '2026-11-05T00:10:00Z', reset_required: true }
const receipt = { receipt_id: action.receipt_id, actor_id: action.actor_id, event_id: action.event_id, operation_id: command.operation_id,
  idempotency_key: command.key, state: 'SUCCEEDED', resource_id: config.eventId, resource_revision: 2, result_status: 200,
  failure_reason: null, created_at: action.committed_at, expires_at: '2026-11-06T00:00:00Z' }
let cases = 0
function reject(work: () => unknown) { assert.throws(work); cases++ }
assert.deepEqual(connectedRegion(snapshot, 0), [0, 1, 129]); cases++
const changed = expectedEffect(snapshot, command); assert.deepEqual(changed.slice(0, 2), [2, 2]); assert.equal(changed[129], 2); assert.equal(changed[258], 1); cases++
const erase: Command = { operation_id: 'eraseConcertMuralPixel', key: id(6), body: { expected_revision: 1, x: 1, y: 1 } }
assert.equal(expectedEffect(snapshot, erase)[129], 0); cases++
const boundary = { pixels: Array.from({ length: 16384 }, (_, n) => n < 256 ? 1 : 0) }
assert.equal(connectedRegion(boundary, 0).length, 256); cases++
boundary.pixels[256] = 1; reject(() => connectedRegion(boundary, 0)); reject(() => connectedRegion(snapshot, -1))
reject(() => expectedEffect(snapshot, { ...command, body: { ...command.body, expected_revision: 2 } }))
assert.deepEqual(bindAction(contracts, action, command, action.actor_id, config.eventId, receipt), action); cases++
for (const patch of [{ actor_id: id(99) }, { event_id: id(99) }, { idempotency_key: id(99) }, { receipt_id: id(99) }, { revision: 3 },
  { kind: 'ERASE' }, { reset_required: false }, { changed_cells: 257 }, { next_allowed_at: '2026-11-04T00:00:00Z' },
  { undo_expires_at: '2026-11-05T00:10:01Z' }, { command: { ...command, body: { ...command.body, x: 1 } } }, { before_colors: [1, 1, 1] }]) {
  reject(() => bindAction(contracts, { ...action, ...patch }, command, action.actor_id, config.eventId, receipt))
}
const capability: Tools = { version: 1, event_id: config.eventId, actor_id: action.actor_id, revision: 2, server_time: snapshot.server_time,
  background_index: 0, fill_limit: 256, cell_allowance: 512, window_seconds: 600, remaining_cells: 509, next_cell_available_at: null,
  next_allowed_at: action.next_allowed_at, editable: true, available: { pencil: true, erase: true, fill: true, undo: true }, undo_window_seconds: 600,
  undo: { action_id: action.action_id, revision: 2, expires_at: action.undo_expires_at, changed_cells: 3 }, latest_action: action, unresolved: null }
assert.deepEqual(bindTools(contracts, capability, action.actor_id, config.eventId, 2), capability); cases++
for (const patch of [{ actor_id: id(99) }, { revision: 3 }, { background_index: 1 }, { remaining_cells: 513 }, { fill_limit: 257 },
  { undo: { ...capability.undo, action_id: id(99) } }, { undo: { ...capability.undo, changed_cells: 4 } }, { before_colors: [1] }]) {
  reject(() => bindTools(contracts, { ...capability, ...patch }, action.actor_id, config.eventId, 2))
}
const calls: string[] = []
let responses: Record<string, { status: number; value: unknown }> = {}
const reader = { contracts, account: config.members[0], config, call: async (op: string, _body?: unknown, parameters: Record<string, string> = {}, query: Record<string, string> = {}) => {
  calls.push(op); assert.equal(contracts.operation(op).method, 'get')
  if (op === 'getOwnConcertMuralAction') assert.deepEqual(parameters, { idempotencyKey: command.key })
  if (op === 'getConcertCommandStatus') assert.deepEqual(query, { operation_id: command.operation_id, idempotency_key: command.key })
  return { ...(responses[op] ?? { status: 404, value: {} }), key: id(90) }
} }
const run = async (status: unknown, result: unknown = action) => {
  calls.length = 0
  responses = { getConcertCommandStatus: { status: 200, value: status }, getConcertMuralSnapshot: { status: 200, value: { ...snapshot, revision: 2, pixels: changed } }, getOwnConcertMuralAction: { status: 200, value: result } }
  return recover(reader, command)
}
assert.equal((await run(receipt)).state, 'SUCCEEDED'); assert.deepEqual(calls, ['getConcertCommandStatus', 'getConcertMuralSnapshot', 'getOwnConcertMuralAction']); cases++
const pending = { ...receipt, state: 'PENDING', resource_id: null, resource_revision: null, result_status: null }
assert.equal((await run(pending)).state, 'PENDING'); assert(!calls.includes('getOwnConcertMuralAction')); cases++
assert.equal((await run({ ...pending, state: 'FAILED', failure_reason: 'MURAL_ACTION_UNRESOLVED' })).state, 'FAILED'); cases++
for (const patch of [{ actor_id: id(99) }, { operation_id: 'placeConcertPixel' }, { idempotency_key: id(99) }, { event_id: id(99) }, { resource_revision: 3 }, { failure_reason: 'MURAL_COOLDOWN' }, { expires_at: action.committed_at }]) {
  await assert.rejects(run({ ...receipt, ...patch })); cases++
}
await assert.rejects(run({ ...pending, state: 'FAILED', failure_reason: 'MADE_UP' })); cases++
await assert.rejects(run({ ...pending, resource_revision: 2 })); cases++
responses.getConcertCommandStatus = { status: 404, value: {} }
assert.equal((await recover(reader, command)).state, 'UNKNOWN'); cases++
await run(receipt); responses.getOwnConcertMuralAction = { status: 404, value: {} }
assert.equal((await recover(reader, command)).state, 'UNKNOWN'); cases++
requireInputs(config); reject(() => requireInputs({ ...config, isolated: false } as unknown as typeof config))
// Source-only URL identity checks; these paths are not actual cache provenance.
const phaserPath = '/@fs/private/tmp/in04-vite-cache/deps/phaser.js?v=abc012ef'
for (const path of [phaserPath, phaserPath.replace('/@fs/', '/@fs//'), phaserPath.replace('in04-vite-cache', 'in04%20vite-cache')]) {
  assert.equal(qualifiedPhaserModuleUrl(path, config.origin), config.origin + path); cases++
  assert.equal(qualifiedPhaserModuleUrl(config.origin + path, config.origin), config.origin + path); cases++
}
for (const value of [undefined, '', '/node_modules/.vite/deps/phaser.js?v=abc012ef',
  phaserPath.split('?')[0], phaserPath.replace('?v=abc012ef', '?v='), phaserPath.replace('abc012ef', 'ABC012EF'),
  phaserPath.replace('abc012ef', 'not-a-hash'), phaserPath + '&v=abc012ef', phaserPath + '&raw', phaserPath + '#fragment', phaserPath + '#',
  phaserPath.replace('phaser.js', 'phaser.js.map'), phaserPath.replace('/deps/', '/other/'),
  phaserPath.replace('in04-vite-cache', 'cache%2fother'), phaserPath.replace('in04-vite-cache', 'cache%5cother'),
  phaserPath.replace('in04-vite-cache', 'cache%00other'), phaserPath.replace('in04-vite-cache', '%2e%2e'),
  phaserPath.replace('in04-vite-cache', '../in04-vite-cache'), phaserPath.replace('in04-vite-cache', 'cache\\other'),
  ' ' + phaserPath, phaserPath + '\n', '//127.0.0.1:4000' + phaserPath, 'http://other.invalid' + phaserPath,
  'http://user:pass@127.0.0.1:4000' + phaserPath, config.origin + phaserPath.replace('/@fs/', '/@fs///')]) {
  reject(() => qualifiedPhaserModuleUrl(value, config.origin))
}
await assert.rejects(loadRuntimeAdapter('', ''), /ACTUAL_OWNER_RUNTIME_MODULE_AND_QUALIFIED_SHA_REQUIRED/); cases++
const raw = await loadContracts()
const union = raw.schemas.Domain_mural_ActionBody as { oneOf: unknown[] }
reject(() => qualifyContracts(new Contracts(raw.operations, { ...raw.schemas, Domain_mural_ActionBody: { oneOf: [union.oneOf[0], union.oneOf[0], union.oneOf[2], union.oneOf[3]] } })))
reject(() => qualifyContracts(new Contracts(raw.operations, { ...raw.schemas, Domain_mural_ActionBody: { anyOf: union.oneOf } })))
const adapterDir = await mkdtemp(fileURLToPath(new URL('./.adapter-check-', import.meta.url)))
try {
  const file = join(adapterDir, 'owner.mjs'), link = join(adapterDir, 'link.mjs')
  const bytes = 'export const runtimeQualification = {inspect(){throw Error("SOURCE_TEST_ONLY")},observeRestart(){throw Error("SOURCE_TEST_ONLY")},muralBinding(){throw Error("SOURCE_TEST_ONLY")}}'
  await writeFile(file, bytes, { mode: 0o600 })
  await assert.rejects(loadRuntimeAdapter(file, '0'.repeat(64)), /RUNTIME_MODULE_SOURCE_HASH_MISMATCH/); cases++
  await symlink(file, link)
  await assert.rejects(loadRuntimeAdapter(link, hash(bytes)), /OWNED_IMMUTABLE_RUNTIME_MODULE_REQUIRED/); cases++
  await chmod(file, 0o666)
  await assert.rejects(loadRuntimeAdapter(file, hash(bytes)), /OWNED_IMMUTABLE_RUNTIME_MODULE_REQUIRED/); cases++
  await chmod(file, 0o600)
  assert.equal(typeof (await loadRuntimeAdapter(file, hash(bytes))).inspect, 'function'); cases++ // Interface only; never a deployment grant.
  await writeFile(file, 'export const runtimeQualification = {}')
  await assert.rejects(loadRuntimeAdapter(file, hash('export const runtimeQualification = {}')), /ACTUAL_OWNER_RUNTIME_IMPLEMENTATION_REQUIRED/); cases++
} finally { await rm(adapterDir, { recursive: true }) }

const source = await readFile(new URL('./wall-b.spec.ts', import.meta.url), 'utf8')
assert(!/route\.fulfill\(\{[^}]*json:|__MURAL_ROOT_ENTRY__|MockWorldTransport|MockSocket|window\.fetch\s*=/.test(source)); cases++
process.stdout.write(JSON.stringify({ result: 'PASS', checks: cases, scope: 'runner binding/recovery/oracle negatives only', realIN04: 'NOT_RUN' }) + '\n')
