// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { createAccountController } from '../../../account/controller'
import { accepted, deferred, fixtureAuth, otherId, saga } from './fixtures'
const owned: ReturnType<typeof fixtureAuth>[] = []
const setup = (...args: Parameters<typeof fixtureAuth>) => { const h = fixtureAuth(...args); owned.push(h); return h }
afterEach(() => { owned.splice(0).forEach(h => h.dispose()); vi.restoreAllMocks() })
const operations = (h: ReturnType<typeof fixtureAuth>) => h.requests.filter(r => !/\/(login|capabilities)$/.test(new URL(r.url).pathname))
it('keeps the generic Auth gate and sends the narrow canonical DELETE without body or key', async () => {
  const h = setup(() => Response.json(accepted(), { status: 202 })); await h.login()
  await expect(h.auth.request({ path: '/api/auth/account', method: 'DELETE', access: 'protected', errorFamily: 'auth', decode: v => v })).rejects.toMatchObject({ kind: 'invalid-request' })
  const scope = h.auth.privacy.capture(), value = await h.auth.accountDeletion.request(scope, new AbortController().signal)
  expect(value.scope).toEqual(scope); expect(value.status).toBe(202); expect(operations(h)).toHaveLength(1)
  const r = operations(h)[0]!
  expect(new URL(r.url).pathname).toBe('/api/auth/account'); expect(r.method).toBe('DELETE'); expect(r.cache).toBe('no-store'); expect(r.credentials).toBe('same-origin')
  expect(r.headers.get('Authorization')).toMatch(/^Bearer /); expect(r.headers.get('X-MCR-Intent')).toBe('browser')
  expect(r.headers.has('Idempotency-Key')).toBe(false); expect(r.headers.has('Content-Type')).toBe(false); expect(await r.text()).toBe('')
})
it('latches one DELETE even if the narrow port is invoked twice', async () => {
  const late = deferred<Response>(), h = setup(() => late.promise); await h.login()
  const scope = h.auth.privacy.capture(), first = h.auth.accountDeletion.request(scope, new AbortController().signal)
  await expect(h.auth.accountDeletion.request(scope, new AbortController().signal)).rejects.toMatchObject({ kind: 'invalid-request' })
  late.resolve(Response.json(accepted(), { status: 202 })); await first; expect(operations(h).filter(r => r.method === 'DELETE')).toHaveLength(1)
})
it('uncertain DELETE recovers only by explicit GET under the exact suspended scope', async () => {
  const h = setup(r => r.method === 'DELETE' ? Promise.reject(new TypeError('offline')) : Response.json(saga())); await h.login()
  const account = createAccountController({ auth: h.auth, deletion: h.auth.accountDeletion })
  account.editConfirmation('DELETE_MY_ACCOUNT'); await account.requestDeletion()
  expect(account.read().phase).toBe('unknown'); expect(h.auth.privacy.capture().gate).toBe('suspended'); expect(operations(h)).toHaveLength(1)
  await account.requestDeletion(); expect(operations(h)).toHaveLength(1); await account.checkDeletion(); expect(account.read().phase).toBe('pending')
  expect(operations(h).map(r => [r.method, new URL(r.url).pathname])).toEqual([['DELETE', '/api/auth/account'], ['GET', '/api/auth/account/deletion']]); account.dispose()
})
it('arbitrary suspension grants no status bypass', async () => {
  const h = setup(); await h.login(); h.auth.privacy.changePolicy(h.auth.privacy.capture(), 'suspended')
  await expect(h.auth.accountDeletion.status(h.auth.privacy.capture(), new AbortController().signal)).rejects.toMatchObject({ kind: 'authentication-required' }); expect(operations(h)).toHaveLength(0)
})
it('another policy bump invalidates the recovery capability', async () => {
  const h = setup(() => Response.json(accepted(), { status: 202 })); await h.login()
  const account = createAccountController({ auth: h.auth, deletion: h.auth.accountDeletion }); account.editConfirmation('DELETE_MY_ACCOUNT'); await account.requestDeletion()
  h.auth.privacy.changePolicy(h.auth.privacy.capture(), 'consent-revoked')
  await expect(h.auth.accountDeletion.status(h.auth.privacy.capture(), new AbortController().signal)).rejects.toMatchObject({ kind: 'authentication-required' })
  expect(operations(h)).toHaveLength(1); expect(account.read().progress).toBeNull(); account.dispose()
})
it.each(['DELETE', 'GET'])('%s 401 clears identity without refresh/replay', async method => {
  const h = setup(() => Response.json({ code: 'UNAUTHENTICATED', message: 'PRIVATE diagnostic' }, { status: 401 })); await h.login()
  const scope = h.auth.privacy.capture(), signal = new AbortController().signal
  await expect(method === 'DELETE' ? h.auth.accountDeletion.request(scope, signal) : h.auth.accountDeletion.status(scope, signal)).rejects.toMatchObject({ status: 401 })
  expect(h.auth.session.read().userId).toBeNull(); expect(h.auth.read().notice).toBe('expired'); expect(operations(h)).toHaveLength(1)
})
it.each(['identity', 'event', 'policy', 'dispose'] as const)('fences ignored-abort result after %s', async kind => {
  const late = deferred<Response>(), h = setup(() => late.promise); await h.login()
  const scope = h.auth.privacy.capture(), result = h.auth.accountDeletion.request(scope, new AbortController().signal).catch(error => error)
  await vi.waitFor(() => expect(operations(h)).toHaveLength(1))
  if (kind === 'identity' || kind === 'event') h.auth.session.replace(scope.generation, { userId: kind === 'identity' ? otherId : scope.userId!, eventId: kind === 'event' ? otherId : scope.eventId, accessToken: 'NEXT' })
  else if (kind === 'policy') h.auth.privacy.changePolicy(scope, 'block'); else h.auth.dispose()
  late.resolve(Response.json(accepted(), { status: 202 })); expect(await result).toHaveProperty('kind'); if (kind === 'identity') expect(h.auth.session.read().userId).toBe(otherId)
})
it('pre-aborted and stale scopes dispatch nothing', async () => {
  const h = setup(); await h.login(); const scope = h.auth.privacy.capture(), abort = new AbortController(); abort.abort()
  await expect(h.auth.accountDeletion.request(scope, abort.signal)).rejects.toHaveProperty('kind'); h.auth.privacy.changePolicy(scope, 'block')
  await expect(h.auth.accountDeletion.request(scope, new AbortController().signal)).rejects.toHaveProperty('kind'); expect(operations(h)).toHaveLength(0)
})
it('recovery never extends the captured credential expiry', async () => {
  const h = setup(() => Response.json(accepted(), { status: 202 }), 2); await h.login(); const initial = Date.now(), scope = h.auth.privacy.capture()
  await h.auth.accountDeletion.request(scope, new AbortController().signal); h.auth.privacy.changePolicy(scope, 'suspended'); vi.spyOn(Date, 'now').mockReturnValue(initial + 3000)
  await expect(h.auth.accountDeletion.status(h.auth.privacy.capture(), new AbortController().signal)).rejects.toMatchObject({ kind: 'authentication-required' }); expect(operations(h)).toHaveLength(1)
})
it('caller cancellation promptly ends an in-flight read', async () => {
  const late = deferred<Response>(), h = setup(() => late.promise); await h.login(); const cancel = new AbortController()
  const result = h.auth.accountDeletion.status(h.auth.privacy.capture(), cancel.signal).catch(error => error)
  await vi.waitFor(() => expect(operations(h)).toHaveLength(1)); cancel.abort(); expect(await result).toHaveProperty('kind'); late.resolve(Response.json(saga())); expect(h.auth.session.read().userId).not.toBeNull()
})
it('a later unrelated suspension cannot reuse a settled read as a recovery handoff', async () => {
  const h = setup(() => Response.json(saga())); await h.login()
  await h.auth.accountDeletion.status(h.auth.privacy.capture(), new AbortController().signal)
  await new Promise(resolve => setTimeout(resolve, 1)); h.auth.privacy.changePolicy(h.auth.privacy.capture(), 'suspended')
  await expect(h.auth.accountDeletion.status(h.auth.privacy.capture(), new AbortController().signal)).rejects.toMatchObject({ kind: 'authentication-required' })
  expect(operations(h)).toHaveLength(1)
})
