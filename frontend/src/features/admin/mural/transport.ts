import { z } from 'zod'
import type { HttpClient, HttpRequest } from '../../../shared/api/http'
import { TransportError } from '../../../shared/api/errors'
import { DOMAIN_OPERATIONS } from '../../../shared/api/domain-operations.generated'
import type { AdminCommand, OperatorContext, OperatorContextPort, PrivacyPort } from './contract'
import { qualifiedContext, revision, sameContext, sameScope, utc, uuid } from './contract'

export const receiptSchema = z.strictObject({ receipt_id: uuid, event_id: uuid.nullable(), actor_id: uuid, operation_id: z.string().min(1).max(100), idempotency_key: uuid,
  state: z.enum(['PENDING', 'SUCCEEDED', 'FAILED', 'RETIRED']), resource_id: uuid.nullable(), resource_revision: revision.nullable(), failure_reason: z.string().min(1).max(100).nullable(),
  created_at: utc, expires_at: utc, result_status: z.number().int().min(200).max(299).nullable() })
export type Receipt = z.infer<typeof receiptSchema>
export type Outcome<T> = { kind: 'confirmed'; data: T } | { kind: 'pending' | 'unknown' } | { kind: 'rejected'; reason: string }
export function createAdminTransport({ http, privacy, context, now = Date.now }: {
  http: Pick<HttpClient, 'request'>; privacy: PrivacyPort; context: OperatorContextPort; now?: () => number
}) {
  let retryAt = 0, retryScope: string | null = null, disposed = false
  const stopPrivacy = privacy.registerCleanup(() => { retryAt = 0; retryScope = null })
  const current = (lease: OperatorContext, signal: AbortSignal) => { const active = qualifiedContext(privacy, context); return !disposed && !signal.aborted && !!active && sameContext(active, lease) }
  function receipt(input: unknown, command: AdminCommand) {
    const r = receiptSchema.parse(input), errors: readonly string[] = Object.values(DOMAIN_OPERATIONS[command.operation].errors).flat()
    if (r.actor_id !== command.scope.userId || r.event_id !== command.scope.eventId || r.operation_id !== command.operation || r.idempotency_key !== command.key
      || Date.parse(r.created_at) >= Date.parse(r.expires_at) || Date.parse(r.expires_at) <= now()) throw new Error('Receipt binding unavailable')
    if (r.state === 'SUCCEEDED') {
      if (!r.resource_id || !r.resource_revision || r.result_status !== 200 || r.failure_reason !== null
        || command.operation !== 'closeConcertEvent' && (r.resource_id !== command.scope.eventId || r.resource_revision !== Number(command.body.expected_revision) + 1)) throw new Error('Invalid success receipt')
    } else if (r.state === 'FAILED') {
      if (!r.failure_reason || !errors.includes(r.failure_reason) || r.resource_id !== null || r.resource_revision !== null || r.result_status !== null) throw new Error('Invalid failed receipt')
    } else if (r.resource_id !== null || r.resource_revision !== null || r.result_status !== null || r.failure_reason !== null) throw new Error('Invalid pending receipt')
    return Object.freeze(r)
  }
  async function request<T>(lease: OperatorContext, signal: AbortSignal, input: Omit<HttpRequest<T>, 'access' | 'errorFamily' | 'signal'>) {
    if (!current(lease, signal)) throw new Error('Operator authority unavailable')
    const nextScope = JSON.stringify(lease.scope)
    if (retryScope !== nextScope) { retryScope = nextScope; retryAt = 0 }
    if (now() < retryAt) throw new Error('Retry window unavailable')
    const result = await http.request({ ...input, access: 'protected', errorFamily: 'domain', signal,
      decode(value, ctx) {
        if (!current(lease, signal) || ctx.path !== input.path || ctx.scope.userId !== lease.scope.userId || ctx.scope.eventId !== lease.scope.eventId
          || ctx.scope.generation !== lease.scope.generation) throw new Error('Unverified operator response')
        return input.decode(value, ctx)
      } }).catch((error: unknown) => {
        if (current(lease, signal) && error instanceof TransportError && error.status === 429) retryAt = now() + Math.min(86400000, Math.max(1000, error.retryAfterMs ?? 1000))
        throw error
      })
    if (!current(lease, signal)) throw new Error('Operator authority changed')
    return result.data
  }
  async function post<T>(lease: OperatorContext, signal: AbortSignal, command: AdminCommand, path: string, decode: (input: unknown) => T): Promise<Outcome<T>> {
    try {
      if (!sameScope(command.scope, lease.scope) || command.roleRevision !== lease.roleRevision || !uuid.safeParse(command.key).success) throw new Error('Invalid command context')
      return await request<Outcome<T>>(lease, signal, { method: 'POST', path, domainOperation: command.operation, idempotencyKey: command.key, body: { json: command.body },
        decode(input, ctx) {
          if (ctx.status === 202) { const r = receipt(input, command); if (r.state !== 'PENDING') throw new Error('Invalid pending response'); return { kind: 'pending' } }
          if (ctx.status !== 200) throw new Error('Invalid mutation status')
          return { kind: 'confirmed', data: decode(input) }
        } })
    } catch (e) {
      if (current(lease, signal) && e instanceof TransportError && e.kind === 'http' && !e.uncertain && e.status !== null && e.status >= 400 && e.status < 500 && e.server?.reason
        && !['IDEMPOTENCY_CONFLICT', 'IDEMPOTENCY_EXPIRED'].includes(e.server.reason)) return { kind: 'rejected', reason: e.server.reason }
      return { kind: 'unknown' }
    }
  }
  return { request, post, receipt,
    dispose() { disposed = true; retryAt = 0; retryScope = null; stopPrivacy() },
    status: (lease: OperatorContext, signal: AbortSignal, command: AdminCommand) => request(lease, signal, { method: 'GET', path: `/api/concert/events/${lease.scope.eventId}/commands`,
      domainOperation: 'getConcertCommandStatus', query: { operation_id: command.operation, idempotency_key: command.key },
      decode(input, ctx) { if (ctx.status !== 200 || !sameScope(command.scope, lease.scope) || command.roleRevision !== lease.roleRevision) throw new Error('Invalid command status'); return receipt(input, command) } }),
  }
}
export type AdminTransport = ReturnType<typeof createAdminTransport>
