import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { createSessionController, type SessionController } from '../../../features/auth/model/session'
import { authResponse, capabilities, deferred, eventId, fixture, isolatedCookiePolicy, userId } from '../../../features/auth/__tests__/fixtures/data'
import { admission } from '../../../features/auth/__tests__/enrollment-consent/fixtures'
const live: SessionController[] = []
beforeEach(() => vi.stubGlobal('location', { origin: 'https://mcr.example' }))
afterEach(() => { live.splice(0).forEach(c => c.dispose()); vi.unstubAllGlobals() })
function setup(handler: (r: Request) => Response | Promise<Response> = () => Response.json(admission()), email = true) {
  const fetch = vi.fn<typeof globalThis.fetch>(async input => {
    const r = input as Request
    if (r.url.endsWith('/capabilities')) { const c = capabilities(); return Response.json(email ? c : { ...c, providers: c.providers.map(p => ({ ...p, enabled: false, disabled_reason: 'NOT_CONFIGURED' })) }) }
    if (r.url.endsWith('/login')) return Response.json(authResponse())
    if (r.url.endsWith('/bootstrap')) return Response.json(fixture())
    if (r.url.endsWith('/logout')) return new Response(null, { status: 204 })
    return handler(r)
  })
  const auth = createSessionController({ eventId, fetch, cookiePolicy: isolatedCookiePolicy(), timeoutMs: 1000 }); live.push(auth)
  return { auth, fetch }
}
const read = { path: `/api/concert/events/${eventId}/me`, method: 'GET' as const, access: 'protected' as const, errorFamily: 'core' as const, decode: (v: unknown) => v }
async function signIn(c: SessionController) { c.start(); await vi.waitFor(() => expect(c.read().emailEnabled).toBe(true)); return c.login('test@example.invalid', 'unchanged-password') }
it('constructs inertly and never restores cookies or starts enrollment implicitly', () => {
  const { auth, fetch } = setup(); expect(fetch).not.toHaveBeenCalled(); expect(auth.read().admission).toBe('unchecked')
})
it.each(['CONSENT_REQUIRED', 'POLICY_UNAVAILABLE', 'ACCOUNT_INACTIVE'])('retains canonical login identity and blocks bootstrap/domain calls on %s', async decision => {
  const { auth, fetch } = setup(() => Response.json(admission(decision)))
  expect(await signIn(auth)).toBe(true); const scope = auth.session.read()
  expect(scope.userId).toBe(userId); expect(auth.read()).toMatchObject({ admission: decision, readiness: 'unavailable', pending: null })
  await expect(auth.loadBootstrap({ ...scope, userId, eventId }, new AbortController().signal)).rejects.toHaveProperty('name', 'ShellLoadError')
  await expect(auth.request(read)).rejects.toHaveProperty('kind', 'invalid-request')
  expect(auth.session.read()).toBe(scope)
  expect(fetch.mock.calls.some(([r]) => /\/(bootstrap|me|signup|attempts|refresh)$/.test((r as Request).url))).toBe(false)
})
it('checks an existing canonical provider/restored identity independently of email capability', async () => {
  const { auth, fetch } = setup(undefined, false)
  auth.session.replace(0, { userId, eventId, accessToken: 'provider-canonical' }); auth.start()
  await vi.waitFor(() => expect(auth.read().admission).toBe('ADMITTED'))
  expect(auth.read().emailEnabled).toBe(false)
  const request = fetch.mock.calls.map(([r]) => r as Request).find(r => r.url.endsWith('/admission'))!
  expect(request.headers.get('Authorization')).toBe('Bearer provider-canonical')
  expect(fetch.mock.calls.some(([r]) => /\/(login|refresh|signup)$/.test((r as Request).url))).toBe(false)
})
it('rechecks server admission before every bootstrap without withdrawing the admitted lease during the read', async () => {
  const late = deferred<Response>(); let checks = 0
  const { auth, fetch } = setup(() => ++checks === 1 ? Response.json(admission()) : late.promise)
  await signIn(auth); const scope = auth.session.read(), seen: string[] = [], stop = auth.subscribe(() => seen.push(auth.read().readiness))
  const result = auth.loadBootstrap({ ...scope, userId, eventId }, new AbortController().signal)
  await vi.waitFor(() => expect(checks).toBe(2)); expect(auth.read()).toMatchObject({ admission: 'ADMITTED', admissionPending: true, readiness: 'ready' })
  expect(fetch.mock.calls.some(([r]) => (r as Request).url.endsWith('/bootstrap'))).toBe(false)
  late.resolve(Response.json(admission())); expect(await result).toHaveProperty('contractVersion', '0.1.0')
  expect(seen).not.toContain('checking'); stop()
})
it('does not let one aborted route cancel another caller sharing the canonical admission read', async () => {
  const late = deferred<Response>(), { auth, fetch } = setup(() => late.promise)
  auth.session.replace(0, { userId, eventId, accessToken: 'same-identity' })
  const controller = new AbortController(), first = auth.checkAdmission(controller.signal), second = auth.checkAdmission()
  controller.abort(); late.resolve(Response.json(admission()))
  expect(await first).toBe(false); expect(await second).toBe(true); expect(fetch).toHaveBeenCalledTimes(1)
  expect(auth.read().readiness).toBe('ready')
})
it('ignores a late admitted decision after canonical replacement', async () => {
  const late = deferred<Response>(), { auth } = setup(() => late.promise)
  auth.session.replace(0, { userId, eventId, accessToken: 'old' }); const pending = auth.checkAdmission()
  await Promise.resolve(); auth.session.replace(auth.session.read().generation, null)
  late.resolve(Response.json(admission())); expect(await pending).toBe(false)
  expect(auth.read()).toMatchObject({ admission: 'unchecked', readiness: 'anonymous' })
})
it('withdraws cached private work on current server revocation without replacing the identity', async () => {
  let revoked = false
  const { auth } = setup(() => Response.json(admission(revoked ? 'CONSENT_REQUIRED' : 'ADMITTED')))
  await signIn(auth); const scope = auth.session.read(), cleanup = vi.fn(), before = auth.privacy.capture()
  auth.privacy.registerCleanup(cleanup); const query = auth.privacy.options({ resource: 'participant', read: () => 'private' })
  await auth.queryClient.fetchQuery(query); revoked = true
  expect(await auth.checkAdmission()).toBe(false)
  expect(auth.session.read()).toBe(scope); expect(auth.read().readiness).toBe('unavailable')
  expect(auth.privacy.isCurrent(before)).toBe(false); expect(auth.queryClient.getQueryData(query.queryKey)).toBeUndefined(); expect(cleanup).toHaveBeenCalledTimes(1)
})
it.each([{}, { ...admission(), acceptedRevision: null }])('rejects malformed or unbound ADMITTED receipt %j without bootstrap', async receipt => {
  const { auth, fetch } = setup(() => Response.json(receipt)); await signIn(auth)
  expect(auth.read()).toMatchObject({ admission: 'ERROR', readiness: 'unavailable' }); expect(auth.session.read().userId).toBe(userId)
  expect(fetch.mock.calls.some(([r]) => (r as Request).url.endsWith('/bootstrap'))).toBe(false)
})
it('clears canonical authority on admission 401 without refresh or retry', async () => {
  const { auth, fetch } = setup(() => Response.json({ code: 'UNAUTHENTICATED', message: 'safe' }, { status: 401 }))
  expect(await signIn(auth)).toBe(false); expect(auth.session.read().userId).toBeNull()
  expect(fetch.mock.calls.filter(([r]) => (r as Request).url.endsWith('/admission'))).toHaveLength(1)
  expect(fetch.mock.calls.some(([r]) => (r as Request).url.endsWith('/refresh'))).toBe(false)
})
