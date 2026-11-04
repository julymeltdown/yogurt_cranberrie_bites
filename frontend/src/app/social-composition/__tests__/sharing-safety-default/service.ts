// TEST ONLY controlled HTTP producer. Never imported by product code.
import { publicIntro, response, id } from '../fixtures'
import { admission } from '../../../../features/auth/__tests__/enrollment-consent/fixtures'

export function scenarioService(actor: string, event: string) {
  const recipient = id(99), intro = { ...publicIntro(), event_id: event, author: { ...publicIntro().author, user_id: recipient } }
  let mode = 'valid', blocked = false, revision = 1
  let last: { key: string; operation: string; body: Record<string, unknown> } | null = null
  let pendingRequest: Record<string, unknown> | null = null
  const report = { report_id: id(95), event_id: event, target_type: 'USER', target_id: recipient, state: 'OPEN', revision: 1, created_at: '2026-11-01T00:00:00Z' }
  const receipt = () => ({ receipt_id: id(96), actor_id: actor, event_id: event, operation_id: last!.operation, idempotency_key: last!.key,
    state: mode === 'resolved' ? 'SUCCEEDED' : 'PENDING', resource_id: mode === 'resolved' ? report.report_id : null, resource_revision: mode === 'resolved' ? 1 : null, failure_reason: null, result_status: mode === 'resolved' ? 200 : null,
    created_at: '2026-11-01T00:00:00Z', expires_at: '2099-11-08T00:00:00Z' })
  return {
    recipient, intro, report, mode(value: string) { mode = value }, blocked: () => blocked, last: () => last,
    async respond(r: Request): Promise<Response> {
      const path = new URL(r.url).pathname
      if (path === '/api/auth/enrollment/admission') return Response.json(admission())
      if (path.endsWith('/commands')) return Response.json(receipt())
      if (path.endsWith('/me/blocks')) return mode === 'blocks-denied' ? Response.json({}, { status: 403 }) : Response.json({ items: blocked ? [{ target_user_id: recipient, blocked_by_me: true, revision }] : [], next_cursor: null })
      if (path.endsWith('/me/reports')) return mode === 'reports-denied' ? Response.json({}, { status: 403 }) : Response.json({ items: mode === 'resolved' ? [report] : [], next_cursor: null })
      if (path.endsWith(`/people/${recipient}`)) return mode === 'target-denied' ? Response.json({}, { status: 403 }) : Response.json(intro.author)
      if (path.endsWith('/intros')) return Response.json({ items: mode === 'no-intro' ? [] : [intro], next_cursor: null })
      if (path.endsWith(`/intros/${intro.intro_id}`)) return mode === 'intro-denied' ? Response.json({}, { status: 403 }) : Response.json(intro)
      if (path.endsWith('/sharing-offer')) return Response.json({ intro_id: intro.intro_id, intro_revision: mode === 'wrong-revision' ? intro.revision + 1 : intro.revision,
        recipient_id: mode === 'wrong-recipient' ? id(98) : recipient, available_handles: [{ platform: 'INSTAGRAM', version: 3 }] })
      if (path.endsWith('/me/social-handles')) return Response.json({ items: [{ user_id: actor, platform: 'INSTAGRAM', version: 2, configured: true }, { user_id: actor, platform: 'X', version: 0, configured: false }] })
      if (path.endsWith('/me/social-requests')) return Response.json({ items: pendingRequest && new URL(r.url).searchParams.get('direction') === 'OUTGOING' ? [pendingRequest] : [], next_cursor: null })
      if (r.method !== 'GET' && /\/social-requests$|\/reports$|\/me\/blocks\//.test(path)) {
        const body = r.method === 'DELETE' ? {} : await r.json() as Record<string, unknown>
        const operation = path.endsWith('/social-requests') ? 'createConcertSocialRequest' : path.endsWith('/reports') ? 'createConcertReport' : r.method === 'PUT' ? 'blockConcertUser' : 'unblockConcertUser'
        last = { key: r.headers.get('Idempotency-Key')!, operation, body }
        if (mode === 'unknown') throw new TypeError('TEST ONLY lost response')
        if (mode === 'pending') return Response.json(receipt(), { status: 202 })
        if (operation === 'createConcertSocialRequest') {
          pendingRequest = { ...body, request_id: id(94), event_id: event, requester_id: actor, state: 'PENDING', approved_platforms: [], revision: 1, created_at: '2026-11-01T00:00:00Z', expires_at: '2099-11-08T00:00:00Z' }
          return Response.json(pendingRequest)
        }
        if (operation === 'createConcertReport') return Response.json({ ...report, target_type: body.target_type, target_id: body.target_id })
        blocked = r.method === 'PUT'; revision++
        return Response.json({ target_user_id: recipient, blocked_by_me: blocked, revision })
      }
      if (pendingRequest && path.endsWith(`/social-requests/${id(94)}`)) return Response.json(pendingRequest)
      return response(r)
    },
  }
}
