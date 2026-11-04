// Synthetic HTTP/isolated cookie-policy fixtures only. No elapsed-expiry, native-cookie or real-service claim.
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { QueryObserver } from '@tanstack/react-query'
import { createSessionController, type SessionController } from '../../model/session'
import { authResponse, capabilities, deferred, eventId, fixture, isolatedCookiePolicy, userId } from '../fixtures/data'

const controllers: SessionController[] = []
beforeEach(() => vi.stubGlobal('location', { origin: 'https://mcr.example' }))
afterEach(() => { controllers.splice(0).forEach(c => c.dispose()); vi.unstubAllGlobals() })
const unauthorized = () => Response.json({ code: 'UNAUTHENTICATED', message: 'Fixture only', details: { reason: 'AUTHENTICATION_REQUIRED' } }, { status: 401 })
const read = () => ({ path: `/api/concert/events/${eventId}/me`, method: 'GET' as const, access: 'protected' as const, errorFamily: 'core' as const, decode: (value: unknown) => value })
function setup(handle: (request: Request) => Response | Promise<Response>) {
  const fetch = vi.fn<typeof globalThis.fetch>(async input => {
    const request = input as Request
    if (request.url.endsWith('/capabilities')) return Response.json(capabilities())
    if (request.url.endsWith('/login')) return Response.json(authResponse())
    return handle(request)
  })
  const auth = createSessionController({ eventId, fetch, qualifiedRefresh: true, cookiePolicy: isolatedCookiePolicy() })
  controllers.push(auth); auth.start()
  return { auth, fetch, calls: (suffix: string) => fetch.mock.calls.filter(([r]) => (r as Request).url.endsWith(suffix)) }
}
async function login(auth: SessionController) {
  await vi.waitFor(() => expect(auth.read().emailEnabled).toBe(true))
  expect(await auth.login('fixture@example.com', 'fixture-password')).toBe(true)
}
function privateState(auth: SessionController) {
  const draft = { value: 'fixture private draft', subscriptions: 1 }
  const cleanup = auth.privacy.registerCleanup(() => { draft.value = ''; draft.subscriptions = 0 })
  const observer = new QueryObserver(auth.queryClient, auth.privacy.options({ resource: 'participant', read: () => 'fixture private view' }))
  const stop = observer.subscribe(() => undefined)
  return { draft, observer, close: () => { stop(); observer.destroy(); cleanup() } }
}

it('qualified startup and a fresh controller still require explicit login; no cookie-only restore is attempted', async () => {
  const first = setup(() => { throw new Error('Unexpected fixture endpoint') })
  await login(first.auth); first.auth.dispose()
  const second = setup(() => { throw new Error('Unexpected fixture endpoint') })
  second.auth.start(); second.auth.start()
  await vi.waitFor(() => expect(second.auth.read().capabilities).toBe('ready'))
  expect(second.fetch).toHaveBeenCalledTimes(1)
  expect(second.auth.read().readiness).toBe('anonymous')
  expect(second.auth.session.read()).toMatchObject({ userId: null, accessToken: null })
  await expect(second.auth.refresh()).rejects.toMatchObject({ kind: 'authentication-required' })
  await expect(second.auth.loadBootstrap({ generation: second.auth.session.read().generation, userId, eventId }, new AbortController().signal)).rejects.toMatchObject({ reason: 'unauthenticated' })
  expect(second.fetch).toHaveBeenCalledTimes(1)
})

it('cancelling bootstrap during shared renewal does not cancel another read or retain a cancelled bootstrap result', async () => {
  const rotation = deferred<Response>()
  const { auth, calls } = setup(request => {
    if (request.url.endsWith('/refresh')) return rotation.promise
    if (request.headers.get('Authorization') === `Bearer ${authResponse().accessToken}`) return unauthorized()
    return Response.json(request.url.endsWith('/bootstrap') ? fixture() : { ok: true })
  })
  await login(auth)
  const before = auth.session.read(), retained = privateState(auth)
  await vi.waitFor(() => expect(retained.observer.getCurrentResult().data).toBe('fixture private view'))
  const abort = new AbortController()
  const bootstrap = auth.loadBootstrap({ ...before, userId, eventId }, abort.signal).catch(error => error)
  const other = auth.request(read())
  await vi.waitFor(() => expect(calls('/refresh')).toHaveLength(1))
  abort.abort(); rotation.resolve(Response.json({ ...authResponse(), accessToken: 'fixture-renewed' }))
  expect(await bootstrap).toMatchObject({ reason: 'unauthenticated' })
  expect((await other).data).toEqual({ ok: true })
  expect(calls('/bootstrap')).toHaveLength(1); expect(calls('/me')).toHaveLength(2); expect(calls('/refresh')).toHaveLength(1)
  expect(auth.session.read()).toMatchObject({ generation: before.generation, tokenVersion: 1, userId })
  expect(before.signal.aborted).toBe(false); expect(auth.read().readiness).toBe('ready')
  expect(retained.observer.getCurrentResult().data).toBe('fixture private view')
  expect(retained.draft.subscriptions).toBe(1); retained.close()
})

it('a held old refresh cannot overwrite a subsequent explicit login after logout', async () => {
  const rotation = deferred<Response>()
  const { auth, calls } = setup(request => request.url.endsWith('/refresh') ? rotation.promise : new Response(null, { status: 204 }))
  await login(auth)
  const before = auth.session.read(), renewal = auth.refresh().catch(error => error)
  await vi.waitFor(() => expect(calls('/refresh')).toHaveLength(1))
  expect(await auth.logout()).toBe(true); await login(auth)
  const next = auth.session.read()
  rotation.resolve(Response.json({ ...authResponse(), accessToken: 'fixture-stale-rotation' }))
  expect(await renewal).toMatchObject({ kind: 'session-changed' })
  expect(before.signal.aborted).toBe(true); expect(next.generation).toBeGreaterThan(before.generation)
  expect(auth.session.read()).toBe(next); expect(next.signal.aborted).toBe(false)
  expect(auth.read()).toMatchObject({ readiness: 'ready', pending: null, notice: null })
})

it('409 refresh fanout removes observer/draft/subscription authority and cannot restart from anonymous memory', async () => {
  const { auth, calls } = setup(request => request.url.endsWith('/refresh')
    ? Response.json({ code: 'REFRESH_CONFLICT', message: 'Fixture rotation conflict' }, { status: 409 })
    : request.url.endsWith('/logout') ? new Response(null, { status: 204 }) : unauthorized())
  await login(auth)
  const before = auth.session.read(), retained = privateState(auth)
  await vi.waitFor(() => expect(retained.observer.getCurrentResult().data).toBe('fixture private view'))
  const result = await Promise.allSettled([auth.loadBootstrap({ ...before, userId, eventId }, new AbortController().signal), auth.request(read()), auth.request(read())])
  expect(result.every(r => r.status === 'rejected')).toBe(true)
  expect(calls('/refresh')).toHaveLength(1); expect(calls('/bootstrap')).toHaveLength(1); expect(calls('/me')).toHaveLength(2)
  expect(before.signal.aborted).toBe(true); expect(auth.session.read().accessToken).toBeNull()
  expect(auth.read()).toMatchObject({ readiness: 'anonymous', notice: 'expired' })
  expect(retained.observer.getCurrentResult().data).toBeUndefined()
  expect(auth.queryClient.getQueryCache().getAll()).toHaveLength(0)
  expect(retained.draft).toEqual({ value: '', subscriptions: 0 })
  await expect(auth.refresh()).rejects.toMatchObject({ kind: 'authentication-required' })
  await expect(auth.request(read())).rejects.toMatchObject({ kind: 'authentication-required' })
  expect(calls('/refresh')).toHaveLength(1); expect(calls('/me')).toHaveLength(2)
  expect(await auth.logout()).toBe(true); expect(auth.read().logout).toBe('confirmed'); retained.close()
})

it('unavailable bootstrap reports shell failure without pretending that canonical authentication expired', async () => {
  const { auth, calls } = setup(request => request.url.endsWith('/bootstrap')
    ? Response.json({ code: 'UNAVAILABLE', message: 'Fixture unavailable' }, { status: 503 }) : Response.json({ ok: true }))
  await login(auth); const before = auth.session.read()
  await expect(auth.loadBootstrap({ ...before, userId, eventId }, new AbortController().signal)).rejects.toMatchObject({ reason: 'unavailable' })
  expect(auth.session.read()).toBe(before); expect(before.signal.aborted).toBe(false)
  expect(auth.read().readiness).toBe('ready'); expect(calls('/refresh')).toHaveLength(0)
  expect((await auth.request(read())).data).toEqual({ ok: true })
})

it('bootstrap that remains unauthorized after one refresh clears authority rather than looping', async () => {
  const { auth, calls } = setup(request => request.url.endsWith('/refresh')
    ? Response.json({ ...authResponse(), accessToken: 'fixture-renewed' }) : unauthorized())
  await login(auth); const before = auth.session.read()
  await expect(auth.loadBootstrap({ ...before, userId, eventId }, new AbortController().signal)).rejects.toMatchObject({ reason: 'unauthenticated' })
  expect(calls('/refresh')).toHaveLength(1); expect(calls('/bootstrap')).toHaveLength(2)
  expect(auth.read()).toMatchObject({ readiness: 'anonymous', notice: 'expired' })
  expect(before.signal.aborted).toBe(true)
})
