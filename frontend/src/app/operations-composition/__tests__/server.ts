// TEST ONLY: controlled HTTP data; no real assignment, venue, evidence or producer.
import wire from './wire.json' with { type: 'json' }
export { wire }
export function createControlledServer() {
  const requests: Request[] = [], receipts = new Map<string, unknown>()
  const state = { role: 'OPERATOR' as 'OPERATOR' | 'MODERATOR' | null, roleRevision: 7, decision: crypto.randomUUID(),
    settings: structuredClone(wire.settings), settingsOverride: null as unknown, settingsStatus: 200,
    lost: false, wall: { event_id: wire.event, revision: 10, state: 'EDITABLE', snapshot_id: wire.snapshot, locks: [] },
    intercept: null as null | ((request: Request) => Promise<Response> | null) }
  const error = (status: number, reason: string) => Response.json({ code: 'TEST_ONLY', message: 'PRIVATE diagnostic must never render', details: { reason } }, { status })
  async function handle(request: Request): Promise<Response> {
    requests.push(request.clone()); const path = new URL(request.url).pathname
    if (path === '/api/auth/capabilities') return Response.json(wire.capabilities)
    if (path === '/api/auth/login' || path === '/api/auth/refresh') return Response.json(wire.auth)
    if (path === '/api/auth/logout') return new Response(null, { status: 204 })
    if (!request.headers.get('Authorization')) return error(401, 'AUTHENTICATION_REQUIRED')
    if (path === '/api/auth/enrollment/admission') return Response.json({ decision: 'ADMITTED',
      policyRevision: '11111111-1111-4111-8111-111111111111', acceptedRevision: '11111111-1111-4111-8111-111111111111', serverTime: new Date().toISOString() })
    const intercepted = state.intercept?.(request); if (intercepted) return intercepted
    if (!path.startsWith(`/api/concert/events/${wire.event}/`)) return error(404, 'RESOURCE_UNAVAILABLE')
    if (path.endsWith('/me/operations-access')) {
      const checked = new Date().toISOString()
      return Response.json({ server_time: checked, access: state.role ? { actor_id: wire.actor, event_id: wire.event,
        role: state.role, role_revision: state.roleRevision, decision_id: state.decision,
        checked_at: checked, expires_at: new Date(Date.parse(checked) + 60_000).toISOString() } : null })
    }
    if (request.method === 'GET') {
      if (path.endsWith('/admin/settings')) return state.settingsStatus === 200 ? Response.json(state.settingsOverride ?? state.settings) : error(state.settingsStatus, state.settingsStatus === 403 ? 'PERMISSION_DENIED' : 'DEPENDENCY_UNAVAILABLE')
      if (path.endsWith('/admin/overview')) return Response.json({ event_id: wire.event, revision: 8, health: 'AVAILABLE', open_reports: 0, pending_jobs: 1 })
      if (path.endsWith('/admin/notices') || path.endsWith('/admin/places') || path.endsWith('/admin/audit')) return Response.json({ items: [], next_cursor: null })
      if (path.endsWith('/admin/mural')) return Response.json(state.wall)
      if (path.endsWith(`/closure/${wire.job}`)) return Response.json({ job_id: wire.job, event_id: wire.event, state: 'PARTIAL_FAILURE', revision: 3,
        steps: ['READ_ONLY', 'FREEZE_MURAL', 'SNAPSHOT', 'EXPIRE_GRANTS', 'EXPIRE_BEACONS', 'EXPORT', 'CLEANUP'].map((name, i) => ({ name, state: i < 5 ? 'SUCCEEDED' : i === 5 ? 'FAILED' : 'PENDING' })) })
      if (path.endsWith('/commands')) return receipts.has(new URL(request.url).searchParams.get('idempotency_key')!)
        ? Response.json(receipts.get(new URL(request.url).searchParams.get('idempotency_key')!)) : error(404, 'RESOURCE_UNAVAILABLE')
    }
    if (request.method === 'POST' && path.endsWith('/admin/mural/control')) {
      const body = await request.json() as { action: string; expected_revision: number }
      if (body.expected_revision !== state.wall.revision) return error(409, 'VERSION_CONFLICT')
      state.wall = { ...state.wall, revision: state.wall.revision + 1, state: body.action === 'PAUSE' ? 'PAUSED' : 'EDITABLE' }
      const key = request.headers.get('Idempotency-Key')!
      receipts.set(key, { receipt_id: wire.snapshot, actor_id: wire.actor, event_id: wire.event, operation_id: 'controlConcertMural',
        idempotency_key: key, state: 'SUCCEEDED', resource_id: wire.event, resource_revision: state.wall.revision, result_status: 200,
        failure_reason: null, created_at: new Date().toISOString(), expires_at: new Date(Date.now() + 86_400_000).toISOString() })
      if (state.lost) throw new TypeError('TEST_ONLY lost response after durable admission')
      return Response.json(state.wall)
    }
    return error(404, 'RESOURCE_UNAVAILABLE')
  }
  return { state, requests, receipts, handle, replaceDecision() { state.decision = crypto.randomUUID() } }
}
