import { QueryClient } from '@tanstack/react-query'
import { createHttpClient, createMemorySession } from '../../../../shared/api/http'
import { createQueryPrivacy } from '../../../../shared/query/privacy'
import { createAdminTransport } from '../transport'
import { createMuralAdminApi } from '../http'
import { createMuralControl } from '../controller'
import { createClosureApi } from '../../closure/http'
import { createEventClosure } from '../../closure/controller'
import { stepNames } from '../../closure/contract'
import type { ClosureJob } from '../../closure/contract'
import type { MuralAdminState, OperatorContext } from '../contract'
export const ids = { event: '00000000-0000-4000-8000-000000000001', actor: '00000000-0000-4000-8000-000000000002',
  snapshot: '00000000-0000-4000-8000-000000000003', restore: '00000000-0000-4000-8000-000000000004',
  job: '00000000-0000-4000-8000-000000000050', receipt: '00000000-0000-4000-8000-000000000060' }
export const wall = (): MuralAdminState => ({ event_id: ids.event, revision: 10, state: 'EDITABLE', snapshot_id: ids.snapshot, locks: [] })
export const job = (state: ClosureJob['state'] = 'PARTIAL_FAILURE', revision = 3): ClosureJob => ({ job_id: ids.job, event_id: ids.event, state, revision,
  steps: stepNames.map((name, i) => ({ name, state: state === 'COMPLETED' || i < 3 ? 'SUCCEEDED' : state === 'PARTIAL_FAILURE' && i === 5 ? 'FAILED' : 'PENDING' })) })
export function fixture() {
  let key = 100, clock = Date.parse('2026-11-04T00:00:00Z')
  const session = createMemorySession(); session.replace(0, { userId: ids.actor, eventId: ids.event, accessToken: 'fixture_access' })
  const privacy = createQueryPrivacy({ client: new QueryClient(), session }), listeners = new Set<() => void>()
  let context: OperatorContext | null = { scope: privacy.capture(), role: 'OPERATOR', roleRevision: 1, eventRevision: 8, phase: 'LIVE' }
  const authority = { read: () => context, subscribe(f: () => void) { listeners.add(f); return () => { listeners.delete(f) } } }
  const server = { wall: wall(), job: job(), mode: 'ack', operator: true, eventRevision: 8, eventPhase: 'LIVE', requests: [] as Request[], receipts: new Map<string, Record<string, unknown>>(),
    transform: (value: unknown, _request: Request): unknown => value,
    intercept: null as null | ((request: Request) => Promise<Response> | null) }
  const reply = (value: unknown, request: Request, status = 200) => Response.json(server.transform(value, request), { status })
  const error = (reason: string, status: number) => Response.json({ code: 'ERROR', message: 'safe fixture', details: { reason, ...(reason === 'RATE_LIMITED' ? { retry_after_ms: 1000 } : {}) } }, { status })
  const fetch: typeof globalThis.fetch = async input => {
    const request = input as Request, url = new URL(request.url); server.requests.push(request.clone())
    const intercepted = server.intercept?.(request); if (intercepted) return intercepted
    if (!server.operator) return error('PERMISSION_DENIED', 403)
    if (url.pathname.endsWith('/commands')) {
      const r = server.receipts.get(url.searchParams.get('idempotency_key')!)
      return r ? reply(r, request) : error('RESOURCE_UNAVAILABLE', 404)
    }
    if (request.method === 'GET') {
      if (url.pathname.endsWith('/admin/mural')) return reply(server.wall, request)
      if (url.pathname.endsWith(`/closure/${ids.job}`)) return reply(server.job, request)
      return error('RESOURCE_UNAVAILABLE', 404)
    }
    if (server.mode === 'forbidden') return error('PERMISSION_DENIED', 403)
    if (server.mode === 'conflict') return error('VERSION_CONFLICT', 409)
    if (server.mode === 'throttle') return error('RATE_LIMITED', 429)
    if (server.mode === 'html') return new Response('<html>private</html>', { status: 503 })
    const body = await request.clone().json(), closure = url.pathname.endsWith('/closure')
    const operation = closure ? 'closeConcertEvent' : url.pathname.endsWith('/control') ? 'controlConcertMural' : url.pathname.endsWith('/locks') ? 'lockConcertMuralRegion' : 'restoreConcertMural'
    if (!closure && body.expected_revision !== server.wall.revision) return error('VERSION_CONFLICT', 409)
    if (closure) {
      if (!['OPEN', 'LIVE'].includes(server.eventPhase)) return error('EVENT_READ_ONLY', 409)
      if (body.expected_revision !== server.eventRevision) return error('VERSION_CONFLICT', 409)
      server.eventRevision++; server.eventPhase = 'READ_ONLY'
    }
    if (!closure) {
      server.wall = { ...server.wall, revision: server.wall.revision + 1,
        state: body.action === 'PAUSE' || operation === 'restoreConcertMural' ? 'PAUSED' : body.action === 'RESUME' ? 'EDITABLE' : body.action === 'FREEZE' ? 'FROZEN' : server.wall.state,
        snapshot_id: operation === 'restoreConcertMural' ? body.snapshot_id : server.wall.snapshot_id }
      if (operation === 'lockConcertMuralRegion') server.wall.locks = body.locked ? [{ x_min: body.x_min, y_min: body.y_min, x_max_exclusive: body.x_max_exclusive, y_max_exclusive: body.y_max_exclusive }] : []
    }
    const pending = server.mode === 'pending'
    const receipt = { receipt_id: ids.receipt, actor_id: ids.actor, event_id: ids.event, operation_id: operation, idempotency_key: request.headers.get('Idempotency-Key'),
      state: pending ? 'PENDING' : 'SUCCEEDED', resource_id: pending ? null : closure ? ids.job : ids.event, resource_revision: pending ? null : closure ? server.job.revision : server.wall.revision,
      result_status: pending ? null : 200, failure_reason: null, created_at: '2026-11-04T00:00:00Z', expires_at: '2026-11-05T00:00:00Z' }
    server.receipts.set(String(receipt.idempotency_key), receipt)
    if (server.mode === 'lost') throw new TypeError('response lost after durable commit')
    return reply(pending ? receipt : closure ? server.job : server.wall, request, pending ? 202 : 200)
  }
  const http = createHttpClient({ session, fetch, timeoutMs: 1000 }), transport = createAdminTransport({ http, privacy, context: authority, now: () => clock })
  const muralApi = createMuralAdminApi(transport), closureApi = createClosureApi(transport)
  const newKey = () => `00000000-0000-4000-8000-${String(++key).padStart(12, '0')}`
  const mural = createMuralControl({ privacy, context: authority, api: muralApi, newKey }), closure = createEventClosure({ privacy, context: authority, api: closureApi, newKey })
  return { session, privacy, authority, server, http, transport, muralApi, closureApi, mural, closure,
    advance(ms: number) { clock += ms },
    update(next: OperatorContext | null) { context = next; for (const f of Array.from(listeners)) f() },
    context: () => context!,
    posts: () => server.requests.filter(r => r.method === 'POST'),
    cleanup() { mural.dispose(); closure.dispose(); transport.dispose(); privacy.dispose() },
  }
}
