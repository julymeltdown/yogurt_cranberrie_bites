// TEST ONLY: controlled public HTTP. The actual main/Auth/router/default ports run unchanged.
// Reuses the canonical Reports fixture's headless HTTP protocol, not its UI or Auth instance.
import { createReportsServer, wire, id } from '../reports-production/server.ts'
export { wire }
export function createPolicyServer() {
  const base = createReportsServer()
  const service = { mode: 'success', policyRevision: 4, eventRevision: 19,
    minimum: '1.0.0', kind: 'NORMAL', receipt: null, pending: null, statusHttp: 200 }
  const sync = () => {
    base.state.settings.revision = service.eventRevision
    base.state.settings.event.revision = service.eventRevision
  }
  sync()
  const now = () => new Date().toISOString()
  const apply = input => {
    service.policyRevision++; service.eventRevision += 3
    service.kind = input.change.action === 'WITHDRAW' ? 'UNAVAILABLE' : input.change.mode
    service.minimum = input.change.action === 'WITHDRAW' ? null : input.change.minimum_client_version
    sync()
  }
  const receipt = (key, state, failure_reason = null) => ({ receipt_id: id(880), actor_id: wire.actor, event_id: wire.event,
    operation_id: 'setConcertServicePolicy', idempotency_key: key, state,
    resource_id: state === 'SUCCEEDED' ? wire.event : null,
    resource_revision: state === 'SUCCEEDED' ? service.eventRevision : null,
    result_status: state === 'SUCCEEDED' ? 200 : null, failure_reason,
    created_at: now(), expires_at: new Date(Date.now() + 86_400_000).toISOString() })
  base.reports.intercept = request => {
    const url = new URL(request.url), path = url.pathname
    if (request.method === 'GET' && path.endsWith('/service-policy')) return Promise.resolve(Response.json({
      event_revision: service.eventRevision, policy_revision: service.policyRevision, kind: service.kind,
      minimum_client_version: service.minimum, server_time: now(), valid_until: new Date(Date.now() + 30_000).toISOString(),
    }))
    if (request.method === 'PUT' && path.endsWith('/admin/service-policy')) return (async () => {
      const input = await request.clone().json(), key = request.headers.get('Idempotency-Key')
      if (service.mode === 'conflict' || input.expected_policy_revision !== service.policyRevision) {
        service.policyRevision++; service.eventRevision++; sync()
        service.receipt = receipt(key, 'FAILED', 'VERSION_CONFLICT')
        return Response.json(service.receipt)
      }
      if (service.mode === 'pending') {
        service.pending = input; service.receipt = receipt(key, 'PENDING')
        return Response.json(service.receipt, { status: 202 })
      }
      apply(input); service.receipt = receipt(key, 'SUCCEEDED')
      if (service.mode === 'lost') throw new TypeError('TEST_ONLY lost response after synthetic policy commit')
      return Response.json(service.receipt)
    })()
    if (request.method === 'GET' && path.endsWith('/commands') && url.searchParams.get('operation_id') === 'setConcertServicePolicy') {
      return Promise.resolve(service.statusHttp === 200 && service.receipt?.idempotency_key === url.searchParams.get('idempotency_key')
        ? Response.json(service.receipt) : base.error(service.statusHttp === 200 ? 404 : service.statusHttp, 'RESOURCE_UNAVAILABLE'))
    }
    if (request.method === 'GET' && path.endsWith('/admin/overview')) return Promise.resolve(Response.json({
      event_id: wire.event, revision: service.eventRevision, health: 'AVAILABLE', open_reports: 1, pending_jobs: 0,
    }))
    if (request.method === 'GET' && path.endsWith('/admin/content')) return Promise.resolve(Response.json({ items: [], next_cursor: null }))
    return null
  }
  return { ...base, service,
    writes: () => base.requests.filter(request => request.method === 'PUT' && new URL(request.url).pathname.endsWith('/admin/service-policy')),
    settle() {
      if (!service.pending || !service.receipt) throw Error('No pending synthetic policy command')
      apply(service.pending); service.receipt = { ...service.receipt, state: 'SUCCEEDED', resource_id: wire.event,
        resource_revision: service.eventRevision, result_status: 200 }; service.pending = null
    },
  }
}
