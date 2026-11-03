import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { createSessionController } from '../model/session'
import type { HttpRequest } from '../../../shared/api/http'
import { authResponse, deferred, eventId, userId, isolatedCookiePolicy } from './fixtures/data'
const requestId = '00000000-0000-4000-8000-000000000029'
const cleanups: (() => void)[] = []
beforeEach(() => vi.stubGlobal('location', { origin: 'https://mcr.example' }))
afterEach(() => { cleanups.splice(0).forEach(fn => fn()); vi.unstubAllGlobals() })
function setup(handler: (request: Request) => Response | Promise<Response>) {
  const fetch = vi.fn<typeof globalThis.fetch>(async input => handler(input as Request))
  const auth = createSessionController({ eventId, fetch, qualifiedRefresh: true, cookiePolicy: isolatedCookiePolicy() })
  cleanups.push(auth.dispose); auth.session.replace(0, { userId, eventId, accessToken: 'sensitive-fixture-token' })
  return { auth, fetch }
}
const read = (): Omit<HttpRequest<unknown>, 'refresh'> => ({ method: 'GET', access: 'protected', errorFamily: 'domain', domainOperation: 'revealConcertSocialHandle', path: `/api/concert/events/${eventId}/social-requests/${requestId}/reveal`, query: { platform: 'INSTAGRAM' }, decode: value => value })
it.each(['requestSensitive', 'request'] as const)('%s never refreshes/replays a protected reveal after 401', async method => {
  const { auth, fetch } = setup(request => { expect(request.cache).toBe('no-store'); expect(new URL(request.url).pathname.endsWith('/reveal')).toBe(true); return Response.json({}, { status: 401 }) })
  await expect(auth[method](read())).rejects.toBeDefined()
  expect(fetch).toHaveBeenCalledTimes(1); expect(auth.session.read().userId).toBeNull()
})
it.each([
  { method: 'POST' }, { access: 'public' }, { errorFamily: 'core' }, { domainOperation: 'listConcertFeed' },
  { refresh: 'once' }, { domainOperation: undefined }, { path: `/api/concert/events/${eventId}/feed` }, { query: { platform: 'INVALID' } },
])('rejects non-sensitive operation/method/access or injected refresh: %j', async patch => {
  const { auth, fetch } = setup(() => Response.json({}))
  const input = { ...read(), ...patch } as Omit<HttpRequest<unknown>, 'refresh'>
  await expect(auth.requestSensitive(input)).rejects.toMatchObject({ kind: 'invalid-request' }); expect(fetch).not.toHaveBeenCalled()
})
it.each(['identity', 'policy'] as const)('rejects late sensitive read after %s invalidation', async kind => {
  const late = deferred<Response>(), { auth, fetch } = setup(() => late.promise)
  const flight = auth.requestSensitive(read()); await vi.waitFor(() => expect(fetch).toHaveBeenCalledTimes(1))
  if (kind === 'identity') auth.session.replace(auth.session.read().generation, null)
  else auth.privacy.changePolicy(auth.privacy.capture(), 'consent-revoked')
  late.resolve(Response.json({ handle: 'fixture-only' })); await expect(flight).rejects.toBeDefined()
  expect(fetch).toHaveBeenCalledTimes(1)
})
it('ordinary eligible GET still refreshes once and retries with the same canonical identity', async () => {
  let feeds = 0, refreshes = 0
  const { auth, fetch } = setup(request => {
    if (request.url.endsWith('/api/auth/refresh')) { refreshes++; return Response.json(authResponse()) }
    feeds++; return feeds === 1 ? Response.json({}, { status: 401 }) : Response.json({ ok: true })
  })
  const result = await auth.request({ method: 'GET', access: 'protected', errorFamily: 'domain', domainOperation: 'listConcertFeed', path: `/api/concert/events/${eventId}/feed`, decode: value => value })
  expect(result.data).toEqual({ ok: true }); expect(feeds).toBe(2); expect(refreshes).toBe(1); expect(fetch).toHaveBeenCalledTimes(3)
})
