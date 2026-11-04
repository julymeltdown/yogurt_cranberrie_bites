import { z } from 'zod'
import type { DecodeContext, HttpClient } from '../../../shared/api/http'
import { TransportError } from '../../../shared/api/errors'
import { DOMAIN_OPERATIONS } from '../../../shared/api/domain-operations.generated'
import type { MuralCommandPort, PixelCommand, PrivacyPort } from '../editor/contract'
import { placePixelSchema } from '../editor/contract'
import { bindingSchema, deltaSchema, matchesPixel, pixelOperationSchema, revision, sameBinding, sameScope, snapshotSchema, uuid } from './contract'
import type { MuralAuthorityPort, MuralReadLease, MuralReadPort } from './contract'

const utc = pixelOperationSchema.shape.committed_at
export const receiptSchema = z.strictObject({ receipt_id: uuid, event_id: uuid.nullable(), actor_id: uuid,
  operation_id: z.string().min(1).max(100), idempotency_key: uuid, state: z.enum(['PENDING', 'SUCCEEDED', 'FAILED', 'RETIRED']),
  resource_id: uuid.nullable(), resource_revision: revision.nullable(), failure_reason: z.string().max(100).nullable(),
  created_at: utc, expires_at: utc, result_status: z.number().int().min(200).max(299).nullable() })
export type MuralReceipt = z.infer<typeof receiptSchema>
export const ownPixelResultSchema = z.strictObject({ receipt_id: uuid, idempotency_key: uuid, result: pixelOperationSchema })
export type OwnPixelResult = z.infer<typeof ownPixelResultSchema>
export function createMuralHttp({ http, privacy, authority, now = Date.now }: {
  http: HttpClient; privacy: PrivacyPort; authority: MuralAuthorityPort; now?: () => number
}): { reads: MuralReadPort; commands: MuralCommandPort } {
  const capture = () => {
    const scope = Object.freeze({ ...privacy.capture() }), active = authority.read()
    if (!active || !privacy.isCurrent(scope) || !scope.userId || !scope.eventId || !bindingSchema.safeParse(active.binding).success || active.binding.eventId !== scope.eventId) throw new Error('Mural authority unavailable')
    return { scope, binding: active.binding, generation: 0 }
  }
  const current = (lease: MuralReadLease, signal: AbortSignal) => !signal.aborted && privacy.isCurrent(lease.scope)
    && sameScope(lease.scope, privacy.capture()) && sameBinding(authority.read()?.binding ?? null, lease.binding)
  const assert = (lease: MuralReadLease, signal: AbortSignal) => { if (!current(lease, signal)) throw new Error('Mural authority changed') }
  const context = (ctx: DecodeContext, lease: MuralReadLease, signal: AbortSignal, path: string, statuses: readonly number[]) => {
    assert(lease, signal)
    if (ctx.path !== path || !statuses.includes(ctx.status) || ctx.scope.userId !== lease.scope.userId || ctx.scope.eventId !== lease.scope.eventId
      || ctx.scope.generation !== lease.scope.generation) throw new Error('Unverified mural response context')
  }
  const base = (lease: MuralReadLease) => `/api/concert/events/${lease.scope.eventId}`
  const receipt = (input: unknown, command: PixelCommand) => {
    const r = receiptSchema.parse(input)
    if (r.actor_id !== command.scope.userId || r.event_id !== command.scope.eventId || r.operation_id !== command.operation || r.idempotency_key !== command.key
      || Date.parse(r.created_at) >= Date.parse(r.expires_at) || Date.parse(r.expires_at) <= now()) throw new Error('Unverified mural receipt binding')
    if (r.state === 'SUCCEEDED') {
      if (r.resource_id !== command.scope.eventId || r.resource_revision !== command.body.expected_revision + 1 || r.result_status !== 200 || r.failure_reason !== null) throw new Error('Unverified mural success')
    } else if (r.state === 'FAILED') {
      const allowed = Object.values(DOMAIN_OPERATIONS.placeConcertPixel.errors).flat() as readonly string[]
      if (!r.failure_reason || !allowed.includes(r.failure_reason) || r.resource_id !== null || r.resource_revision !== null || r.result_status !== null) throw new Error('Unverified mural failure')
    } else if (r.resource_id !== null || r.resource_revision !== null || r.result_status !== null || r.failure_reason !== null) throw new Error('Unverified pending mural receipt')
    return Object.freeze(r)
  }
  const commandLease = (command: PixelCommand, signal: AbortSignal, write: boolean) => {
    const lease = capture()
    if (!sameScope(command.scope, lease.scope) || command.operation !== 'placeConcertPixel' || !uuid.safeParse(command.key).success
      || !placePixelSchema.safeParse(command.body).success || write && !authority.read()?.writable) throw new Error('Mural command unavailable')
    assert(lease, signal); return lease
  }
  return {
    reads: {
      async snapshot({ lease, signal }) {
        assert(lease, signal); const path = `${base(lease)}/mural/snapshot`
        const result = await http.request({ method: 'GET', path, access: 'protected', errorFamily: 'domain', domainOperation: 'getConcertMuralSnapshot', signal,
          decode(value, ctx) { context(ctx, lease, signal, path, [200]); const data = snapshotSchema.parse(value); if (data.event_id !== lease.scope.eventId) throw new Error('Wrong mural event'); return data } })
        assert(lease, signal); return result.data
      },
      async delta({ lease, signal, afterRevision }) {
        assert(lease, signal); revision.parse(afterRevision); const path = `${base(lease)}/mural/operations`
        const result = await http.request({ method: 'GET', path, access: 'protected', errorFamily: 'domain', domainOperation: 'getConcertMuralDelta', query: { after_revision: afterRevision }, signal,
          decode(value, ctx) { context(ctx, lease, signal, path, [200]); const data = deltaSchema.parse(value)
            if (data.event_id !== lease.scope.eventId || data.from_revision !== afterRevision) throw new Error('Wrong mural delta baseline'); return data } })
        assert(lease, signal); return result.data
      },
    },
    commands: {
      async place(command, signal) {
        let pendingLease: MuralReadLease | null = null
        try {
          const lease = commandLease(command, signal, true), path = `${base(lease)}/mural/pixels`
          pendingLease = lease
          const response = await http.request({ method: 'POST', path, access: 'protected', errorFamily: 'domain', domainOperation: 'placeConcertPixel',
            idempotencyKey: command.key, body: { json: command.body }, signal,
            decode(value, ctx) { context(ctx, lease, signal, path, [200, 202])
              if (ctx.status === 202) { const r = receipt(value, command); if (r.state !== 'PENDING') throw new Error('Invalid pending response'); return { kind: 'pending' as const } }
              const pixel = matchesPixel(value, command); if (!pixel) throw new Error('Unverified pixel result'); return { kind: 'acknowledged' as const, pixel }
            } })
          assert(lease, signal); return response.data
        } catch (error) {
          // Only the shared transport's verified definitive 4xx envelope proves rejection.
          if (pendingLease && current(pendingLease, signal) && error instanceof TransportError && error.kind === 'http'
            && error.status !== null && error.status >= 400 && error.status < 500 && !error.uncertain && error.server?.reason) return { kind: 'rejected', reason: error.server.reason }
          return { kind: 'unknown' }
        }
      },
      async check(command, signal) {
        try {
          const lease = commandLease(command, signal, false), path = `${base(lease)}/commands`
          const response = await http.request({ method: 'GET', path, access: 'protected', errorFamily: 'domain', domainOperation: 'getConcertCommandStatus',
            query: { operation_id: command.operation, idempotency_key: command.key }, signal,
            decode(value, ctx) { context(ctx, lease, signal, path, [200]); return receipt(value, command) } })
          assert(lease, signal); const r = response.data
          if (r.state === 'PENDING') return { kind: 'pending' }
          if (r.state === 'FAILED') return { kind: 'rejected', reason: r.failure_reason! }
          if (r.state !== 'SUCCEEDED') return { kind: 'unknown' }
          const resultPath = `${base(lease)}/mural/commands/${command.key}/result`
          const original = await http.request({ method: 'GET', path: resultPath, access: 'protected', errorFamily: 'domain', domainOperation: 'getConcertMuralOwnResult', signal,
            decode(value, ctx) {
              context(ctx, lease, signal, resultPath, [200]); const envelope = ownPixelResultSchema.parse(value)
              const p = matchesPixel(envelope.result, command)
              if (envelope.receipt_id !== r.receipt_id || envelope.idempotency_key !== command.key || !p
                || p.revision !== r.resource_revision) throw new Error('Unverified original mural result')
              return p
            } })
          assert(lease, signal); return { kind: 'acknowledged', pixel: original.data }
        } catch { return { kind: 'unknown' } }
      },
    },
  }
}
