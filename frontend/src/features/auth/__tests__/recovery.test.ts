import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createSessionController, type SessionController } from '../model/session'
import { validResetPassword, validResetToken } from '../model/recovery-contract'
import { authResponse, capabilities, deferred, eventId, isolatedCookiePolicy, userId } from './fixtures/data'
import { TransportError } from '../../../shared/api/errors'
const token = 'A'.repeat(43), password = '  eight bytes  '
const controllers: SessionController[] = []
beforeEach(() => vi.stubGlobal('location', { origin: 'https://mcr.example' }))
afterEach(() => { controllers.splice(0).forEach(c => c.dispose()); vi.useRealTimers(); vi.unstubAllGlobals() })
function setup(handler: (request: Request) => Response | Promise<Response> = () => Response.json({ status: 'SENT' }), timeoutMs = 1000) {
  const cookiePolicy = isolatedCookiePolicy(), run = vi.spyOn(cookiePolicy, 'run')
  const fetch = vi.fn<typeof globalThis.fetch>(async input => {
    const request = input as Request
    if (request.url.endsWith('/capabilities')) return Response.json(capabilities())
    if (request.url.endsWith('/login')) return Response.json(authResponse())
    if (request.url.endsWith('/logout')) return new Response(null, { status: 204 })
    return handler(request)
  })
  const auth = createSessionController({ eventId: null, fetch, cookiePolicy, timeoutMs }); controllers.push(auth)
  return { auth, fetch, cookiePolicy, run }
}
describe('frozen recovery contract and explicit no-retry commands', () => {
  it.each(['SENT', 'RESET', 'ACCEPTED', 'future-source-status', ''])('accepts string status %j without displaying or inferring delivery from it', async status => {
    const { auth, fetch } = setup(() => Response.json({ status }))
    expect(await auth.requestPasswordReset('unknown@example.com')).toBe('received')
    expect(fetch).toHaveBeenCalledOnce(); expect(auth.session.read().accessToken).toBeNull()
  })
  it('uses the canonical transport and casing, with no auth header, referrer, cache or retry', async () => {
    const requests: Request[] = [], { auth, run } = setup(request => { requests.push(request.clone()); return Response.json({ status: 'RESET' }) })
    expect(await auth.requestPasswordReset('fan@example.com')).toBe('received')
    expect(await auth.confirmPasswordReset(token, password)).toBe('confirmed')
    expect(requests.map(r => new URL(r.url).pathname)).toEqual(['/api/auth/password/reset/request', '/api/auth/password/reset/confirm'])
    expect(await requests[0]!.json()).toEqual({ email: 'fan@example.com' })
    expect(await requests[1]!.json()).toEqual({ token, newPassword: password })
    for (const r of requests) {
      expect(r.method).toBe('POST'); expect(r.headers.get('X-MCR-Intent')).toBe('browser')
      expect(r.headers.has('Authorization')).toBe(false); expect(r.credentials).toBe('same-origin')
      expect(r.cache).toBe('no-store'); expect(r.referrerPolicy).toBe('no-referrer'); expect(r.redirect).toBe('error')
    }
    expect(run.mock.calls.map(([kind]) => kind)).toEqual(['recovery'])
    expect(run.mock.calls[0]![2]).toBeInstanceOf(AbortSignal)
    expect(auth.queryClient.getQueryCache().getAll()).toHaveLength(0); expect(auth.queryClient.getMutationCache().getAll()).toHaveLength(0)
  })
  it.each([['1234567', false], ['12345678', true], ['a'.repeat(128), true], ['a'.repeat(129), false],
    ['é'.repeat(4), true], ['é'.repeat(64), true], ['é'.repeat(65), false], ['😀'.repeat(32), true], ['😀'.repeat(33), false],
    [' '.repeat(8), true], ['\ud800abcde123', false]] as const)('measures raw UTF-8 password bytes: %j', (value, valid) => {
    expect(validResetPassword(value)).toBe(valid)
  })
  it('does not dispatch invalid email/password/token and never trims passwords', async () => {
    const { auth, fetch } = setup()
    expect(await auth.requestPasswordReset('bad')).toBe('invalid-input')
    expect(await auth.confirmPasswordReset(token, 'short')).toBe('invalid-input')
    expect(await auth.confirmPasswordReset('token with whitespace', password)).toBe('invalid-token')
    expect(fetch).not.toHaveBeenCalled()
    expect(validResetToken('')).toBe(false); expect(validResetToken('a'.repeat(8193))).toBe(false)
    expect(validResetToken('token\n')).toBe(false); expect(validResetToken(token)).toBe(true)
  })
  it.each([null, {}, { status: 1 }, { userId, accessToken: 'must-not-import' }])('rejects malformed success %j without authority or success', async body => {
    const { auth, fetch } = setup(() => Response.json(body))
    expect(await auth.confirmPasswordReset(token, password)).toBe('uncertain')
    expect(fetch).toHaveBeenCalledOnce(); expect(auth.session.read().accessToken).toBeNull()
  })
  it('does not accept a wrong success HTTP status', async () => {
    const { auth } = setup(() => Response.json({ status: 'RESET' }, { status: 202 }))
    expect(await auth.confirmPasswordReset(token, password)).toBe('uncertain')
  })
  it.each([400, 401, 404, 422])('keeps invalid/expired/used token HTTP %s neutral and explicit', async status => {
    const { auth, fetch } = setup(() => Response.json({ code: 'GATEWAY_ERROR', message: `${token} raw private error` }, { status }))
    expect(await auth.confirmPasswordReset(token, password)).toBe('invalid-token'); expect(fetch).toHaveBeenCalledOnce()
  })
  it.each([403, 404])('does not infer account existence or email delivery from request HTTP %s', async status => {
    const { auth } = setup(() => Response.json({ code: 'RAW', message: 'Account not found' }, { status }))
    expect(await auth.requestPasswordReset('unknown@example.com')).toBe('unavailable')
  })
  it('maps throttling without scheduling a retry', async () => {
    const { auth, fetch } = setup(() => Response.json({ code: 'GATEWAY_429', message: 'raw' }, { status: 429, headers: { 'Retry-After': '1' } }))
    expect(await auth.requestPasswordReset('fan@example.com')).toBe('rate-limited'); expect(fetch).toHaveBeenCalledOnce()
  })
  it.each(['network', 'server', 'invalid-body'] as const)('does not claim or replay uncertain %s success', async mode => {
    const { auth, fetch } = setup(() => {
      if (mode === 'network') throw new TypeError('private transport detail')
      return mode === 'server' ? new Response('raw', { status: 503 }) : new Response('<html>raw</html>', { headers: { 'Content-Type': 'text/html' } })
    })
    expect(await auth.confirmPasswordReset(token, password)).toBe('uncertain'); expect(fetch).toHaveBeenCalledOnce()
  })
})
describe('single canonical generation and recovery lifetime', () => {
  it('guards duplicate request/confirm commands until the first one settles', async () => {
    const delayed = deferred<Response>(), { auth, fetch } = setup(() => delayed.promise)
    const first = auth.requestPasswordReset('fan@example.com')
    expect(await auth.requestPasswordReset('fan@example.com')).toBe('busy')
    expect(await auth.confirmPasswordReset(token, password)).toBe('busy')
    await vi.waitFor(() => expect(fetch).toHaveBeenCalledOnce())
    delayed.resolve(Response.json({ status: 'SENT' })); expect(await first).toBe('received')
  })
  it('revokes canonical identity/private state before confirmation, never installs an auth response or second identity', async () => {
    let auth!: SessionController, cleared = false
    const fixture = setup(() => {
      expect(cleared).toBe(true); expect(auth.session.read().accessToken).toBeNull()
      expect(auth.queryClient.getQueryCache().getAll()).toHaveLength(0)
      return Response.json({ status: 'RESET' })
    }); auth = fixture.auth
    auth.session.replace(0, { userId, eventId, accessToken: 'old-memory-token' })
    const signal = auth.session.read().signal
    auth.queryClient.setQueryData(['private', userId], 'private-data'); auth.privacy.registerCleanup(() => { cleared = true })
    expect(await auth.confirmPasswordReset(token, password)).toBe('confirmed')
    expect(signal.aborted).toBe(true); expect(auth.session.read().userId).toBeNull()
  })
  it('reserves confirmation before synchronous invalidation observers can start a competing login', async () => {
    const { auth } = setup()
    let nested: Promise<boolean> | undefined
    auth.subscribe(() => { nested = auth.login('fan@example.com', password) })
    expect(await auth.confirmPasswordReset(token, password)).toBe('confirmed'); expect(await nested).toBe(false)
  })
  it.each(['logout', 'account-change', 'peer', 'dispose', 'cancel'] as const)('aborts and fences late ignored-abort response after %s', async change => {
    const delayed = deferred<Response>(), { auth, fetch, cookiePolicy } = setup(() => delayed.promise)
    const pending = auth.requestPasswordReset('fan@example.com')
    await vi.waitFor(() => expect(fetch).toHaveBeenCalledOnce())
    const request = fetch.mock.calls[0]![0] as Request
    if (change === 'logout') await auth.logout()
    else if (change === 'account-change') auth.session.replace(auth.session.read().generation, { userId, eventId, accessToken: 'replacement-token' })
    else if (change === 'peer') cookiePolicy.invalidate()
    else if (change === 'dispose') auth.dispose()
    else auth.cancelRecovery()
    expect(request.signal.aborted).toBe(true); expect(await pending).toBe('cancelled')
    delayed.resolve(Response.json({ status: 'SENT' }))
    await Promise.resolve(); expect(auth.session.read().accessToken).not.toBe('late-secret')
  })
  it('bounds a hung request that ignores abort, without automatic replay', async () => {
    const delayed = deferred<Response>(), { auth, fetch } = setup(() => delayed.promise, 25)
    expect(await auth.confirmPasswordReset(token, password)).toBe('uncertain')
    expect(fetch).toHaveBeenCalledOnce(); expect((fetch.mock.calls[0]![0] as Request).signal.aborted).toBe(true)
    delayed.resolve(Response.json({ status: 'RESET' }))
  })
  it('rejects a pre-aborted caller without dispatch or local identity revocation', async () => {
    const { auth, fetch } = setup(), abort = new AbortController()
    auth.session.replace(0, { userId, eventId, accessToken: 'old-token' }); abort.abort()
    expect(await auth.confirmPasswordReset(token, password, abort.signal)).toBe('cancelled')
    expect(fetch).not.toHaveBeenCalled(); expect(auth.session.read().accessToken).toBe('old-token')
  })
  it('requires the existing coordination policy before dispatching confirmation', async () => {
    const { auth, fetch, cookiePolicy } = setup()
    Object.defineProperty(cookiePolicy, 'available', { value: false })
    expect(await auth.confirmPasswordReset(token, password)).toBe('unavailable'); expect(fetch).not.toHaveBeenCalled()
  })
  it('cancels a queued confirmation before coordination dispatch without retaining a busy slot', async () => {
    const { auth, fetch, run } = setup()
    run.mockImplementation(async (_kind, _work, signal) => new Promise((_resolve, reject) => {
      signal!.addEventListener('abort', () => reject(new TransportError('aborted')), { once: true })
    }))
    const pending = auth.confirmPasswordReset(token, password)
    auth.cancelRecovery(); expect(await pending).toBe('cancelled'); expect(fetch).not.toHaveBeenCalled()
    expect(await auth.requestPasswordReset('fan@example.com')).toBe('received')
  })
  it('does not let a feature bypass the canonical recovery methods through request()', async () => {
    const { auth, fetch } = setup()
    await expect(auth.request({ path: '/api/auth/password/reset/confirm', method: 'POST', access: 'public', errorFamily: 'auth', decode: x => x })).rejects.toBeInstanceOf(TransportError)
    expect(fetch).not.toHaveBeenCalled()
  })
})
