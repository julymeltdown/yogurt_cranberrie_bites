import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { QueryObserver } from '@tanstack/react-query'
import { createSessionController, type SessionController } from '../model/session'
import { TransportError } from '../../../shared/api/errors'
import { authResponse, capabilities, deferred, eventId, fixture, isolatedCookiePolicy, userId } from './fixtures/data'
const controllers: SessionController[] = []
beforeEach(() => vi.stubGlobal('location', { origin: 'https://mcr.example' }))
afterEach(() => { controllers.splice(0).forEach(c => c.dispose()); vi.useRealTimers(); vi.unstubAllGlobals() })
function setup(handler: (request: Request) => Promise<Response> | Response = () => Response.json(fixture()), qualifiedRefresh = false) {
  const cookiePolicy = isolatedCookiePolicy()
  const fetch = vi.fn<typeof globalThis.fetch>(async input => {
    const request = input as Request, path = new URL(request.url).pathname
    if (path === '/api/auth/capabilities') return Response.json(capabilities())
    if (path === '/api/auth/login') return Response.json(authResponse())
    return handler(request)
  })
  const controller = createSessionController({ eventId, fetch, cookiePolicy, qualifiedRefresh, timeoutMs: 1000 })
  controllers.push(controller); controller.start()
  return { controller, fetch, cookiePolicy }
}
async function login(c: SessionController) {
  await vi.waitFor(() => expect(c.read().emailEnabled).toBe(true))
  expect(await c.login('fan@example.com', 'test-password')).toBe(true)
}
const privateRead = () => ({ path: `/api/concert/events/${eventId}/me`, method: 'GET' as const, access: 'protected' as const, errorFamily: 'core' as const, decode: (value: unknown) => value })
const unauthorized = () => Response.json({ code: 'UNAUTHENTICATED', message: 'Private raw error must not be shown', details: { reason: 'AUTHENTICATION_REQUIRED' } }, { status: 401 })
describe('session authority, privacy and stale completion fences', () => {
  it('starts capabilities once and never restores a cookie automatically', async () => {
    const { controller: c, fetch } = setup(); c.start(); c.start()
    await vi.waitFor(() => expect(c.read().emailEnabled).toBe(true))
    expect(fetch).toHaveBeenCalledTimes(1)
    expect(c.session.read()).toMatchObject({ userId: null, accessToken: null })
  })
  it('requires explicit canonical event configuration and performs no discovery', async () => {
    const fetch = vi.fn<typeof globalThis.fetch>()
    for (const invalid of [null, '', 'fixture', '00000000-0000-4000-8000-00000000020A']) {
      const c = createSessionController({ eventId: invalid, fetch, cookiePolicy: isolatedCookiePolicy() }); controllers.push(c); c.start()
      expect(c.read().readiness).toBe('unavailable'); expect(await c.login('fan@example.com', 'pass')).toBe(false)
    }
    expect(fetch).not.toHaveBeenCalled()
  })
  it('does not double-submit login and discards a late response after logout', async () => {
    const delayed = deferred<Response>(), cookiePolicy = isolatedCookiePolicy()
    const fetch = vi.fn<typeof globalThis.fetch>(async input => {
      const path = new URL((input as Request).url).pathname
      return path.endsWith('capabilities') ? Response.json(capabilities()) : path.endsWith('login') ? delayed.promise : new Response(null, { status: 204 })
    })
    const c = createSessionController({ eventId, fetch, cookiePolicy }); controllers.push(c); c.start()
    await vi.waitFor(() => expect(c.read().emailEnabled).toBe(true))
    const first = c.login('fan@example.com', 'password')
    expect(await c.login('fan@example.com', 'password')).toBe(false)
    await vi.waitFor(() => expect(fetch).toHaveBeenCalledTimes(2))
    expect(await c.logout()).toBe(true)
    delayed.resolve(Response.json(authResponse()))
    expect(await first).toBe(false); expect(c.session.read().accessToken).toBeNull()
    expect(c.read().logout).toBe('confirmed')
    expect(fetch.mock.calls.filter(([r]) => (r as Request).url.endsWith('/login'))).toHaveLength(1)
  })
  it('removes observer data, private queries, drafts/subscriptions and identity before server logout settles', async () => {
    const delayed = deferred<Response>(), { controller: c } = setup(() => delayed.promise)
    await login(c)
    let draft = 'private draft', subscriptions = 1
    c.privacy.registerCleanup(() => { draft = ''; subscriptions = 0 })
    const observer = new QueryObserver(c.queryClient, c.privacy.options({ resource: 'participant', read: () => 'private participant' }))
    const stop = observer.subscribe(() => undefined)
    await vi.waitFor(() => expect(observer.getCurrentResult().data).toBe('private participant'))
    const signal = c.session.read().signal, pending = c.logout()
    expect(signal.aborted).toBe(true); expect(draft).toBe(''); expect(subscriptions).toBe(0)
    expect(observer.getCurrentResult().data).toBeUndefined(); expect(c.queryClient.getQueryCache().getAll()).toHaveLength(0)
    expect(c.session.read().accessToken).toBeNull(); expect(c.read().logout).toBe('none')
    delayed.resolve(new Response(null, { status: 503 })); expect(await pending).toBe(false)
    expect(c.read().logout).toBe('unconfirmed'); stop(); observer.destroy()
  })
  it('makes a peer cookie writer revoke local access and ignores a late private read', async () => {
    const delayed = deferred<Response>(), { controller: c, cookiePolicy, fetch } = setup(() => delayed.promise)
    await login(c); const result = c.request(privateRead()).catch(error => error)
    await vi.waitFor(() => expect(fetch).toHaveBeenCalledTimes(3))
    cookiePolicy.invalidate(); delayed.resolve(Response.json({ private: 'late' }))
    expect(await result).toBeInstanceOf(TransportError)
    expect(c.session.read().userId).toBeNull(); expect(c.read().notice).toBe('changed')
  })
  it('clears on revoked auth without attempting production refresh or replay', async () => {
    const { controller: c, fetch } = setup(unauthorized); await login(c)
    await expect(c.request(privateRead())).rejects.toMatchObject({ status: 401 })
    expect(c.session.read().accessToken).toBeNull(); expect(fetch).toHaveBeenCalledTimes(3)
    expect(fetch.mock.calls.some(([r]) => (r as Request).url.endsWith('/refresh'))).toBe(false)
  })
  it('rejects wrong-context or malformed full bootstrap and clears private authority', async () => {
    const b = fixture(); b.participant.user_id = '00000000-0000-4000-8000-000000000999'
    const { controller: c } = setup(() => Response.json(b)); await login(c)
    const scope = c.session.read()
    await expect(c.loadBootstrap({ ...scope, userId, eventId }, new AbortController().signal)).rejects.toMatchObject({ reason: 'unavailable' })
    expect(c.session.read().accessToken).toBeNull()
  })
  it('validates full bundle and context before returning the shell projection', async () => {
    const { controller: c } = setup(); await login(c)
    const scope = c.session.read()
    expect(await c.loadBootstrap({ ...scope, userId, eventId }, new AbortController().signal)).toMatchObject({ contractVersion: '0.1.0', worldAvailable: false })
  })
  it('expires memory authority without automatic renewal', async () => {
    const short = authResponse(); short.expiresIn = 1
    const cookiePolicy = isolatedCookiePolicy(), f = vi.fn<typeof globalThis.fetch>(async input => (input as Request).url.endsWith('capabilities') ? Response.json(capabilities()) : Response.json(short))
    const next = createSessionController({ eventId, fetch: f, cookiePolicy }); controllers.push(next); next.start(); await login(next)
    await vi.waitFor(() => expect(next.read().notice).toBe('expired'), { timeout: 1500 })
    expect(next.session.read().accessToken).toBeNull(); expect(f).toHaveBeenCalledTimes(2)
  })
})
describe('explicitly qualified refresh path', () => {
  it('joins simultaneous 401s and delayed old-token failures into one cookie rotation, one read replay each', async () => {
    const rotate = deferred<Response>(), oldRead = deferred<Response>()
    let reads = 0
    const { controller: c, fetch } = setup(request => {
      if (request.url.endsWith('/refresh')) return rotate.promise
      reads++
      if (request.headers.get('Authorization') === `Bearer ${authResponse().accessToken}`) return reads === 3 ? oldRead.promise : unauthorized()
      return Response.json({ authorized: true })
    }, true)
    await login(c)
    const requests = Array.from({ length: 3 }, () => c.request(privateRead()))
    await vi.waitFor(() => expect(fetch.mock.calls.filter(([r]) => (r as Request).url.endsWith('/refresh'))).toHaveLength(1))
    rotate.resolve(Response.json({ ...authResponse(), accessToken: 'rotated-token' }))
    await requests[0]; await requests[1]
    oldRead.resolve(unauthorized()); await requests[2]
    expect(fetch.mock.calls.filter(([r]) => (r as Request).url.endsWith('/refresh'))).toHaveLength(1)
    expect(c.session.read()).toMatchObject({ accessToken: 'rotated-token', tokenVersion: 1, userId })
    expect(reads).toBe(6)
  })
  it('never refreshes or replays a mutation, even under qualified read recovery', async () => {
    const { controller: c, fetch } = setup(unauthorized, true); await login(c)
    await expect(c.request({ ...privateRead(), path: `/api/concert/events/${eventId}/me/preferences`, method: 'PATCH', body: { json: { sound_enabled: true } } })).rejects.toMatchObject({ status: 401 })
    expect(fetch).toHaveBeenCalledTimes(3); expect(c.session.read().userId).toBeNull()
  })
  it('failed refresh clears and concurrent callers cannot retry the same cookie', async () => {
    const { controller: c, fetch } = setup(unauthorized, true); await login(c)
    const result = await Promise.allSettled(Array.from({ length: 5 }, () => c.request(privateRead())))
    expect(result.every(r => r.status === 'rejected')).toBe(true)
    expect(fetch.mock.calls.filter(([r]) => (r as Request).url.endsWith('/refresh'))).toHaveLength(1)
    expect(c.session.read().accessToken).toBeNull()
  })
  it('late rotation cannot restore an identity revoked by logout', async () => {
    const delayed = deferred<Response>(), { controller: c, fetch } = setup(request => request.url.endsWith('/refresh') ? delayed.promise : new Response(null, { status: 204 }), true)
    await login(c); const result = c.refresh().catch(e => e)
    await vi.waitFor(() => expect(fetch.mock.calls.some(([r]) => (r as Request).url.endsWith('/refresh'))).toBe(true))
    await c.logout(); delayed.resolve(Response.json({ ...authResponse(), accessToken: 'late-rotation' }))
    expect(await result).toBeInstanceOf(TransportError); expect(c.session.read().accessToken).toBeNull()
  })
})
