import { createSessionController } from '../../../../features/auth/model/session'
import { authResponse, capabilities, eventId, isolatedCookiePolicy, userId } from '../../../../features/auth/__tests__/fixtures/data'
import { withAdmission } from '../../../../features/auth/__tests__/fixtures/admission'
import type { MuralAdminState, OperatorContext } from '../../../../features/admin/mural/contract'
import { stepNames, type ClosureJob } from '../../../../features/admin/closure/contract'
import { createMuralClosureEntry } from '../index'

export const ids = { event: eventId, actor: userId, snapshot: '00000000-0000-4000-8000-000000000003',
  restore: '00000000-0000-4000-8000-000000000004', job: '00000000-0000-4000-8000-000000000050' }
export const partialJob = (revision = 3): ClosureJob => ({ job_id: ids.job, event_id: ids.event, state: 'PARTIAL_FAILURE', revision,
  steps: stepNames.map((name, i) => ({ name, state: i < 5 ? 'SUCCEEDED' : i === 5 ? 'FAILED' : 'PENDING' })) })
/** Synthetic server/auth facts only; imports actual canonical Auth, controllers and transport. */
export function fixture() {
  let sequence = 1000, current: OperatorContext | null = null
  const listeners = new Set<() => void>()
  const context = { read: () => current, subscribe(listener: () => void) { listeners.add(listener); return () => { listeners.delete(listener) } } }
  const server = { requests: [] as Request[], wall: { event_id: ids.event, revision: 10, state: 'EDITABLE', snapshot_id: ids.snapshot, locks: [] } as MuralAdminState,
    job: partialJob(), mode: 'ack', operator: true, receipts: new Map<string, Record<string, unknown>>(), admissions: 0,
    intercept: null as null | ((request: Request) => Promise<Response> | null) }
  const error = (reason: string, status: number) => Response.json({ code: 'FIXTURE', message: 'Safe deterministic error', details: { reason } }, { status })
  const auth = createSessionController({ eventId, cookiePolicy: isolatedCookiePolicy(), timeoutMs: 1000, fetch: withAdmission(async input => {
    const request = input as Request, url = new URL(request.url), path = url.pathname
    server.requests.push(request.clone())
    if (path === '/api/auth/capabilities') return Response.json(capabilities())
    if (path === '/api/auth/login') return Response.json(authResponse())
    if (path === '/api/auth/logout') return new Response(null, { status: 204 })
    if (!request.headers.get('Authorization')) return error('AUTHENTICATION_REQUIRED', 401)
    const held = server.intercept?.(request); if (held) return held
    if (!server.operator) return error('PERMISSION_DENIED', 403)
    if (!path.startsWith(`/api/concert/events/${ids.event}/`)) return error('RESOURCE_UNAVAILABLE', 404)
    if (request.method === 'GET') {
      if (path.endsWith('/commands')) return server.receipts.has(url.searchParams.get('idempotency_key')!)
        ? Response.json(server.receipts.get(url.searchParams.get('idempotency_key')!)) : error('RESOURCE_UNAVAILABLE', 404)
      if (path.endsWith('/admin/mural')) return Response.json(server.wall)
      if (path.endsWith(`/closure/${ids.job}`)) return Response.json(server.job)
      return error('RESOURCE_UNAVAILABLE', 404)
    }
    if (server.mode === 'conflict') return error('VERSION_CONFLICT', 409)
    const body = await request.clone().json(), closure = path.endsWith('/closure')
    const operation = closure ? 'closeConcertEvent' : path.endsWith('/control') ? 'controlConcertMural' : path.endsWith('/locks') ? 'lockConcertMuralRegion' : 'restoreConcertMural'
    if (!closure && body.expected_revision !== server.wall.revision || closure && body.confirmation === 'RESUME_CLOSURE' && body.expected_revision !== server.job.revision) return error('VERSION_CONFLICT', 409)
    if (closure) server.job = { ...server.job, state: 'RUNNING', revision: server.job.revision + 1,
      steps: server.job.steps.map(s => s.state === 'FAILED' ? { ...s, state: 'PENDING' } : s) }
    else server.wall = { ...server.wall, revision: server.wall.revision + 1,
      state: body.action === 'PAUSE' || operation === 'restoreConcertMural' ? 'PAUSED' : body.action === 'RESUME' ? 'EDITABLE' : body.action === 'FREEZE' ? 'FROZEN' : server.wall.state,
      snapshot_id: operation === 'restoreConcertMural' ? body.snapshot_id : server.wall.snapshot_id }
    ++server.admissions
    const key = request.headers.get('Idempotency-Key')!
    server.receipts.set(key, { receipt_id: ids.snapshot, actor_id: ids.actor, event_id: ids.event, operation_id: operation, idempotency_key: key,
      state: 'SUCCEEDED', resource_id: closure ? ids.job : ids.event, resource_revision: closure ? server.job.revision : server.wall.revision,
      result_status: 200, failure_reason: null, created_at: '2026-11-04T00:00:00Z', expires_at: '2026-11-06T00:00:00Z' })
    if (server.mode === 'lost') throw new TypeError('Synthetic response lost after admission')
    return Response.json(closure ? server.job : server.wall)
  }) })
  const update = (next: OperatorContext | null) => { current = next; for (const listener of Array.from(listeners)) listener() }
  let entry: ReturnType<typeof createMuralClosureEntry> | undefined
  async function open() {
    const ready = new Promise<void>(resolve => { const stop = auth.subscribe(() => { if (auth.read().capabilities !== 'loading') { stop(); resolve() } }) })
    auth.start(); await ready
    if (!await auth.login('operator@example.com', 'test-password')) throw new Error('Synthetic login failed')
    update({ scope: auth.privacy.capture(), role: 'OPERATOR', roleRevision: 1, eventRevision: 8, phase: 'LIVE' })
    entry = createMuralClosureEntry({ auth, scope: auth.privacy.capture(), context, now: () => Date.parse('2026-11-05T00:00:00Z'),
      newKey: () => `00000000-0000-4000-8000-${String(++sequence).padStart(12, '0')}` })
    return entry
  }
  return { auth, context, server, open, update, current: () => current!, listeners: () => listeners.size,
    posts: () => server.requests.filter(r => r.method === 'POST' && !new URL(r.url).pathname.startsWith('/api/auth/')),
    dispose() { entry?.dispose(); auth.dispose() } }
}
