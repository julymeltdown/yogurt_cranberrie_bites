// TEST ONLY deterministic C04 producer; never imported by product entry points.
import source from '../../../../../contracts/fixtures/domain/approved-social-request.json'
export function sharingService(userId: string, eventId: string) {
  const other = '00000000-0000-4000-8000-000000000099'
  let request = { ...structuredClone(source), event_id: eventId, requester_id: other, recipient_id: userId, expires_at: '2099-11-07T10:00:00Z' }
  let version = 3, configured = true, mode = 'valid'
  let command: { key: string; operation: string; body: unknown } | null = null
  let delayed: (() => void) | null = null
  const secret = 'synthetic_private_entry_name'
  const handles = () => ({ items: [{ user_id: userId, platform: 'INSTAGRAM', version, configured }, { user_id: userId, platform: 'X', version: 0, configured: false }] })
  const reveal = () => ({ request_id: request.request_id, platform: 'INSTAGRAM', owner_id: other, handle_version: 2, handle: secret, grant_revision: request.revision, expires_at: request.expires_at })
  function resolveCommand() { request = { ...request, state: 'REJECTED', approved_platforms: [], revision: request.revision + 1 } }
  return {
    secret, handles, request: () => request,
    mode(value: string) { mode = value },
    pending() { request = { ...request, state: 'PENDING', revision: 1, approved_platforms: [] } },
    release() { delayed?.(); delayed = null },
    async respond(r: Request): Promise<Response | null> {
      const url = new URL(r.url), path = url.pathname
      if (!/social-handles|social-requests|\/commands$/.test(path)) return null
      if (mode === 'error') return Response.json({ code: 'UNAVAILABLE', message: 'TEST ONLY', details: { reason: 'RESOURCE_UNAVAILABLE' } }, { status: 503 })
      if (r.method !== 'GET') {
        const body: unknown = r.method === 'DELETE' ? null : await r.json()
        command = { key: r.headers.get('Idempotency-Key')!, operation: path.includes('/social-handles/') ? r.method === 'DELETE' ? 'removeConcertHandle' : 'setConcertHandle' : 'decideConcertSocialRequest', body }
        if (mode === 'uncertain') throw new TypeError('TEST ONLY lost response')
        if (path.includes('/social-handles/')) {
          version++; configured = r.method !== 'DELETE'
          return Response.json(handles().items[0])
        }
        resolveCommand(); return Response.json(request)
      }
      if (path.endsWith('/commands')) {
        if (!command) throw new Error('No TEST command')
        if (mode === 'resolved') resolveCommand()
        return Response.json({ receipt_id: '00000000-0000-4000-8000-000000000100', event_id: eventId, actor_id: mode === 'foreign-receipt' ? other : userId,
          operation_id: command.operation, idempotency_key: command.key, state: mode === 'resolved' || mode === 'foreign-receipt' ? 'SUCCEEDED' : 'PENDING',
          resource_id: mode === 'resolved' || mode === 'foreign-receipt' ? request.request_id : null,
          resource_revision: mode === 'resolved' || mode === 'foreign-receipt' ? request.revision : null, failure_reason: null,
          created_at: '2026-11-01T09:00:00Z', expires_at: '2099-11-08T09:00:00Z', result_status: mode === 'resolved' || mode === 'foreign-receipt' ? 200 : null })
      }
      if (path.endsWith('/social-handles')) return Response.json(handles())
      if (path.endsWith('/me/social-requests')) return Response.json({ items: mode === 'empty' || url.searchParams.get('direction') === 'OUTGOING' ? [] : [request], next_cursor: null })
      if (path.includes('/reveal')) {
        if (mode === 'denied-reveal') return Response.json({}, { status: 401 })
        if (mode === 'late-reveal') return new Promise(resolve => { delayed = () => resolve(Response.json(reveal())) })
        return Response.json(reveal())
      }
      return Response.json(request)
    },
  }
}
