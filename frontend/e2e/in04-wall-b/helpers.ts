import assert from 'node:assert/strict'
import type { randomUUID } from 'node:crypto'
import type { Page } from '@playwright/test'
import type { Schemas } from '../../src/shared/api/generated/types'
import { Contracts, NotRun, bindOwnResult, need, loadContracts } from '../real/mural-helpers.ts'
import type { Actor, GateConfig, Receipt, Snapshot } from '../real/mural-helpers.ts'

export type Command = Schemas['Domain_mural_ActionBody'] & { readonly key: string }
export type OwnAction = Schemas['Domain_mural_OwnAction']
export type Tools = Schemas['Domain_mural_ToolCapabilities']
export const mutations = ['placeConcertPixel', 'eraseConcertMuralPixel', 'fillConcertMuralRegion', 'undoConcertMuralAction'] as const
const kinds = { placeConcertPixel: 'PENCIL', eraseConcertMuralPixel: 'ERASE', fillConcertMuralRegion: 'FILL', undoConcertMuralAction: 'UNDO' } as const
/** The retained decoder predates oneOf. This exact frozen union has four
 * mutually exclusive literal discriminators: oneOf and anyOf are equivalent
 * only after verifying that fact. Keep every original branch/ref/closed rule.
 */
export function qualifyContracts(base: Contracts): Contracts {
  const union = base.schemas.Domain_mural_ActionBody as { oneOf?: { type?: string; additionalProperties?: boolean; required?: string[]; properties?: Record<string, { const?: string }> }[] }
  assert(union && Object.keys(union).length === 1 && Array.isArray(union.oneOf) && union.oneOf.length === 4)
  const names = union.oneOf.map(branch => {
    assert(branch.type === 'object' && branch.additionalProperties === false)
    assert.deepEqual([...(branch.required ?? [])].sort(), ['body', 'operation_id'])
    assert.deepEqual(Object.keys(branch.properties ?? {}).sort(), ['body', 'operation_id'])
    assert.deepEqual(Object.keys(branch.properties!.operation_id!), ['const'])
    return branch.properties!.operation_id!.const
  })
  assert.deepEqual([...names].sort(), [...mutations].sort())
  return new Contracts(base.operations, { ...base.schemas, Domain_mural_ActionBody: { anyOf: union.oneOf } })
}
export async function loadWallContracts(config?: GateConfig) { return qualifyContracts(await loadContracts(config)) }

export function reason(value: unknown): unknown {
  return value && typeof value === 'object' && 'details' in value && value.details && typeof value.details === 'object' && 'reason' in value.details ? value.details.reason : undefined
}
export function bindAction(contracts: Contracts, input: unknown, command: Command, actorId: string, eventId: string, receipt?: Receipt): OwnAction {
  const action = contracts.decode<OwnAction>('Domain_mural_OwnAction', input)
  assert.equal(action.actor_id, actorId); assert.equal(action.event_id, eventId)
  assert.equal(action.idempotency_key, command.key); assert.equal(action.command.operation_id, command.operation_id)
  assert.deepEqual(action.command.body, command.body); assert.equal(action.kind, kinds[command.operation_id])
  assert.equal(action.revision, command.body.expected_revision + 1)
  assert(Date.parse(action.next_allowed_at) >= Date.parse(action.committed_at))
  const window = Date.parse(action.undo_expires_at) - Date.parse(action.committed_at)
  assert(window > 0 && window <= 600000)
  if (action.kind === 'PENCIL' || action.kind === 'ERASE') assert.equal(action.changed_cells, 1)
  if (action.kind !== 'PENCIL') assert.equal(action.reset_required, true)
  if (receipt) {
    assert.equal(action.receipt_id, receipt.receipt_id)
    assert.equal(action.revision, receipt.resource_revision)
    assert(Date.parse(action.undo_expires_at) <= Date.parse(receipt.expires_at))
  }
  return action
}
export function bindTools(contracts: Contracts, input: unknown, actorId: string, eventId: string, revision?: number): Tools {
  const tools = contracts.decode<Tools>('Domain_mural_ToolCapabilities', input)
  assert.equal(tools.actor_id, actorId); assert.equal(tools.event_id, eventId)
  if (revision !== undefined) assert.equal(tools.revision, revision)
  const latest = tools.latest_action
  if (latest) {
    bindAction(contracts, latest, { ...latest.command, key: latest.idempotency_key }, actorId, eventId)
    assert(latest.revision <= tools.revision); assert(Date.parse(latest.committed_at) <= Date.parse(tools.server_time))
  }
  if (tools.undo) {
    assert(latest && latest.kind !== 'UNDO')
    assert.equal(tools.undo.action_id, latest.action_id); assert.equal(tools.undo.revision, tools.revision)
    assert.equal(tools.undo.revision, latest.revision); assert.equal(tools.undo.changed_cells, latest.changed_cells)
    assert.equal(tools.undo.expires_at, latest.undo_expires_at)
    assert(Date.parse(tools.undo.expires_at) > Date.parse(tools.server_time))
  }
  return tools
}
type Reader = Pick<Actor, 'contracts' | 'account' | 'config'> & {
  call(id: string, body?: unknown, parameters?: Record<string, string>, query?: Record<string, string>): Promise<{ status: number; value: unknown }>
}
export function originalKey(value: string): ReturnType<typeof randomUUID> {
  assert(/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(value))
  return value as ReturnType<typeof randomUUID>
}
export type Recovery = { state: 'UNKNOWN' | 'PENDING' } | { state: 'FAILED'; reason: string } | { state: 'SUCCEEDED'; action: OwnAction; receipt: Receipt }
/** GET-only; a missing/pending result or equal snapshot can never acknowledge a command. */
export async function recover(reader: Reader, command: Command): Promise<Recovery> {
  const status = await reader.call('getConcertCommandStatus', undefined, {}, { operation_id: command.operation_id, idempotency_key: command.key })
  if (status.status !== 200) return { state: 'UNKNOWN' }
  const r = reader.contracts.decode<Receipt>('Domain_common_CommandReceipt', status.value)
  assert.equal(r.actor_id, reader.account.userId); assert.equal(r.event_id, reader.config.eventId)
  assert.equal(r.operation_id, command.operation_id); assert.equal(r.idempotency_key, command.key)
  assert(Date.parse(r.created_at) < Date.parse(r.expires_at))
  const s = await reader.call('getConcertMuralSnapshot')
  assert.equal(s.status, 200)
  const snapshot = reader.contracts.decode<Snapshot>('Domain_mural_MuralSnapshot', s.value)
  assert.equal(snapshot.event_id, reader.config.eventId)
  assert(Date.parse(r.expires_at) > Date.parse(snapshot.server_time))
  if (r.state === 'PENDING' || r.state === 'FAILED') {
    assert.equal(r.resource_id, null); assert.equal(r.resource_revision, null); assert.equal(r.result_status, null)
    if (r.state === 'PENDING') { assert.equal(r.failure_reason, null); return { state: 'PENDING' } }
    const allowed = reader.contracts.operation(command.operation_id) as { errors?: Record<string, string[]> }
    // Allowed reasons come from the frozen operation registry, never a client guess.
    assert(r.failure_reason && Object.values(allowed.errors ?? {}).flat().includes(r.failure_reason))
    return { state: 'FAILED', reason: r.failure_reason }
  }
  if (r.state !== 'SUCCEEDED') return { state: 'UNKNOWN' }
  assert.equal(r.resource_id, reader.config.eventId); assert.equal(r.resource_revision, command.body.expected_revision + 1)
  assert.equal(r.result_status, 200); assert.equal(r.failure_reason, null)
  assert(snapshot.revision >= r.resource_revision!)
  const result = await reader.call('getOwnConcertMuralAction', undefined, { idempotencyKey: command.key })
  if (result.status !== 200) return { state: 'UNKNOWN' }
  const action = bindAction(reader.contracts, result.value, command, reader.account.userId, reader.config.eventId, r)
  if (command.operation_id === 'placeConcertPixel') {
    const own = await reader.call('getConcertMuralOwnResult', undefined, { key: command.key })
    if (own.status !== 200) return { state: 'UNKNOWN' }
    bindOwnResult(reader.contracts.decode('Domain_mural_OwnPixelResult', own.value), r, command.body, reader.account.userId, reader.config.eventId, command.key, snapshot.server_time)
  }
  return { state: 'SUCCEEDED', action, receipt: r }
}
/** Independent expected-effect oracle; no production preview helper is imported. */
export function connectedRegion(s: Pick<Snapshot, 'pixels'>, seed: number, cap = 256): number[] {
  assert(Number.isInteger(seed) && seed >= 0 && seed < 16384 && s.pixels.length === 16384)
  const pending = [seed], visited = new Set<number>(), cells: number[] = [], color = s.pixels[seed]
  while (pending.length) {
    const n = pending.pop()!
    if (visited.has(n)) continue
    visited.add(n)
    if (s.pixels[n] !== color) continue
    cells.push(n)
    if (cells.length > cap) throw new NotRun('LIVE_BOUNDED_REGION_EXCEEDS_256')
    if (n % 128) pending.push(n - 1)
    if (n % 128 !== 127) pending.push(n + 1)
    if (n >= 128) pending.push(n - 128)
    if (n < 16256) pending.push(n + 128)
  }
  return cells.sort((a, b) => a - b)
}
export function boundedSeed(s: Snapshot, maxCells: number): number {
  // Find one naturally existing non-background region; no seed mutation or fabricated raster.
  const rejected = new Set<number>()
  for (let seed = 0; seed < 16384; seed++) {
    if (!s.pixels[seed] || rejected.has(seed)) continue
    try {
      const cells = connectedRegion(s, seed, 16384)
      if (cells.length <= Math.min(256, maxCells)) return seed
      cells.forEach(n => rejected.add(n))
    } catch { /* The actual seed must remain a valid bounded authoritative grid. */ }
  }
  throw new NotRun('QUALIFIED_SEED_WITH_AFFORDABLE_BOUNDED_REGION_REQUIRED')
}
export function expectedEffect(before: Snapshot, command: Command, undoBefore?: Snapshot): number[] {
  const pixels = [...before.pixels]
  assert.equal(command.body.expected_revision, before.revision)
  if (command.operation_id === 'undoConcertMuralAction') {
    assert(undoBefore); return [...undoBefore.pixels]
  }
  const body = command.body, seed = body.y * 128 + body.x
  const color = command.operation_id === 'eraseConcertMuralPixel' ? 0 : command.body.color_index
  assert.notEqual(pixels[seed], color)
  const cells = command.operation_id === 'fillConcertMuralRegion' ? connectedRegion(before, seed) : [seed]
  cells.forEach(n => { pixels[n] = color })
  return pixels
}
export async function tools(actor: Actor, revision?: number) {
  const r = await actor.call('getConcertMuralTools'); assert.equal(r.status, 200)
  return bindTools(actor.contracts, r.value, actor.account.userId, actor.config.eventId, revision)
}
export async function preview(page: Page, command: Command) {
  const wall = page.getByRole('region', { name: 'Wall editor', exact: true })
  if (command.operation_id === 'undoConcertMuralAction') {
    await wall.getByRole('button', { name: 'Preview undo of my last action', exact: true }).click(); return
  }
  const label = command.operation_id === 'placeConcertPixel' ? 'Pencil preview' : command.operation_id === 'eraseConcertMuralPixel' ? 'Erase preview' : 'Fill preview'
  await wall.getByRole('button', { name: label, exact: true }).click()
  if ('color_index' in command.body) await wall.getByRole('button', { name: new RegExp(`^Color ${command.body.color_index} #[0-9A-F]{6}$`) }).click()
  const details = wall.locator('details')
  if (await details.getAttribute('open') === null) await details.locator('summary').click()
  await wall.getByLabel('X', { exact: true }).fill(String(command.body.x))
  await wall.getByLabel('Y', { exact: true }).fill(String(command.body.y))
  await wall.getByRole('button', { name: 'Preview pixel', exact: true }).click()
}
export const confirmLabels = { placeConcertPixel: 'Confirm pixel', eraseConcertMuralPixel: 'Confirm erase', fillConcertMuralRegion: 'Confirm fill', undoConcertMuralAction: 'Confirm undo' } as const
export async function rendered(page: Page, s: Snapshot) {
  const wall = page.getByRole('region', { name: 'Wall editor', exact: true })
  await wall.getByRole('button', { name: 'Fit wall', exact: true }).click()
  assert.equal(await wall.getByText(`REV ${s.revision} · ${s.state}`, { exact: true }).count(), 1)
  const pixels = await wall.locator('.mcr-mural__surface canvas').evaluate((canvas) => {
    const c = canvas as HTMLCanvasElement, r = c.getBoundingClientRect(), bytes = c.getContext('2d')!.getImageData(0, 0, c.width, c.height).data
    const top = (r.height - r.width) / 2, colors: string[] = []
    for (let i = 0; i < 16384; i++) {
      const x = Math.floor((i % 128 + .5) * r.width / 128 * c.width / r.width)
      const y = Math.floor((top + (Math.floor(i / 128) + .5) * r.width / 128) * c.height / r.height)
      const offset = (y * c.width + x) * 4
      if (bytes[offset + 3] !== 255) throw Error('OPAQUE_REAL_RASTER_REQUIRED')
      colors.push('#' + Array.from(bytes.slice(offset, offset + 3), n => n.toString(16).padStart(2, '0')).join('').toUpperCase())
    }
    return colors
  })
  assert.deepEqual(pixels, s.pixels.map(index => s.palette[index]))
}
export function requireInputs(config: GateConfig) {
  need(config.allowDestructiveFixtureEndState && config.isolated, 'EXCLUSIVE_ISOLATED_TERMINAL_EVENT_REQUIRED')
}
