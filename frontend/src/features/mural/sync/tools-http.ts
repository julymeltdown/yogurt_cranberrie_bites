import { z } from 'zod'
import type { DecodeContext, HttpClient } from '../../../shared/api/http'
import { TransportError } from '../../../shared/api/errors'
import { DOMAIN_OPERATIONS } from '../../../shared/api/domain-operations.generated'
import type { PrivacyScope } from '../../../shared/query/privacy'
import type { PrivacyPort } from '../editor/contract'
import { placePixelSchema } from '../editor/contract'
import { eraseBodySchema, fillBodySchema, undoBodySchema, recoveredCommandSchema, matchActionReceipt, WALL_TOOL_FRESH_MS } from '../editor/tools'
import type { RecoveredCommand, WallActionCommand, WallToolPort } from '../editor/tools'
import { bindingSchema, pixelOperationSchema, revision, sameBinding, sameScope, uuid } from './contract'
import type { MuralAuthorityPort, MuralReadLease } from './contract'
import { receiptSchema } from './http'

// Closed decoders follow BE202 c18d5ba9; generated registry is imported byte-for-byte.
const utc = pixelOperationSchema.shape.committed_at
const actionBody = z.discriminatedUnion('operation_id', [
  z.strictObject({ operation_id: z.literal('placeConcertPixel'), body: placePixelSchema }),
  z.strictObject({ operation_id: z.literal('eraseConcertMuralPixel'), body: eraseBodySchema }),
  z.strictObject({ operation_id: z.literal('fillConcertMuralRegion'), body: fillBodySchema }),
  z.strictObject({ operation_id: z.literal('undoConcertMuralAction'), body: undoBodySchema }),
])
export const ownWallActionSchema = z.strictObject({ action_id: uuid, receipt_id: uuid, event_id: uuid, actor_id: uuid,
  command: actionBody, idempotency_key: uuid, kind: z.enum(['PENCIL', 'ERASE', 'FILL', 'UNDO']), revision,
  changed_cells: z.number().int().min(1).max(256), committed_at: utc, next_allowed_at: utc, undo_expires_at: utc, reset_required: z.boolean() })
export const wallToolsWireSchema = z.strictObject({ version: z.literal(1), event_id: uuid, actor_id: uuid, revision, server_time: utc,
  background_index: z.literal(0).nullable(), fill_limit: z.literal(256), cell_allowance: z.literal(512), window_seconds: z.literal(600),
  remaining_cells: z.number().int().min(0).max(512), next_cell_available_at: utc.nullable(), next_allowed_at: utc.nullable(), editable: z.boolean(),
  available: z.strictObject({ pencil: z.boolean(), erase: z.boolean(), fill: z.boolean(), undo: z.boolean() }), undo_window_seconds: z.literal(600),
  undo: z.strictObject({ action_id: uuid, revision, expires_at: utc, changed_cells: z.number().int().min(1).max(256) }).nullable(),
  latest_action: ownWallActionSchema.nullable(),
  unresolved: z.strictObject({ command: actionBody, idempotency_key: uuid, receipt_id: uuid }).nullable(),
})
const names = { erase: 'eraseConcertMuralPixel', fill: 'fillConcertMuralRegion', undo: 'undoConcertMuralAction' } as const
const normalized = { placeConcertPixel: 'placeConcertPixel', eraseConcertMuralPixel: 'erase', fillConcertMuralRegion: 'fill', undoConcertMuralAction: 'undo' } as const
const kinds = { placeConcertPixel: 'PENCIL', eraseConcertMuralPixel: 'ERASE', fillConcertMuralRegion: 'FILL', undoConcertMuralAction: 'UNDO' } as const
function original(command: z.infer<typeof actionBody>, key: string): RecoveredCommand {
  return recoveredCommandSchema.parse({ operation: normalized[command.operation_id], key, body: command.body })
}
function sameBody(left: object, right: object) {
  const keys = Object.keys(left).sort(), other = Object.keys(right).sort()
  return keys.length === other.length && keys.every((key, i) => key === other[i] && Reflect.get(left, key) === Reflect.get(right, key))
}
function ownAction(input: unknown, scope: PrivacyScope) {
  const action = ownWallActionSchema.parse(input)
  if (action.actor_id !== scope.userId || action.event_id !== scope.eventId || action.kind !== kinds[action.command.operation_id]
    || action.revision !== action.command.body.expected_revision + 1
    || Date.parse(action.next_allowed_at) < Date.parse(action.committed_at)
    || Date.parse(action.undo_expires_at) <= Date.parse(action.committed_at)
    || Date.parse(action.undo_expires_at) - Date.parse(action.committed_at) > 600000
    || (action.kind === 'PENCIL' || action.kind === 'ERASE') && action.changed_cells !== 1
    || action.kind !== 'PENCIL' && !action.reset_required) throw new Error('Unqualified own mural action')
  return action
}
export function createMuralToolsHttp({ http, privacy, authority, now = Date.now, monotonicNow = () => performance.now() }: {
  http: HttpClient; privacy: PrivacyPort; authority: MuralAuthorityPort; now?: () => number; monotonicNow?: () => number
}): WallToolPort {
  const capture = (expected: PrivacyScope, signal: AbortSignal, write = false): MuralReadLease => {
    const scope = Object.freeze({ ...privacy.capture() }), active = authority.read()
    if (signal.aborted || !privacy.isCurrent(scope) || !sameScope(scope, expected) || !scope.userId || !scope.eventId || !active
      || !bindingSchema.safeParse(active.binding).success || active.binding.eventId !== scope.eventId || write && !active.writable) throw new Error('Mural tools authority unavailable')
    return { scope, binding: active.binding, generation: 0 }
  }
  const current = (lease: MuralReadLease, signal: AbortSignal) => !signal.aborted && privacy.isCurrent(lease.scope)
    && sameScope(lease.scope, privacy.capture()) && sameBinding(authority.read()?.binding ?? null, lease.binding)
  const assert = (lease: MuralReadLease, signal: AbortSignal) => { if (!current(lease, signal)) throw new Error('Mural tools authority changed') }
  const context = (ctx: DecodeContext, lease: MuralReadLease, signal: AbortSignal, path: string, statuses: readonly number[]) => {
    assert(lease, signal)
    if (ctx.path !== path || !statuses.includes(ctx.status) || ctx.scope.userId !== lease.scope.userId || ctx.scope.eventId !== lease.scope.eventId
      || ctx.scope.generation !== lease.scope.generation) throw new Error('Unverified tool response context')
  }
  const base = (lease: MuralReadLease) => `/api/concert/events/${lease.scope.eventId}`
  const bindCommand = (command: WallActionCommand) => {
    const parsed = recoveredCommandSchema.parse(commandIdentity(command))
    if (parsed.operation === 'placeConcertPixel') throw new Error('Wrong action kind')
  }
  const bindResult = (value: unknown, command: WallActionCommand, receiptId?: string) => {
    const action = ownAction(value, command.scope)
    if (action.idempotency_key !== command.key || action.command.operation_id !== names[command.operation]
      || !sameBody(action.command.body, command.body) || receiptId !== undefined && action.receipt_id !== receiptId) throw new Error('Wrong original action')
    const receipt = matchActionReceipt({ actionId: action.action_id, eventId: action.event_id, actorId: action.actor_id,
      commandKey: action.idempotency_key, operation: command.operation, revision: action.revision, changedCells: action.changed_cells,
      committedAt: action.committed_at, nextAllowedAt: action.next_allowed_at, resetRequired: action.reset_required }, command)
    if (!receipt) throw new Error('Unverified original action receipt')
    return receipt
  }
  const bindReceipt = (value: unknown, command: WallActionCommand) => {
    const r = receiptSchema.parse(value), allowed = Object.values(DOMAIN_OPERATIONS[names[command.operation]].errors).flat() as readonly string[]
    if (r.actor_id !== command.scope.userId || r.event_id !== command.scope.eventId || r.operation_id !== names[command.operation]
      || r.idempotency_key !== command.key || Date.parse(r.created_at) >= Date.parse(r.expires_at) || Date.parse(r.expires_at) <= now()) throw new Error('Unverified command receipt')
    if (r.state === 'SUCCEEDED') {
      if (r.resource_id !== command.scope.eventId || r.resource_revision !== command.body.expected_revision + 1 || r.result_status !== 200 || r.failure_reason !== null) throw new Error('Wrong action success')
    } else if (r.state === 'FAILED') {
      if (!r.failure_reason || !allowed.includes(r.failure_reason) || r.resource_id !== null || r.resource_revision !== null || r.result_status !== null) throw new Error('Wrong action failure')
    } else if (r.resource_id !== null || r.resource_revision !== null || r.result_status !== null || r.failure_reason !== null) throw new Error('Wrong pending action')
    return r
  }
  return {
    async load(expected, signal) {
      const started = monotonicNow(), lease = capture(expected, signal), path = `${base(lease)}/mural/tools`
      const response = await http.request({ method: 'GET', path, access: 'protected', errorFamily: 'domain', domainOperation: 'getConcertMuralTools', signal,
        decode(value, ctx) {
          context(ctx, lease, signal, path, [200])
          if (monotonicNow() >= started + WALL_TOOL_FRESH_MS) throw new Error('Expired point-in-time capability response')
          const data = wallToolsWireSchema.parse(value)
          if (data.actor_id !== expected.userId || data.event_id !== expected.eventId) throw new Error('Wrong private tool scope')
          const latest = data.latest_action ? ownAction(data.latest_action, expected) : null, undo = data.undo
          if (latest && (latest.revision > data.revision || Date.parse(latest.committed_at) > Date.parse(data.server_time))) throw new Error('Future own action')
          if (undo && (!latest || latest.kind === 'UNDO' || undo.action_id !== latest.action_id || undo.revision !== latest.revision
            || undo.changed_cells !== latest.changed_cells || undo.expires_at !== latest.undo_expires_at || undo.revision !== data.revision)) throw new Error('Wrong undo eligibility')
          return { version: data.version, actorId: data.actor_id, eventId: data.event_id, revision: data.revision, serverTime: data.server_time,
            editable: data.editable, nextAllowedAt: data.next_allowed_at,
            capabilities: { ...data.available, backgroundIndex: data.background_index }, fillLimit: data.fill_limit, undoWindowMs: 600000,
            budget: { limit: 512, windowMs: 600000, remaining: data.remaining_cells, nextCellAvailableAt: data.next_cell_available_at },
            undoTarget: undo ? { actionId: undo.action_id, revision: undo.revision, changedCells: undo.changed_cells, expiresAt: undo.expires_at } : null,
            unresolved: data.unresolved ? original(data.unresolved.command, data.unresolved.idempotency_key) : null }
        } })
      assert(lease, signal)
      if (monotonicNow() >= started + WALL_TOOL_FRESH_MS) throw new Error('Expired point-in-time capability response')
      return response.data
    },
    async submit(command, signal) {
      let lease: MuralReadLease | null = null
      try {
        lease = capture(command.scope, signal, true); bindCommand(command)
        const bound = lease, path = `${base(bound)}/mural/${command.operation}`
        const response = await http.request({ method: 'POST', path, access: 'protected', errorFamily: 'domain', domainOperation: names[command.operation],
          idempotencyKey: command.key, body: { json: command.body }, signal,
          decode(value, ctx) {
            context(ctx, bound, signal, path, [200, 202])
            if (ctx.status === 202) { if (bindReceipt(value, command).state !== 'PENDING') throw new Error('Wrong pending result'); return { kind: 'pending' as const } }
            return { kind: 'acknowledged' as const, receipt: bindResult(value, command) }
          } })
        assert(bound, signal); return response.data
      } catch (error) {
        if (lease && current(lease, signal) && error instanceof TransportError && error.kind === 'http'
          && error.status !== null && error.status >= 400 && error.status < 500 && !error.uncertain && error.server?.reason) {
          // This 409 also represents a still-pending SAME original receipt.
          // Only GET status FAILED proves that this attempt can be retired.
          if (error.server.reason === 'MURAL_ACTION_UNRESOLVED') return { kind: 'unknown' }
          return { kind: 'rejected', reason: error.server.reason }
        }
        return { kind: 'unknown' }
      }
    },
    async check(command, signal) {
      try {
        const lease = capture(command.scope, signal); bindCommand(command)
        const path = `${base(lease)}/commands`
        const response = await http.request({ method: 'GET', path, access: 'protected', errorFamily: 'domain', domainOperation: 'getConcertCommandStatus',
          query: { operation_id: names[command.operation], idempotency_key: command.key }, signal,
          decode(value, ctx) { context(ctx, lease, signal, path, [200]); return bindReceipt(value, command) } })
        assert(lease, signal); const receipt = response.data
        if (receipt.state === 'PENDING') return { kind: 'pending' }
        if (receipt.state === 'FAILED') return { kind: 'rejected', reason: receipt.failure_reason! }
        if (receipt.state !== 'SUCCEEDED') return { kind: 'unknown' }
        const resultPath = `${base(lease)}/mural/actions/${command.key}`
        const result = await http.request({ method: 'GET', path: resultPath, access: 'protected', errorFamily: 'domain', domainOperation: 'getOwnConcertMuralAction', signal,
          decode(value, ctx) { context(ctx, lease, signal, resultPath, [200]); return bindResult(value, command, receipt.receipt_id) } })
        assert(lease, signal); return { kind: 'acknowledged', receipt: result.data }
      } catch { return { kind: 'unknown' } }
    },
  }
}
function commandIdentity(command: WallActionCommand) {
  return { operation: command.operation, key: command.key, body: command.body }
}
