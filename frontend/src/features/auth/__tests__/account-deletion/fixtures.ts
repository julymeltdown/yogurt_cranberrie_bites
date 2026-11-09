import { createSessionController } from '../../model/session'
import { authResponse, capabilities, eventId, isolatedCookiePolicy, userId } from '../fixtures/data'
import { withAdmission } from '../fixtures/admission'
import { deletionSaga } from '../../../account/__tests__/deletion-fixtures'
export { deferred } from '../../../../test-support/fixtures'
export { eventId, userId }
export const otherId = '00000000-0000-4000-8000-000000000999'
export function saga(complete = false) {
  const s = deletionSaga(complete), now = Date.now()
  s.userId = userId; s.createdAt = new Date(now - 60000).toISOString(); s.updatedAt = new Date(now - 1000).toISOString()
  s.retentionUntil = new Date(now + 86400000).toISOString(); s.completedAt = complete ? new Date(now - 2000).toISOString() : null
  for (const step of s.steps) { step.nextAttemptAt = new Date(now + 60000).toISOString(); step.completedAt = complete ? new Date(now - 3000).toISOString() : null }
  return s
}
export function accepted(s = saga()) { return { userId, deleted: true, sagaId: s.id, saga: s } }
export function fixtureAuth(handle: (request: Request) => Response | Promise<Response> = () => Response.json(accepted()), expiresIn = 300) {
  const requests: Request[] = []
  const auth = createSessionController({ eventId, cookiePolicy: isolatedCookiePolicy(), qualifiedRefresh: true, timeoutMs: 1000,
    fetch: withAdmission(async input => {
      const r = input as Request; requests.push(r.clone()); const path = new URL(r.url).pathname
      if (path.endsWith('/capabilities')) return Response.json(capabilities())
      if (path.endsWith('/login')) return Response.json({ ...authResponse(), expiresIn })
      if (path.endsWith('/logout')) return new Response(null, { status: 204 })
      return handle(r)
    }),
  })
  return { auth, requests, async login() {
    auth.start()
    if (auth.read().capabilities === 'loading') await new Promise<void>(resolve => { const stop = auth.subscribe(() => { if (auth.read().capabilities !== 'loading') { stop(); resolve() } }) })
    if (!await auth.login('synthetic@example.org', 'fixture-password')) throw new Error('Synthetic login failed')
  }, dispose() { auth.dispose() } }
}
