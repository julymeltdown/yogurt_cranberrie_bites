import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { createSessionController, type SessionController } from '../model/session'
import { createHttpClient, type HttpRequest } from '../../../shared/api/http'
import { eventId, userId, isolatedCookiePolicy, deferred } from './fixtures/data'
import { withAdmission } from './fixtures/admission'
import { type DomainOperationId } from '../../../shared/api/domain-operations.generated'

const key = '00000000-0000-4000-8000-000000000501', party = '00000000-0000-4000-8000-000000000601'
const prefix = `/api/concert/events/${eventId}`
const controllers: SessionController[] = []
beforeEach(() => vi.stubGlobal('location', { origin: 'https://mcr.example' }))
afterEach(() => { controllers.splice(0).forEach(c => c.dispose()); vi.unstubAllGlobals() })
function setup(handler: (request: Request) => Response | Promise<Response> = () => Response.json({ ok: true })) {
  const fetch = vi.fn<typeof globalThis.fetch>(async input => handler(input as Request))
  const auth = createSessionController({ eventId, fetch: withAdmission(fetch), cookiePolicy: isolatedCookiePolicy() })
  controllers.push(auth)
  auth.session.replace(0, { userId, eventId, accessToken: 'memory-token' })
  return { auth, fetch }
}
// Explicit undefined is used only to probe missing caller fields at the runtime boundary.
type RequestPatch = { [K in keyof HttpRequest<unknown>]?: HttpRequest<unknown>[K] | undefined }
const read = (patch: RequestPatch = {}): HttpRequest<unknown> => ({
  path: `${prefix}/parties`, method: 'GET', access: 'protected', errorFamily: 'domain',
  domainOperation: 'listConcertParties', decode: value => value, ...patch,
}) as HttpRequest<unknown>
const apply = (patch: RequestPatch = {}): HttpRequest<unknown> => read({
  path: `${prefix}/parties/${party}/applications`, method: 'POST', domainOperation: 'applyConcertParty',
  idempotencyKey: key, body: { json: { expected_revision: 1 } }, ...patch,
})
it.each([
  ['listConcertParties', 'parties', 'CLOSED'], ['listConcertGiveaways', 'giveaways', 'PAUSED'],
] as const)('dispatches registered %s state through canonical FE09 with privacy controls', async (operation, route, state) => {
  const { auth, fetch } = setup(request => {
    const url = new URL(request.url)
    expect(url.searchParams.get('state')).toBe(state)
    expect(url.searchParams.get('limit')).toBe('20')
    expect(request.headers.get('Authorization')).toBe('Bearer memory-token')
    expect(request.headers.has('X-MCR-Intent')).toBe(false)
    expect(request.credentials).toBe('same-origin'); expect(request.cache).toBe('no-store'); expect(request.redirect).toBe('error')
    return Response.json({ ok: true })
  })
  await expect(auth.request(read({ domainOperation: operation, path: `${prefix}/${route}`, query: { state, limit: 20 } }))).resolves.toMatchObject({ data: { ok: true } })
  expect(fetch).toHaveBeenCalledTimes(1)
})
it.each([
  { query: { state: 'DRAFT' } }, { query: { state: 'PAUSED' } }, { query: { state: 'CLOSED', token: 'secret' } },
  { query: { state: 'CLOSED', unknown: true } }, { query: { limit: 101 } }, { query: { limit: '20' } },
  { query: { cursor: '' } }, { query: { starts_after: '2026-02-30T00:00:00Z' } },
  { path: `${prefix}/giveaways` }, { method: 'POST' }, { access: 'public' },
  { domainOperation: 'constructor' as DomainOperationId }, { domainOperation: undefined },
  { errorFamily: 'core' }, { idempotencyKey: key },
] satisfies RequestPatch[])('rejects an invalid domain read before dispatch: %j', async patch => {
  const { auth, fetch } = setup()
  await expect(auth.request(read(patch))).rejects.toMatchObject({ kind: 'invalid-request', uncertain: false })
  expect(fetch).not.toHaveBeenCalled()
})
it('keeps unqualified and Auth state queries forbidden', async () => {
  const { auth, fetch } = setup()
  await expect(auth.request(read({ domainOperation: undefined, errorFamily: 'core', query: { state: 'CLOSED' } }))).rejects.toMatchObject({ kind: 'invalid-request' })
  const http = createHttpClient({ session: auth.session, fetch })
  await expect(http.request(read({ domainOperation: undefined, errorFamily: 'core', access: 'public', path: '/api/auth/capabilities', query: { state: 'CLOSED' } }))).rejects.toMatchObject({ kind: 'invalid-request' })
  expect(fetch).not.toHaveBeenCalled()
})
it('sends the exact canonical key header and intent for a qualified command', async () => {
  const { auth, fetch } = setup(request => {
    expect(request.headers.get('Idempotency-Key')).toBe(key)
    expect(request.headers.get('X-MCR-Intent')).toBe('browser')
    expect(request.headers.has('X-Idempotency-Key')).toBe(false)
    return Response.json({ ok: true })
  })
  await auth.request(apply()); expect(fetch).toHaveBeenCalledTimes(1)
})
it.each([
  { idempotencyKey: undefined }, { idempotencyKey: 'wrong' },
  { headers: { 'Idempotency-Key': key } }, { headers: { Authorization: 'Bearer injected' } },
  { errorFamily: 'core' }, { domainOperation: 'listConcertParties' as const },
  { path: `${prefix}/parties/not-a-uuid/applications` },
] satisfies RequestPatch[])('rejects unqualified command metadata: %j', async patch => {
  const { auth, fetch } = setup()
  await expect(auth.request(apply(patch))).rejects.toMatchObject({ kind: 'invalid-request' })
  expect(fetch).not.toHaveBeenCalled()
})
it.each([
  [409, { reason: 'DUPLICATE_APPLICATION' }, 'DUPLICATE_APPLICATION'],
  [403, { reason: 'ORIGIN_NOT_ALLOWED' }, 'ORIGIN_NOT_ALLOWED'],
  [429, { reason: 'RATE_LIMITED', retry_after_ms: 500 }, 'RATE_LIMITED'],
  [400, { reason: 'DUPLICATE_APPLICATION' }, null],
  [409, { reason: 'VERSION_CONFLICT', current_revision: 2 }, null],
  [409, { reason: 'DUPLICATE_APPLICATION', private: 'secret' }, null],
  [429, { reason: 'RATE_LIMITED' }, null],
  [429, { reason: 'RATE_LIMITED', retry_after_ms: 3600001 }, null],
] as const)('binds error reason/details to registered HTTP status %s: %j', async (status, details, reason) => {
  const { auth, fetch } = setup(() => Response.json({ code: 'CONFLICT', message: 'Not rendered', details }, { status }))
  const error = await auth.request(apply()).catch(e => e)
  expect(error).toMatchObject({ kind: 'http', status, uncertain: false })
  if (reason) expect(error.server.reason).toBe(reason)
  else expect(error.server).toBeNull()
  expect(error.retryAfterMs).toBe(reason === 'RATE_LIMITED' ? 500 : null)
  expect(fetch).toHaveBeenCalledTimes(1)
})
it('passes 202 to the synchronous decoder and never polls or replays the pending command', async () => {
  const { auth, fetch } = setup(() => Response.json({ state: 'PENDING' }, { status: 202 }))
  const decode = vi.fn((value: unknown, context: { status: number }) => {
    expect(context.status).toBe(202); expect(value).toEqual({ state: 'PENDING' }); return 'pending'
  })
  expect((await auth.request(apply({ decode }))).data).toBe('pending')
  expect(fetch).toHaveBeenCalledTimes(1)
})
it('rejects an unregistered success status as uncertain after command dispatch', async () => {
  const { auth, fetch } = setup(() => Response.json({ state: 'PENDING' }, { status: 201 }))
  await expect(auth.request(apply())).rejects.toMatchObject({ kind: 'invalid-response', uncertain: true })
  expect(fetch).toHaveBeenCalledTimes(1)
})
it('keeps a lost command response uncertain with exactly one dispatch', async () => {
  const { auth, fetch } = setup(() => { throw new Error('lost response') })
  await expect(auth.request(apply())).rejects.toMatchObject({ kind: 'network', uncertain: true })
  expect(fetch).toHaveBeenCalledTimes(1)
})
it('discards late domain command results after a canonical scope change', async () => {
  const pending = deferred<Response>(), { auth, fetch } = setup(() => pending.promise)
  const result = auth.request(apply()).catch(e => e)
  await vi.waitFor(() => expect(fetch).toHaveBeenCalledTimes(1))
  auth.session.replace(auth.session.read().generation, null)
  pending.resolve(Response.json({ ok: true }))
  expect(await result).toMatchObject({ kind: 'session-changed', uncertain: true })
  expect(fetch).toHaveBeenCalledTimes(1)
})
