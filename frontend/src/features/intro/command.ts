import { z } from 'zod'
import type { DomainOperationId } from '../../shared/api/domain-operations.generated'
import { domainOperation } from '../../shared/api/domain'
import { TransportError } from '../../shared/api/errors'
import type { JsonValue } from '../../shared/api/errors'
import type { SocialOptions, SocialRuntime } from '../people/runtime'
import type { PrivacyScope } from '../../shared/query/privacy'
import { freeze, revision, timestamp, uuid } from '../people/contract'
export type CommandPhase = 'idle' | 'sending' | 'pending' | 'uncertain' | 'checking' | 'reconciling' | 'succeeded' | 'failed' | 'conflict' | 'retired'
export interface CommandState { phase: CommandPhase; message: string; locked: boolean }
export const idleCommand = (): CommandState => ({ phase: 'idle', message: '', locked: false })
const receiptSchema = z.strictObject({ receipt_id: uuid, actor_id: uuid, event_id: uuid.nullable(), operation_id: z.string().min(1).max(100), idempotency_key: uuid,
  state: z.enum(['PENDING', 'SUCCEEDED', 'FAILED', 'RETIRED']), resource_id: uuid.nullable(), resource_revision: revision.nullable(), failure_reason: z.string().min(1).nullable(), created_at: timestamp, expires_at: timestamp, result_status: z.number().int().min(200).max(299).nullable() })
type Receipt = z.infer<typeof receiptSchema>
interface Binding { scope: PrivacyScope; operation: DomainOperationId; key: string; target: string | null }
function decodeReceipt(value: unknown, b: Binding, pending: boolean) {
  const r = receiptSchema.parse(value), op = domainOperation(b.operation)!
  if (r.actor_id !== b.scope.userId || r.event_id !== b.scope.eventId || r.operation_id !== b.operation || r.idempotency_key !== b.key
    || Date.parse(r.created_at) >= Date.parse(r.expires_at) || pending && r.state !== 'PENDING') throw new Error('Invalid receipt binding')
  if (r.state === 'SUCCEEDED') {
    if (!r.resource_id || !r.resource_revision || r.result_status !== 200 || r.failure_reason !== null || b.target && r.resource_id !== b.target) throw new Error('Invalid receipt success')
  } else if (r.state === 'FAILED') {
    if (!r.failure_reason || !Object.values(op.errors).flat().includes(r.failure_reason) || r.result_status !== null || r.resource_id !== null || r.resource_revision !== null) throw new Error('Invalid failed receipt')
  } else if (r.failure_reason !== null || r.result_status !== null || r.resource_id !== null || r.resource_revision !== null) throw new Error('Invalid receipt state')
  return freeze(r)
}
/** One original command identity. No retry API exists, and inaccessible status never unlocks a new key. */
export function createCommand<T>(runtime: SocialRuntime, options: SocialOptions, changed: (state: CommandState) => void, accepted: (value: T) => void,
  reread: (id: string, minimumRevision: number) => Promise<T>) {
  let state = idleCommand(), binding: Binding | null = null, serial = 0
  const now = options.now ?? Date.now
  const emit = (patch: Partial<CommandState>) => { state = freeze({ ...state, ...patch }); changed(state) }
  const current = (b: Binding, version: number) => binding === b && serial === version && runtime.current(b.scope)
  async function receipt(r: Receipt, b: Binding, version: number) {
    if (!current(b, version)) return
    if (r.state === 'RETIRED' || Date.parse(r.expires_at) <= now()) { emit({ phase: 'retired', locked: true, message: 'The original receipt is expired or retired. Its outcome cannot authorize another submission.' }); return }
    if (r.state === 'PENDING') { emit({ phase: 'pending', locked: true, message: 'The command is pending. Check its status explicitly.' }); return }
    if (r.state === 'FAILED') { emit({ phase: 'failed', locked: true, message: 'The original command failed. Inspect current information; this receipt does not authorize a replacement command.' }); return }
    emit({ phase: 'reconciling', locked: true, message: 'The server confirmed execution. Reading current authorized state…' })
    if (!current(b, version)) return
    const value = await reread(r.resource_id!, r.resource_revision!)
    if (current(b, version)) { accepted(value); if (!current(b, version)) return; binding = null; emit({ phase: 'succeeded', locked: false, message: 'Current server state verified.' }) }
  }
  return { read: () => state,
    interrupt() { if (binding && state.locked && !['failed', 'retired'].includes(state.phase)) { serial++; emit({ phase: 'uncertain', locked: true, message: 'Access changed during the command. Its outcome is unknown; check the original command when access returns.' }) } },
    reset() { serial++; binding = null; state = idleCommand(); changed(state) },
    async submit(input: { operation: DomainOperationId; path: string; method: 'PUT' | 'POST'; json: JsonValue; target: string | null; decode(value: unknown): T; safety?: boolean }) {
      if (!runtime.usable() || state.locked || (!input.safety && !runtime.writable())) return false
      const scope = runtime.scope()!, key = (options.createId ?? (() => crypto.randomUUID()))(); uuid.parse(key)
      const b: Binding = { scope, key, operation: input.operation, target: input.target }, version = ++serial; binding = b
      emit({ phase: 'sending', locked: true, message: 'Sending…' })
      try {
        if (!current(b, version)) return false
        const value = await runtime.run('command', () => ({ path: input.path, method: input.method, access: 'protected', errorFamily: 'domain', domainOperation: b.operation, idempotencyKey: key, body: { json: input.json },
          decode: (value, context): { result: T } | { receipt: Receipt } => context.status === 202 ? { receipt: decodeReceipt(value, b, true) } : { result: input.decode(value) } }))
        if (!current(b, version)) return false
        if ('receipt' in value) await receipt(value.receipt, b, version)
        else { accepted(value.result); if (!current(b, version)) return false; binding = null; emit({ phase: 'succeeded', locked: false, message: 'Change confirmed by the server.' }) }
        return true
      } catch (error) {
        if (!current(b, version)) return false
        const e = error instanceof TransportError ? error : null, reason = e?.server?.reason
        const uncertain = !e || e.uncertain || !e.status || e.status >= 500 || e.status === 404 || reason === 'IDEMPOTENCY_CONFLICT' || reason === 'IDEMPOTENCY_EXPIRED'
        if (uncertain) emit({ phase: 'uncertain', locked: true, message: 'The outcome is unknown. Your draft is preserved. Check the original command before any new submission.' })
        else { binding = null; emit({ phase: reason === 'VERSION_CONFLICT' ? 'conflict' : 'failed', locked: false, message: reason === 'VERSION_CONFLICT' ? 'The introduction changed. Your draft is preserved. Load and review the latest version.' : reason === 'PROVIDER_UNAVAILABLE' ? 'Verification provider unavailable. No verification was granted.' : 'The change was not confirmed. Your draft is preserved.' }) }
        return false
      }
    },
    async check() {
      const b = binding; if (!b || !runtime.current(b.scope) || ['sending', 'checking', 'retired'].includes(state.phase)) return
      const version = ++serial; emit({ phase: 'checking', locked: true, message: 'Checking the original command…' })
      try {
        if (!current(b, version)) return
        const r = await runtime.run('command-status', () => ({ path: `/api/concert/events/${b.scope.eventId}/commands`, method: 'GET', access: 'protected', errorFamily: 'domain', domainOperation: 'getConcertCommandStatus', query: { operation_id: b.operation, idempotency_key: b.key }, decode: value => decodeReceipt(value, b, false) }))
        if (current(b, version)) await receipt(r, b, version)
      } catch { if (current(b, version)) emit({ phase: 'uncertain', locked: true, message: 'Command status is unavailable. This does not prove whether it ran. Do not repeat the command.' }) }
    },
  }
}
