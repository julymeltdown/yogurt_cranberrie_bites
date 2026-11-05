import { waitFor } from '@testing-library/react'
import { createSessionController } from '../../../auth/model/session'
import { authResponse, capabilities, eventId, userId, isolatedCookiePolicy } from '../../../auth/__tests__/fixtures/data'
import { admission } from '../../../auth/__tests__/enrollment-consent/fixtures'
import { reportAuthorization } from '../../../../app/safety-entry/authorization'
import { response, post, id } from '../../../../app/social-composition/__tests__/fixtures'
import { createSafetyController } from '../../controller'

/** Canonical Auth and actual report target authorization, controlled HTTP only. */
export async function reportFixture(mode: 'ready' | 'pending' | 'unavailable' = 'ready') {
  const requests: Request[] = []
  const key = id(601)
  const receipt = () => ({ receipt_id: id(602), event_id: eventId, actor_id: userId,
    operation_id: 'createConcertReport', idempotency_key: key, state: 'PENDING',
    resource_id: null, resource_revision: null, failure_reason: null, result_status: null,
    created_at: new Date(Date.now() - 1000).toISOString(), expires_at: new Date(Date.now() + 3600000).toISOString() })
  const bind = (value: unknown): unknown => {
    if (Array.isArray(value)) return value.map(bind)
    if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([k, v]) =>
      [k, k === 'event_id' ? eventId : ['user_id', 'owner_id', 'actor_id'].includes(k) ? userId : bind(v)]))
    return value
  }
  const auth = createSessionController({ eventId, cookiePolicy: isolatedCookiePolicy(), fetch: async input => {
    const r = input as Request, path = new URL(r.url).pathname
    requests.push(r.clone())
    if (path === '/api/auth/capabilities') return Response.json(capabilities())
    if (path === '/api/auth/login') return Response.json(authResponse())
    if (path === '/api/auth/enrollment/admission') return Response.json(admission())
    if (path.endsWith('/commands')) return Response.json(receipt())
    if (path.endsWith('/me/blocks') || path.endsWith('/me/reports')) {
      if (mode === 'unavailable') return Response.json({ code: 'DEPENDENCY_UNAVAILABLE', message: 'PRIVATE_HTTP_DETAIL', details: { reason: 'DEPENDENCY_UNAVAILABLE' } }, { status: 503 })
      return Response.json({ items: [], next_cursor: null })
    }
    if (path.endsWith('/reports') && r.method === 'POST') {
      if (mode === 'pending') return Response.json(receipt(), { status: 202 })
      return Response.json({ report_id: id(603), event_id: eventId, target_type: 'POST', target_id: post.post_id,
        state: 'OPEN', revision: 1, created_at: new Date().toISOString() })
    }
    const result = response(r)
    return Response.json(bind(await result.json()), { status: result.status })
  } })
  auth.start()
  await waitFor(() => { if (auth.read().capabilities === 'loading') throw new Error('Auth capabilities pending') })
  if (!await auth.login('fixture@example.test', 'fixture-password') || !await auth.checkAdmission()) throw new Error('Canonical admission required')
  const controller = createSafetyController({ auth, reportsAvailable: true, blocksAvailable: true,
    authorization: reportAuthorization(auth), newKey: () => key })
  return { auth, controller, requests, search: `?targetType=POST&targetId=${post.post_id}`,
    dispose() { controller.dispose(); auth.dispose() } }
}
