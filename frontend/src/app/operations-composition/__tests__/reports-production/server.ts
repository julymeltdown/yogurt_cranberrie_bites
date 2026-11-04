// Controlled public-contract responses only; no real role issuer or protected evidence producer.
import { createControlledServer, wire } from '../server.ts'
import type { Report } from '../../../../features/admin/reports/model/contract'
export { wire }
export const id = (n: number) => `00000000-0000-4000-8000-${String(n).padStart(12, '0')}`
export function createReportsServer() {
  const base = createControlledServer()
  const state = {
    report: { report_id: id(301), event_id: wire.event, target_type: 'POST', target_id: id(401),
      state: 'OPEN', revision: 1, created_at: '2026-11-03T00:00:00Z' } as Report,
    list: null as unknown, detail: null as unknown, result: null as unknown,
    mode: 'success' as 'success' | 'pending' | 'lost' | 'conflict' | 'denied' | 'unavailable',
    status: 'PENDING' as 'PENDING' | 'SUCCEEDED' | 'FAILED', statusHttp: 200, key: '',
    intercept: null as null | ((request: Request) => Promise<Response> | null),
  }
  const error = (status: number, reason: string) => Response.json({ code: `CONCERT_${status}`,
    message: 'PRIVATE_BACKEND_DIAGNOSTIC', details: { reason } }, { status })
  const receipt = () => ({ receipt_id: id(801), actor_id: wire.actor, event_id: wire.event,
    operation_id: 'decideConcertReport', idempotency_key: state.key, state: state.status,
    resource_id: state.status === 'SUCCEEDED' ? state.report.report_id : null,
    resource_revision: state.status === 'SUCCEEDED' ? state.report.revision : null,
    result_status: state.status === 'SUCCEEDED' ? 200 : null,
    failure_reason: state.status === 'FAILED' ? 'DEPENDENCY_UNAVAILABLE' : null,
    created_at: new Date().toISOString(), expires_at: new Date(Date.now() + 86_400_000).toISOString() })
  base.state.intercept = request => {
    const custom = state.intercept?.(request); if (custom) return custom
    const path = new URL(request.url).pathname, prefix = `/api/concert/events/${wire.event}/admin/reports`
    if (path === prefix && request.method === 'GET') return Promise.resolve(Response.json(state.list ?? { items: [state.report], next_cursor: null }))
    if (path === `${prefix}/${state.report.report_id}` && request.method === 'GET') {
      return Promise.resolve(Response.json(state.detail ?? { report: state.report, reporter_id: id(999),
        reason: 'PRIVACY', text: 'Private test statement', evidence_media_ids: [id(501)] }))
    }
    if (path === `${prefix}/${state.report.report_id}/decision` && request.method === 'POST') return (async () => {
      state.key = request.headers.get('Idempotency-Key') ?? ''
      const input = await request.clone().json() as { expected_revision: number; decision: 'RESOLVED' | 'DISMISSED' }
      if (state.mode === 'conflict') return error(409, 'VERSION_CONFLICT')
      if (state.mode === 'denied') return error(403, 'PERMISSION_DENIED')
      if (state.mode === 'unavailable') return error(503, 'DEPENDENCY_UNAVAILABLE')
      if (state.mode === 'lost') throw new TypeError('TEST_ONLY admitted command with lost response')
      if (state.mode === 'pending') { state.status = 'PENDING'; return Response.json(receipt(), { status: 202 }) }
      state.report = { ...state.report, revision: input.expected_revision + 1, state: input.decision }
      return Response.json(state.result ?? state.report)
    })()
    if (path.endsWith('/commands') && new URL(request.url).searchParams.get('operation_id') === 'decideConcertReport') {
      return Promise.resolve(state.statusHttp === 200 ? Response.json(receipt())
        : error(state.statusHttp, state.statusHttp === 404 ? 'RESOURCE_UNAVAILABLE' : state.statusHttp === 429 ? 'RATE_LIMITED' : 'DEPENDENCY_UNAVAILABLE'))
    }
    return null
  }
  return { ...base, reports: state, error, receipt }
}
