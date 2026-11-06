import { createSessionController } from '../../../features/auth/model/session'
import { isolatedCookiePolicy, capabilities, authResponse } from '../../../features/auth/__tests__/fixtures/data'
import { autoAdmit, withAdmission } from '../../../features/auth/__tests__/fixtures/admission'
import { eventId, rooms, userId } from './data'

/** The pinned data harness predates canonical admission; this adapter adds only that boundary. */
export function authHarness(handler?: (request: Request) => Promise<Response> | Response) {
  const requests: Request[] = []
  const auth = createSessionController({ eventId, cookiePolicy: isolatedCookiePolicy(), fetch: withAdmission(async input => {
    const request = input as Request; requests.push(request)
    if (handler) return handler(request)
    const path = new URL(request.url).pathname
    if (path === '/api/auth/capabilities') return Response.json(capabilities())
    if (path === '/api/auth/login') return Response.json(authResponse())
    if (path === '/api/auth/logout') return new Response(null, { status: 204 })
    if (path.endsWith('/rooms')) return Response.json(rooms())
    throw new Error('Unexpected test request')
  }) })
  auth.session.replace(0, { userId, eventId, accessToken: 'memory-only-test-token' }); autoAdmit(auth)
  return { auth, requests }
}
