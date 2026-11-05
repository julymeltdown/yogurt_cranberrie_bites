// TEST ONLY controlled producer; never imported by product code.
// Shape provenance: sharing-safety-default/service.ts, social-composition fixtures,
// approved-social-request.json, archive fixtures and Auth account-deletion fixtures at2b3819fb.
import { scenarioService } from '../../../src/app/social-composition/__tests__/sharing-safety-default/service'
import { page } from '../../../src/features/archive/__tests__/fixtures/data'
import { id } from '../../../src/app/social-composition/__tests__/fixtures'
import { servicePolicyFixture } from '../../../src/app/pre-world-policy/__tests__/fixtures'
import { accepted, saga } from '../../../src/features/auth/__tests__/account-deletion/fixtures'
export function privateService(actor: string, event: string) {
  const previous = scenarioService(actor, event), recipient = id(99)
  let mode = 'loaded', age = 'UNKNOWN', configured = true, version = 2, blocked = true, read = false
  let request = { request_id: id(94), event_id: event, requester_id: recipient, recipient_id: actor,
    intro_id: previous.intro.intro_id, intro_revision: previous.intro.revision,
    consents: [{ platform: 'INSTAGRAM', requester_handle_version: 3, recipient_handle_version: 2, requester_consents: true }],
    approved_platforms: [] as string[], state: 'PENDING', revision: 1,
    created_at: '2026-11-01T00:00:00Z', expires_at: '2099-11-08T00:00:00Z' }
  const notification = () => ({ notification_id: id(90), event_id: event, kind: 'SOCIAL', target: { target_type: 'SOCIAL_REQUEST', target_id: request.request_id }, created_at: '2026-11-01T00:00:00Z', read_at: read ? new Date().toISOString() : null, revision: read ? 2 : 1 })
  const failure = (status = 503) => Response.json({ code: 'RESOURCE_UNAVAILABLE', message: 'PRIVATE_PRODUCER_DETAIL_MUST_NOT_RENDER', details: { reason: 'RESOURCE_UNAVAILABLE' } }, { status })
  return {
    recipient,
    mode(value: string) { mode = value; if (value.startsWith('request-')) { const state = value.slice(8); request = { ...request, state, approved_platforms: state === 'APPROVED' ? ['INSTAGRAM'] : [], revision: request.revision + 1, created_at: '2026-10-01T00:00:00Z', expires_at: state === 'EXPIRED' ? '2026-11-01T00:00:00Z' : '2099-11-08T00:00:00Z' } } },
    age(value: string) { age = value },
    async respond(r: Request): Promise<Response> {
      const path = new URL(r.url).pathname
      if (path.endsWith('/service-policy')) return Response.json(servicePolicyFixture())
      if (path.endsWith('/archive')) {
        if (mode === 'error') return failure()
        const data = page(new URL(r.url).searchParams.has('cursor'), event)
        if (mode === 'empty') { data.feed.items = []; data.feed.next_cursor = null }
        return Response.json(data)
      }
      if (path === '/api/auth/account' && r.method === 'DELETE') return Response.json(accepted(saga()), { status: 202 })
      if (path === '/api/auth/account/deletion') return Response.json(saga())
      if (r.method === 'GET' && mode === 'error' && /social-handles|social-requests|age-status|notifications|me\/blocks/.test(path)) return failure()
      if (path.endsWith('/me/age-status')) return Response.json({ user_id: actor, event_id: event, status: age,
        verified_at: age === 'VERIFIED' || age === 'EXPIRED' ? '2026-10-01T00:00:00Z' : null,
        expires_at: age === 'VERIFIED' ? '2099-11-01T00:00:00Z' : age === 'EXPIRED' ? '2026-11-01T00:00:00Z' : null, verification_available: false })
      if (path.endsWith('/me/social-handles')) return Response.json({ items: [{ user_id: actor, platform: 'INSTAGRAM', version, configured }, { user_id: actor, platform: 'X', version: 0, configured: false }] })
      if (path.endsWith('/me/social-handles/INSTAGRAM') && ['PUT','DELETE'].includes(r.method)) { version++; configured = r.method === 'PUT'; return Response.json({ user_id: actor, platform: 'INSTAGRAM', version, configured }) }
      if (path.endsWith('/me/social-requests')) return Response.json({ items: mode === 'empty' || new URL(r.url).searchParams.get('direction') === 'OUTGOING' ? [] : [request], next_cursor: null })
      if (path.endsWith(`/social-requests/${request.request_id}`) && r.method === 'GET') return Response.json(request)
      if (path.endsWith(`/social-requests/${request.request_id}/decision`)) {
        const body = await r.json() as { decision: string; approved_platforms: string[] }
        request = { ...request, state: body.decision, approved_platforms: body.approved_platforms, revision: request.revision + 1 }
        return Response.json(request)
      }
      if (path.endsWith(`/social-requests/${request.request_id}/reveal`)) return Response.json({ request_id: request.request_id, platform: 'INSTAGRAM', owner_id: recipient, handle_version: 3, handle: 'controlled_fixture', grant_revision: request.revision, expires_at: new Date(Date.now() + 5000).toISOString() })
      if (path.endsWith(`/social-requests/${request.request_id}/revoke`)) { request = { ...request, state: 'REVOKED', revision: request.revision + 1 }; return Response.json(request) }
      if (path.endsWith('/me/blocks')) return Response.json({ items: mode === 'empty' || !blocked ? [] : [{ target_user_id: recipient, blocked_by_me: true, revision: 1 }], next_cursor: null })
      if (path.endsWith(`/me/blocks/${recipient}`) && r.method === 'DELETE') { blocked = false; return Response.json({ target_user_id: recipient, blocked_by_me: false, revision: 2 }) }
      if (path.endsWith('/me/notifications/unread-count')) return Response.json({ actor_id: actor, event_id: event, unread_total: read || mode === 'empty' ? 0 : 1, as_of: new Date().toISOString() })
      if (path.endsWith('/me/notifications/read-all')) { read = true; return Response.json({ read_count: 1, read_at: new Date().toISOString() }) }
      if (path.endsWith(`/me/notifications/${id(90)}/read`)) { read = true; return Response.json(notification()) }
      if (path.endsWith('/me/notifications')) return Response.json({ items: mode === 'empty' || read && new URL(r.url).searchParams.get('unread_only') === 'true' ? [] : [notification()], next_cursor: null })
      return previous.respond(r)
    },
  }
}
