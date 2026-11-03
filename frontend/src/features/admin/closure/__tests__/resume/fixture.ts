import { QueryClient } from '@tanstack/react-query'
import { createHttpClient, createMemorySession } from '../../../../../shared/api/http'
import { createQueryPrivacy } from '../../../../../shared/query/privacy'
import { createAdminTransport } from '../../../mural/transport'
import type { AdminCommand, OperatorContext } from '../../../mural/contract'
import { createClosureApi } from '../../http'
import { createEventClosure } from '../../controller'
import { closureCommandSchema, stepNames } from '../../contract'
import type { ClosureJob } from '../../contract'

export const ids = { actor: '00000000-0000-4000-8000-000000000002', other: '00000000-0000-4000-8000-000000000099',
  event: '00000000-0000-4000-8000-000000000001', job: '00000000-0000-4000-8000-000000000123' }
export const job = (state: ClosureJob['state'] = 'PARTIAL_FAILURE', revision = 8): ClosureJob => ({ job_id: ids.job, event_id: ids.event, revision, state,
  steps: stepNames.map((name, i) => ({ name, state: state === 'COMPLETED' || i < 5 ? 'SUCCEEDED' : state === 'PARTIAL_FAILURE' && name === 'EXPORT' ? 'FAILED' : 'PENDING' })) })
type StoredReceipt = { receipt_id: string; event_id: string; actor_id: string; operation_id: string; idempotency_key: string; state: string;
  resource_id: string | null; resource_revision: number | null; failure_reason: string | null; created_at: string; expires_at: string; result_status: number | null }
export function fixture({ existing = true, timeoutMs = 1000, newKey }: { existing?: boolean; timeoutMs?: number; newKey?: () => string } = {}) {
  let sequence = 1000, now = Date.parse('2026-11-04T01:00:00Z')
  const session = createMemorySession(), tokens = new Map<string, string>(), privacy = createQueryPrivacy({ client: new QueryClient(), session })
  function login(actor = ids.actor, event = ids.event) {
    const token = `fixture_${actor}`; tokens.set(token, actor)
    session.replace(session.read().generation, { userId: actor, eventId: event, accessToken: token })
  }
  login()
  let context: OperatorContext | null = { scope: privacy.capture(), role: 'OPERATOR', roleRevision: 1, eventRevision: 23, phase: existing ? 'READ_ONLY' : 'LIVE' }
  const listeners = new Set<() => void>(), authority = { read: () => context, subscribe(f: () => void) { listeners.add(f); return () => { listeners.delete(f) } } }
  const server = { job: job(), phase: existing ? 'READ_ONLY' : 'LIVE', eventRevision: 23, started: existing, initiator: ids.actor, operator: true, policy: true,
    mode: 'ack', admissions: 0, requests: [] as Request[], receipts: new Map<string, StoredReceipt>(), bodies: new Map<string, string>(),
    commands: new Map<string, ReturnType<typeof closureCommandSchema.parse>>(), effects: new Map(stepNames.map((name, i) => [name, i < 5 ? 1 : 0])),
    transform: (value: unknown, _request: Request): unknown => value,
    intercept: null as null | ((request: Request) => Promise<Response> | null) }
  const error = (reason: string, status: number) => Response.json({ code: 'FIXTURE', message: 'Safe deterministic error', details: { reason, ...(reason === 'RATE_LIMITED' ? { retry_after_ms: 1000 } : {}) } }, { status })
  function apply(key: string) {
    const body = server.commands.get(key)!, r = server.receipts.get(key)!
    if (body.confirmation === 'RESUME_CLOSURE') {
      server.job = { ...server.job, revision: server.job.revision + 1, state: 'RUNNING', steps: server.job.steps.map(s => s.state === 'FAILED' ? { ...s, state: 'PENDING' } : { ...s }) }
    } else {
      server.started = true; server.phase = 'READ_ONLY'; server.eventRevision++
      server.job = job('RUNNING', 1)
    }
    server.admissions++
    Object.assign(r, { state: 'SUCCEEDED', resource_id: ids.job, resource_revision: server.job.revision, result_status: 200 })
  }
  const fetch: typeof globalThis.fetch = async input => {
    const request = input as Request, url = new URL(request.url); server.requests.push(request.clone())
    const intercepted = server.intercept?.(request); if (intercepted) return intercepted
    const actor = tokens.get((request.headers.get('Authorization') ?? '').replace(/^Bearer /, ''))
    if (!actor) return error('AUTHENTICATION_REQUIRED', 401)
    if (!server.policy) return error('DEPENDENCY_UNAVAILABLE', 503)
    if (!server.operator) return error('PERMISSION_DENIED', 403)
    if (!url.pathname.startsWith(`/api/concert/events/${ids.event}/`)) return error('RESOURCE_UNAVAILABLE', 404)
    if (request.method === 'GET') {
      if (actor !== server.initiator) return error('RESOURCE_UNAVAILABLE', 404)
      if (url.pathname.endsWith('/commands')) {
        const r = server.receipts.get(url.searchParams.get('idempotency_key')!)
        return r && r.actor_id === actor && url.searchParams.get('operation_id') === r.operation_id ? Response.json(server.transform(r, request)) : error('RESOURCE_UNAVAILABLE', 404)
      }
      if (url.pathname.endsWith(`/closure/${ids.job}`) && server.started) return Response.json(server.transform(server.job, request))
      return error('RESOURCE_UNAVAILABLE', 404)
    }
    if (url.pathname !== `/api/concert/events/${ids.event}/admin/closure` || request.method !== 'POST') return error('RESOURCE_UNAVAILABLE', 404)
    const parsed = closureCommandSchema.safeParse(await request.clone().json())
    if (!parsed.success) return error('INVALID_INPUT', 400)
    const body = parsed.data, key = request.headers.get('Idempotency-Key')!, canonical = JSON.stringify(body)
    if (body.confirmation === 'RESUME_CLOSURE' && (actor !== server.initiator || body.job_id !== ids.job || !server.started)) return error('RESOURCE_UNAVAILABLE', 404)
    if (server.bodies.has(key)) {
      if (server.bodies.get(key) !== canonical) return error('IDEMPOTENCY_CONFLICT', 409)
      const r = server.receipts.get(key)!
      return r.state === 'PENDING' ? Response.json(server.transform(r, request), { status: 202 }) : Response.json(server.transform(server.job, request))
    }
    if (server.mode === 'conflict') return error('VERSION_CONFLICT', 409)
    if (server.mode === 'throttle') return error('RATE_LIMITED', 429)
    if (server.mode === 'unavailable') return error('DEPENDENCY_UNAVAILABLE', 503)
    if (body.confirmation === 'RESUME_CLOSURE') {
      if (server.phase !== 'READ_ONLY' || server.job.state !== 'PARTIAL_FAILURE') return error('INVALID_TRANSITION', 409)
      if (body.expected_revision !== server.job.revision) return error('VERSION_CONFLICT', 409)
    } else {
      if (!['OPEN', 'LIVE'].includes(server.phase) || server.started) return error('EVENT_READ_ONLY', 409)
      if (body.expected_revision !== server.eventRevision) return error('VERSION_CONFLICT', 409)
      server.initiator = actor
    }
    const r: StoredReceipt = { receipt_id: `00000000-0000-4000-8000-${String(++sequence).padStart(12, '0')}`, event_id: ids.event, actor_id: actor,
      operation_id: 'closeConcertEvent', idempotency_key: key, state: 'PENDING', resource_id: null, resource_revision: null, failure_reason: null,
      created_at: '2026-11-04T00:00:00Z', expires_at: '2026-11-05T00:00:00Z', result_status: null }
    server.receipts.set(key, r); server.bodies.set(key, canonical); server.commands.set(key, body)
    if (server.mode === 'pending') return Response.json(server.transform(r, request), { status: 202 })
    apply(key)
    if (server.mode === 'lost') throw new TypeError('Lost admission response')
    return Response.json(server.transform(server.job, request))
  }
  const http = createHttpClient({ session, fetch, timeoutMs }), transport = createAdminTransport({ http, privacy, context: authority, now: () => now }), api = createClosureApi(transport)
  const controller = createEventClosure({ privacy, context: authority, api, newKey: newKey ?? (() => `00000000-0000-4000-8000-${String(++sequence).padStart(12, '0')}`) })
  function update(next: OperatorContext | null) { context = next; for (const f of Array.from(listeners)) f() }
  return { session, privacy, authority, server, api, controller, login, update, context: () => context!,
    syncContext() { if (context) update({ ...context, scope: privacy.capture(), phase: server.phase as OperatorContext['phase'], eventRevision: server.eventRevision }) },
    command: (): AdminCommand => controller.read().command!,
    async open() { controller.start(); return existing ? controller.refresh(ids.job) : true },
    posts: () => server.requests.filter(r => r.method === 'POST'),
    advance(ms: number) { now += ms },
    settle() { for (const [key, r] of server.receipts) if (r.state === 'PENDING') apply(key) },
    failAgain() { server.job = { ...server.job, revision: server.job.revision + 1, state: 'PARTIAL_FAILURE', steps: server.job.steps.map(s => s.name === 'EXPORT' ? { ...s, state: 'FAILED' } : { ...s }) } },
    complete() {
      for (const step of server.job.steps) if (step.state !== 'SUCCEEDED') server.effects.set(step.name, 1)
      server.job = job('COMPLETED', server.job.revision + 1); server.phase = 'ARCHIVED'; server.eventRevision++
    },
    cleanup() { controller.dispose(); transport.dispose(); privacy.dispose(); server.requests.length = 0; server.receipts.clear(); server.bodies.clear(); server.commands.clear(); tokens.clear() },
  }
}
