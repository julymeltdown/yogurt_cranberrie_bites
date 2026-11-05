import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createSessionController, type SessionController } from '../../model/session'
import type { CookiePolicy } from '../../model/cookie-policy'
import { authResponse, deferred, eventId, isolatedCookiePolicy, userId } from '../fixtures/data'

const controllers: SessionController[] = []
beforeEach(() => { vi.stubGlobal('location', { origin: 'https://mcr.example' }) })
afterEach(() => { controllers.splice(0).forEach(controller => controller.dispose()); vi.useRealTimers(); vi.unstubAllGlobals() })
function setup(options: { refresh?: (request: Request) => Response | Promise<Response>; admission?: string;
  admissionResponse?: () => Response | Promise<Response>; cookiePolicy?: CookiePolicy; event?: string | null } = {}) {
  const requests: Request[] = []
  const fetch = vi.fn<typeof globalThis.fetch>(async input => {
    const request = input as Request; requests.push(request)
    if (request.url.endsWith('/refresh')) return options.refresh?.(request) ?? Response.json(authResponse())
    if (request.url.endsWith('/enrollment/admission')) return options.admissionResponse?.() ?? Response.json({ decision: options.admission ?? 'ADMITTED',
      policyRevision: '00000000-0000-4000-8000-000000000123', acceptedRevision: options.admission ? null : '00000000-0000-4000-8000-000000000123', serverTime: '2026-11-05T00:00:00Z' })
    if (request.url.endsWith('/logout')) return new Response(null, { status: 204 })
    throw new Error('Unexpected OAuth fixture request')
  })
  const cookiePolicy = options.cookiePolicy ?? isolatedCookiePolicy()
  const controller = createSessionController({ eventId: options.event === undefined ? eventId : options.event, fetch, cookiePolicy, timeoutMs: 1000 })
  controllers.push(controller)
  return { controller, requests, fetch, cookiePolicy }
}
describe('anonymous canonical OAuth handoff', () => {
  it('adopts only decoded refresh credentials, runs admission, and clears the old privacy scope', async () => {
    const { controller: auth, requests } = setup(), before = auth.session.read(), cleanup = vi.fn()
    auth.privacy.registerCleanup(cleanup)
    expect(await auth.completeOAuthHandoff()).toBe('ready')
    expect(auth.session.read()).toMatchObject({ userId, eventId, accessToken: authResponse().accessToken, generation: before.generation + 1 })
    expect(auth.read().admission).toBe('ADMITTED'); expect(auth.read().readiness).toBe('ready')
    expect(before.signal.aborted).toBe(true); expect(cleanup).toHaveBeenCalledOnce()
    expect(requests.map(request => new URL(request.url).pathname)).toEqual(['/api/auth/refresh', '/api/auth/enrollment/admission'])
    expect(requests[0]!.method).toBe('POST'); expect(requests[0]!.credentials).toBe('same-origin')
    expect(requests[0]!.headers.get('authorization')).toBeNull()
    expect(requests[0]!.headers.get('x-mcr-intent')).toBe('browser')
    expect(requests[1]!.headers.get('authorization')).toBe(`Bearer ${authResponse().accessToken}`)
  })
  it('joins concurrent callers and refuses a later marker instead of rotating the cookie again', async () => {
    const delayed = deferred<Response>(), { controller: auth, requests } = setup({ refresh: () => delayed.promise })
    const first = auth.completeOAuthHandoff(), second = auth.completeOAuthHandoff()
    expect(first).toBe(second)
    await vi.waitFor(() => expect(requests).toHaveLength(1))
    delayed.resolve(Response.json(authResponse())); expect(await first).toBe('ready')
    expect(await auth.completeOAuthHandoff()).toBe('unavailable')
    expect(requests.filter(request => request.url.endsWith('/refresh'))).toHaveLength(1)
  })
  it.each([401, 403, 409, 429, 503])('does not adopt or retry after refresh status %i', async status => {
    const { controller: auth, requests } = setup({ refresh: () => Response.json({ code: 'UNAVAILABLE', message: 'Unavailable' }, { status }) })
    expect(await auth.completeOAuthHandoff()).toBe('unavailable')
    expect(await auth.completeOAuthHandoff()).toBe('unavailable')
    expect(auth.session.read().userId).toBeNull(); expect(requests).toHaveLength(1)
  })
  it.each(['CONSENT_REQUIRED', 'POLICY_UNAVAILABLE', 'ACCOUNT_INACTIVE'])('keeps canonical admission %s authoritative', async admission => {
    const { controller: auth, requests } = setup({ admission })
    expect(await auth.completeOAuthHandoff()).toBe('restricted')
    expect(auth.read().admission).toBe(admission); expect(auth.read().readiness).not.toBe('ready')
    expect(auth.session.read().userId).toBe(userId)
    await expect(auth.request({ path: '/api/concert/events/x', method: 'GET', access: 'protected', errorFamily: 'core', decode: value => value })).rejects.toBeDefined()
    expect(requests.every(request => /\/(refresh|enrollment\/admission)$/.test(request.url))).toBe(true)
  })
  it.each([{ ...authResponse(), expiresIn: 0 }, { ...authResponse(), refreshToken: 'FORBIDDEN' }, { ...authResponse(), userId: 'invalid' }])('rejects malformed credentials without identity adoption', async body => {
    const { controller: auth, requests } = setup({ refresh: () => Response.json(body) })
    expect(await auth.completeOAuthHandoff()).toBe('unavailable')
    expect(auth.session.read().userId).toBeNull(); expect(requests).toHaveLength(1)
  })
  it('preserves a replacement account when an ignored-abort refresh completes late', async () => {
    const delayed = deferred<Response>(), { controller: auth, requests } = setup({ refresh: () => delayed.promise })
    const pending = auth.completeOAuthHandoff()
    await vi.waitFor(() => expect(requests).toHaveLength(1))
    const replacement = auth.session.replace(auth.session.read().generation, { userId, eventId, accessToken: 'replacement-token' })!
    delayed.resolve(Response.json(authResponse()))
    expect(await pending).toBe('unavailable'); expect(auth.session.isCurrent(replacement)).toBe(true)
    expect(auth.session.read().accessToken).toBe('replacement-token')
  })
  it('aborts the original transport scope and cannot adopt after route cancellation', async () => {
    const delayed = deferred<Response>(), { controller: auth, requests } = setup({ refresh: () => delayed.promise }), route = new AbortController()
    const pending = auth.completeOAuthHandoff(route.signal)
    await vi.waitFor(() => expect(requests).toHaveLength(1)); route.abort()
    expect(requests[0]!.signal.aborted).toBe(true)
    delayed.resolve(Response.json(authResponse())); expect(await pending).toBe('unavailable')
    expect(auth.session.read().userId).toBeNull(); expect(requests).toHaveLength(1)
  })
  it('cannot restore identity after logout interrupts refresh', async () => {
    const delayed = deferred<Response>(), { controller: auth, requests } = setup({ refresh: () => delayed.promise })
    const pending = auth.completeOAuthHandoff(); await vi.waitFor(() => expect(requests).toHaveLength(1))
    await auth.logout(); delayed.resolve(Response.json(authResponse()))
    expect(await pending).toBe('unavailable'); expect(auth.session.read().userId).toBeNull()
  })
  it('preserves verified identity and admission when the callback exits after adoption', async () => {
    const delayed = deferred<Response>(), route = new AbortController()
    const { controller: auth, requests } = setup({ admissionResponse: () => delayed.promise })
    const pending = auth.completeOAuthHandoff(route.signal)
    await vi.waitFor(() => expect(requests).toHaveLength(2))
    const installed = auth.session.read(); route.abort()
    delayed.resolve(Response.json({ decision: 'ADMITTED', policyRevision: '00000000-0000-4000-8000-000000000123',
      acceptedRevision: '00000000-0000-4000-8000-000000000123', serverTime: '2026-11-05T00:00:00Z' }))
    expect(await pending).toBe('unavailable'); expect(auth.session.isCurrent(installed)).toBe(true)
    expect(auth.read()).toMatchObject({ admission: 'ADMITTED', pending: null })
  })
  it('does not dispatch when a queued cookie lock is cancelled', async () => {
    const lock = deferred<void>(), policy = isolatedCookiePolicy()
    policy.run = async (_kind, work) => { await lock.promise; return work() }
    const { controller: auth, requests } = setup({ cookiePolicy: policy }), route = new AbortController()
    const pending = auth.completeOAuthHandoff(route.signal); route.abort(); lock.resolve()
    expect(await pending).toBe('unavailable'); expect(requests).toHaveLength(0)
  })
  it('does not dispatch for an already cancelled route', async () => {
    const { controller: auth, requests } = setup(), route = new AbortController(); route.abort()
    expect(await auth.completeOAuthHandoff(route.signal)).toBe('unavailable'); expect(requests).toHaveLength(0)
  })
  it('preserves an identity installed reentrantly while adoption notifies subscribers', async () => {
    const { controller: auth } = setup(); let replaced = false
    const stop = auth.subscribe(() => {
      if (replaced || !auth.session.read().userId) return
      replaced = true
      auth.session.replace(auth.session.read().generation, { userId, eventId, accessToken: 'reentrant-replacement' })
    })
    expect(await auth.completeOAuthHandoff()).toBe('unavailable')
    expect(auth.session.read().accessToken).toBe('reentrant-replacement'); stop()
  })
  it('does not invalidate a current authenticated identity merely because a callback was visited', async () => {
    const { controller: auth, requests } = setup()
    const before = auth.session.replace(auth.session.read().generation, { userId, eventId, accessToken: 'existing-token' })!
    expect(await auth.completeOAuthHandoff()).toBe('unavailable')
    expect(auth.session.isCurrent(before)).toBe(true); expect(requests).toHaveLength(0)
  })
  it('preserves the production refresh gate after successful explicit callback adoption', async () => {
    const { controller: auth, requests } = setup(); expect(await auth.completeOAuthHandoff()).toBe('ready')
    await expect(auth.refresh()).rejects.toMatchObject({ kind: 'authentication-required' })
    expect(requests.filter(request => request.url.endsWith('/refresh'))).toHaveLength(1)
  })
  it('fails closed before dispatch without an event or native cookie policy', async () => {
    const noEvent = setup({ event: null }), policy = { ...isolatedCookiePolicy(), available: false }, unavailable = setup({ cookiePolicy: policy })
    expect(await noEvent.controller.completeOAuthHandoff()).toBe('unavailable')
    expect(await unavailable.controller.completeOAuthHandoff()).toBe('unavailable')
    expect(noEvent.requests).toHaveLength(0); expect(unavailable.requests).toHaveLength(0)
  })
  it('uses the canonical credential expiry rather than retaining callback authority', async () => {
    vi.useFakeTimers()
    const { controller: auth } = setup({ refresh: () => Response.json({ ...authResponse(), expiresIn: 1 }) })
    expect(await auth.completeOAuthHandoff()).toBe('ready')
    await vi.advanceTimersByTimeAsync(1001)
    expect(auth.session.read().userId).toBeNull(); expect(auth.read().readiness).toBe('anonymous')
  })
})
