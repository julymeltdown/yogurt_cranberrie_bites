import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { createSessionController, type SessionController } from '../model/session'
import { decodeEnrollment, decodeResend, validEnrollmentPassword, validVerificationCode } from '../model/enrollment-contract'
import { authResponse, capabilities, deferred, eventId, isolatedCookiePolicy, userId } from './fixtures/data'
const password = '  unchanged password  ', email = 'member@example.com', code = '123456'
const ack = { eligibility: true, consent: true }, controllers: SessionController[] = []
beforeEach(() => vi.stubGlobal('location', { origin: 'https://mcr.example' }))
afterEach(() => { controllers.splice(0).forEach(c => c.dispose()); vi.unstubAllGlobals() })
async function setup(handler: (r: Request) => Response | Promise<Response> = r => Response.json(r.url.endsWith('/verify') ? { userId, status: 'VERIFIED' } : r.url.endsWith('/resend') ? { status: 'ACCEPTED' } : { userId, status: 'PENDING_VERIFICATION' }, { status: r.url.endsWith('/resend') ? 202 : 200 }), preview = true, timeoutMs = 1000, emailEnabled = true) {
  const requests: Request[] = [], cookiePolicy = isolatedCookiePolicy(), cookieRun = vi.spyOn(cookiePolicy, 'run')
  const fetch = vi.fn<typeof globalThis.fetch>(async input => {
    const r = input as Request
    if (r.url.endsWith('/capabilities')) return Response.json(emailEnabled ? capabilities() : { ...capabilities(), providers: capabilities().providers.map(p => ({ ...p, enabled: false, disabled_reason: 'NOT_CONFIGURED' })) })
    requests.push(r.clone())
    if (r.url.endsWith('/login')) return Response.json(authResponse())
    if (r.url.endsWith('/logout')) return new Response(null, { status: 204 })
    return handler(r)
  })
  const auth = createSessionController({ eventId, fetch, cookiePolicy, syntheticEnrollment: preview, timeoutMs }); controllers.push(auth)
  auth.start(); await vi.waitFor(() => expect(auth.read().capabilities).toBe('ready'))
  return { auth, requests, fetch, cookieRun }
}
it.each(['12345678', ' '.repeat(8), '가'.repeat(42) + 'aa', 'a'.repeat(128)])('accepts raw UTF8 password boundary %j', value => expect(validEnrollmentPassword(value)).toBe(true))
it.each(['short', '가'.repeat(43), 'a'.repeat(129), '\ud80012345678'])('rejects invalid UTF8 password %j', value => expect(validEnrollmentPassword(value)).toBe(false))
it.each(['12345', '1234567', '１２３４５６', ' 12345', 'abcdef'])('rejects non-six-digit code %j', value => expect(validVerificationCode(value)).toBe(false))
it('keeps production signup policy disabled even with provider available and checked boxes', async () => {
  const { auth, requests } = await setup(undefined, false)
  expect(await auth.enrollment.signup(email, password, ack)).toEqual({ kind: 'policy-unavailable' }); expect(requests).toHaveLength(0)
})
it('checks both synthetic acknowledgements before sending the exact two-field payload; never installs identity', async () => {
  const { auth, requests, cookieRun } = await setup()
  for (const a of [{ eligibility: false, consent: true }, { eligibility: true, consent: false }]) expect(await auth.enrollment.signup(email, password, a)).toEqual({ kind: 'invalid-input' })
  expect(await auth.enrollment.signup(email, password, ack)).toEqual({ kind: 'created' })
  expect(await requests[0]!.json()).toEqual({ email, password }); expect(requests[0]!.headers.get('Authorization')).toBeNull()
  expect(cookieRun).not.toHaveBeenCalled(); expect(auth.session.read().accessToken).toBeNull(); expect(auth.session.read().userId).toBeNull()
  expect(auth.queryClient.getQueryCache().getAll()).toHaveLength(0); expect(auth.queryClient.getMutationCache().getAll()).toHaveLength(0)
})
it('verifies existing accounts and accepts only202ACCEPTED resend without synthetic signup qualification', async () => {
  const { auth, requests } = await setup(undefined, false)
  expect(await auth.enrollment.verify(email, code)).toEqual({ kind: 'verified' })
  expect(await auth.enrollment.resend(email)).toEqual({ kind: 'received' })
  expect(await requests[0]!.json()).toEqual({ email, code }); expect(await requests[1]!.json()).toEqual({ email })
  expect(auth.session.read().accessToken).toBeNull()
})
it.each(['signup', 'verify', 'resend'] as const)('fences duplicate and simultaneous auth actions while %s pending', async kind => {
  const late = deferred<Response>(), { auth, requests } = await setup(() => late.promise)
  const operation = kind === 'signup' ? auth.enrollment.signup(email, password, ack) : kind === 'verify' ? auth.enrollment.verify(email, code) : auth.enrollment.resend(email)
  expect(await auth.enrollment.resend(email)).toEqual({ kind: 'busy' })
  expect(await auth.login(email, password)).toBe(false)
  expect(await auth.requestPasswordReset(email)).toBe('unavailable')
  expect(await auth.confirmPasswordReset('A'.repeat(43), password)).toBe('unavailable')
  await vi.waitFor(() => expect(requests).toHaveLength(1))
  late.resolve(Response.json(kind === 'resend' ? { status: 'ACCEPTED' } : { userId, status: kind === 'signup' ? 'PENDING_VERIFICATION' : 'VERIFIED' }, { status: kind === 'resend' ? 202 : 200 }))
  expect((await operation).kind).toBe(kind === 'signup' ? 'created' : kind === 'verify' ? 'verified' : 'received')
})
it('blocks enrollment during recovery and login, using the same operation owner', async () => {
  const late = deferred<Response>(), { auth } = await setup(() => late.promise)
  const op = auth.requestPasswordReset(email)
  expect(await auth.enrollment.verify(email, code)).toEqual({ kind: 'busy' })
  late.resolve(Response.json({ status: 'SENT' })); await op
  // Login reserves its pending state synchronously before its cookie work.
  const login = auth.login(email, password)
  expect(await auth.enrollment.resend(email)).toEqual({ kind: 'busy' }); await login
  expect(await auth.enrollment.verify(email, code)).toEqual({ kind: 'unavailable' })
})
it.each(['privacy', 'account', 'cancel', 'dispose'] as const)('aborts and fences late successful enrollment after %s', async action => {
  const late = deferred<Response>(), { auth, requests } = await setup(() => late.promise)
  const op = auth.enrollment.verify(email, code)
  await vi.waitFor(() => expect(requests).toHaveLength(1))
  if (action === 'privacy') auth.privacy.changePolicy(auth.privacy.capture(), 'consent-revoked')
  else if (action === 'account') auth.session.replace(auth.session.read().generation, { userId, eventId, accessToken: 'other-user-token' })
  else if (action === 'cancel') auth.enrollment.cancel()
  else auth.dispose()
  expect(requests[0]!.signal.aborted).toBe(true)
  late.resolve(Response.json({ userId, status: 'VERIFIED' })); expect(await op).toEqual({ kind: 'cancelled' })
})
it('bounds a signal-ignoring transport and reports uncertainty after dispatch', async () => {
  const { auth, requests } = await setup(() => new Promise(() => {}), true, 30)
  expect((await auth.enrollment.verify(email, code)).kind).toBe('uncertain'); expect(requests).toHaveLength(1)
})
it.each([400, 401, 404, 410, 422])('offers recovery for unusable/expired code HTTP%s', async status => {
  const { auth } = await setup(() => Response.json({ code: 'PRIVATE', message: 'secret error' }, { status }))
  expect(await auth.enrollment.verify(email, code)).toEqual({ kind: 'invalid-code' })
})
it('keeps duplicate signup response neutral without a check-email request', async () => {
  const { auth, requests } = await setup(() => Response.json({ code: 'CONFLICT', message: 'Email registered: private' }, { status: 409 }))
  expect(await auth.enrollment.signup(email, password, ack)).toEqual({ kind: 'not-created' }); expect(requests).toHaveLength(1)
})
it.each([429, 503])('honors only bounded server RetryAfter on HTTP%s and never replays', async status => {
  const { auth, requests } = await setup(() => Response.json({ code: 'WAIT', message: 'raw' }, { status, headers: { 'Retry-After': '2' } }))
  const before = Date.now(), r = await auth.enrollment.resend(email)
  expect(r.kind).toBe(status === 429 ? 'rate-limited' : 'uncertain'); expect(r.retryAt).toBeGreaterThanOrEqual(before + 2000)
  expect(await auth.enrollment.verify(email, code)).toEqual({ kind: 'rate-limited', retryAt: r.retryAt }); expect(requests).toHaveLength(1)
})
it('does not invent retry or expiry metadata for opaque accepted or429 responses', async () => {
  const { auth } = await setup(() => Response.json({ code: 'WAIT', message: 'raw' }, { status: 429 }))
  expect(await auth.enrollment.resend(email)).toEqual({ kind: 'rate-limited' }); expect(auth.enrollment.retryAt()).toBe(0)
})
it.each(['FUTURE_STATUS', 'VERIFIED', 'PENDING_VERIFICATION'])('does not derive signup completion from arbitrary status %s', async status => {
  const { auth } = await setup(() => Response.json({ userId, status }))
  expect((await auth.enrollment.signup(email, password, ack)).kind).toBe(status === 'PENDING_VERIFICATION' ? 'created' : 'uncertain')
  expect((await auth.enrollment.verify(email, code)).kind).toBe(status === 'VERIFIED' ? 'verified' : 'uncertain')
})
it.each([{ status: 'ACCEPTED', userId }, { status: 'SENT' }, { status: 1 }])('rejects malformed resend %j with uncertainty', async body => {
  const { auth } = await setup(() => Response.json(body, { status: 202 }))
  expect((await auth.enrollment.resend(email)).kind).toBe('uncertain')
})
it('rejects wrong successfulHTTP status and malformed enrollment schema', async () => {
  const { auth } = await setup(() => Response.json({ userId: 'notuuid', status: 'VERIFIED' }))
  expect((await auth.enrollment.verify(email, code)).kind).toBe('uncertain')
  const context = { status: 200, path: '/api/auth/email/resend', scope: { generation: 0, userId: null, eventId: null } }
  expect(() => decodeResend({ status: 'ACCEPTED' }, context)).toThrow()
  expect(decodeEnrollment({ userId, status: 'arbitrary-string' }, context)).toEqual({ status: 'arbitrary-string' })
})
it('never dispatches with a disabled provider, invalid input, or preaborted signal', async () => {
  const { auth, requests } = await setup(undefined, true, 1000, false)
  expect((await auth.enrollment.verify(email, code)).kind).toBe('unavailable'); expect(requests).toHaveLength(0)
  const ready = await setup()
  expect((await ready.auth.enrollment.signup(email, 'short', ack)).kind).toBe('invalid-input')
  expect((await ready.auth.enrollment.verify(email, '12345')).kind).toBe('invalid-input')
  expect((await ready.auth.enrollment.resend('bad email')).kind).toBe('invalid-input')
  expect((await ready.auth.enrollment.verify(email, code, AbortSignal.abort())).kind).toBe('cancelled')
  expect(ready.requests).toHaveLength(0)
})

it('logout locally invalidates a pending enrollment before its late completion', async () => {
  const late = deferred<Response>(), { auth, requests } = await setup(() => late.promise)
  const op = auth.enrollment.verify(email, code)
  await vi.waitFor(() => expect(requests).toHaveLength(1))
  expect(await auth.logout()).toBe(true)
  expect(requests[0]!.signal.aborted).toBe(true)
  late.resolve(Response.json({ userId, status: 'VERIFIED' })); expect(await op).toEqual({ kind: 'cancelled' })
})

it.each([null, 'malformed-event'])('discovers anonymous verification/resend availability without event %j and preserves FE09 readiness', async event => {
  const pending = deferred<Response>(), policy = isolatedCookiePolicy(), cookieRun = vi.spyOn(policy, 'run')
  const fetch = vi.fn<typeof globalThis.fetch>(async input => {
    const r = input as Request
    if (r.url.endsWith('/capabilities')) return pending.promise
    if (r.url.endsWith('/verify')) return Response.json({ userId, status: 'VERIFIED' })
    if (r.url.endsWith('/resend')) return Response.json({ status: 'ACCEPTED' }, { status: 202 })
    throw new Error('Unexpected request')
  })
  const auth = createSessionController({ eventId: event, fetch, cookiePolicy: policy }); controllers.push(auth)
  auth.start(); expect(fetch).not.toHaveBeenCalled()
  expect((await auth.enrollment.verify(email, code)).kind).toBe('unavailable')
  const a = auth.checkEnrollmentAvailability(), b = auth.checkEnrollmentAvailability()
  await vi.waitFor(() => expect(fetch).toHaveBeenCalledOnce())
  pending.resolve(Response.json(capabilities())); await Promise.all([a, b])
  expect(auth.read()).toMatchObject({ readiness: 'unavailable', capabilities: 'ready', emailEnabled: false })
  expect((await auth.enrollment.verify(email, code)).kind).toBe('verified')
  expect((await auth.enrollment.resend(email)).kind).toBe('received')
  expect((await auth.enrollment.signup(email, password, ack)).kind).toBe('policy-unavailable')
  expect(await auth.login(email, password)).toBe(false); expect(auth.session.read().accessToken).toBeNull()
  expect(cookieRun).not.toHaveBeenCalled(); expect(fetch).toHaveBeenCalledTimes(3)
})
it('allows explicit enrollment discovery retry after failed no-event provider discovery', async () => {
  let attempt = 0
  const fetch = vi.fn<typeof globalThis.fetch>(async () => ++attempt === 1 ? new Response(null, { status: 503 }) : Response.json(capabilities()))
  const auth = createSessionController({ eventId: null, fetch, cookiePolicy: isolatedCookiePolicy() }); controllers.push(auth)
  auth.start(); await auth.checkEnrollmentAvailability()
  expect(auth.enrollment.available()).toBe(false); expect(auth.read().capabilities).toBe('error')
  await auth.checkEnrollmentAvailability(); expect(auth.enrollment.available()).toBe(true)
  expect(auth.read().readiness).toBe('unavailable'); expect(fetch).toHaveBeenCalledTimes(2)
})
