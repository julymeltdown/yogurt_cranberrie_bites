import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import authSuccess from '../../../../../contracts/fixtures/core/auth-success.json'
import authUnauthenticated from '../../../../../contracts/fixtures/core/auth-error-unauthenticated.json'
import authLimited from '../../../../../contracts/fixtures/core/auth-error-rate-limited.json'
import conflict from '../../../../../contracts/fixtures/core/error-version-conflict.json'
import originDenied from '../../../../../contracts/fixtures/core/error-origin-not-allowed.json'
import { createHttpClient, createMemorySession, type Decoder, type HttpRequest, type RefreshCredentials } from '../http'
import { TransportError } from '../errors'

const USER = authSuccess.userId
const OTHER = '00000000-0000-4000-8000-000000000102'
const EVENT = '00000000-0000-4000-8000-000000000201'
const OTHER_EVENT = '00000000-0000-4000-8000-000000000202'
const PATH = `/api/concert/events/${EVENT}/me`
const json = (data: unknown = { ok: true }, status = 200, headers?: HeadersInit) => Response.json(data, { status, ...(headers ? { headers } : {}) })
function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (error: unknown) => void
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no })
  return { promise, resolve, reject }
}
const decode: Decoder<boolean> = value => {
  if (!value || typeof value !== 'object' || !('ok' in value) || value.ok !== true) throw new Error('Invalid DTO')
  return true
}
const grant: Decoder<RefreshCredentials> = value => {
  if (!value || typeof value !== 'object' || !('userId' in value) || typeof value.userId !== 'string'
    || !('accessToken' in value) || typeof value.accessToken !== 'string'
    || !('refreshToken' in value) || value.refreshToken !== ''
    || !('tokenType' in value) || value.tokenType !== 'Bearer'
    || !('expiresIn' in value) || typeof value.expiresIn !== 'number' || value.expiresIn <= 0) throw new Error('Invalid AuthResponse')
  return { userId: value.userId, accessToken: value.accessToken }
}
const read = (extra: Partial<HttpRequest<boolean>> = {}): HttpRequest<boolean> => ({
  path: PATH, method: 'GET', access: 'protected', errorFamily: 'core', decode, ...extra,
})
function setup(handler: (request: Request) => Response | Promise<Response>, anonymous = false) {
  const session = createMemorySession()
  if (!anonymous) session.replace(0, { userId: USER, eventId: EVENT, accessToken: 'old-token' })
  const fetch = vi.fn<typeof globalThis.fetch>(async input => handler(input as Request))
  const client = createHttpClient({ session, fetch, refresh: { decode: grant } })
  return { session, fetch, client }
}
const errorOf = async (promise: Promise<unknown>): Promise<TransportError> => {
  try { await promise; throw new Error('Expected rejection') }
  catch (error) { expect(error).toBeInstanceOf(TransportError); return error as TransportError }
}
beforeEach(() => vi.stubGlobal('location', { origin: 'https://mcr.example' }))
afterEach(() => { vi.unstubAllGlobals(); vi.useRealTimers(); vi.restoreAllMocks() })

describe('session scope and stale response protection', () => {
  it('uses atomic generation replacement and token CAS without browser storage', () => {
    const storage = { getItem: vi.fn(), setItem: vi.fn() }
    vi.stubGlobal('localStorage', storage); vi.stubGlobal('sessionStorage', storage)
    const session = createMemorySession()
    const initial = session.read()
    expect(session.replace(0, { userId: USER, eventId: EVENT, accessToken: 'one' })).toMatchObject({ generation: 1, userId: USER, eventId: EVENT })
    expect(initial.signal.aborted).toBe(true)
    expect(session.replace(0, null)).toBeNull()
    const first = session.read()
    expect(session.commitRefresh(first, { userId: OTHER, accessToken: 'wrong' })).toBe(false)
    expect(session.commitRefresh(first, { userId: USER, accessToken: 'two' })).toBe(true)
    expect(session.commitRefresh(first, { userId: USER, accessToken: 'stale' })).toBe(false)
    expect(session.read()).toMatchObject({ generation: 1, tokenVersion: 1, accessToken: 'two' })
    expect(first.signal.aborted).toBe(false)
    session.replace(1, null)
    expect(first.signal.aborted).toBe(true)
    expect(session.read()).toMatchObject({ generation: 2, userId: null, accessToken: null, eventId: null })
    expect(storage.getItem).not.toHaveBeenCalled(); expect(storage.setItem).not.toHaveBeenCalled()
  })

  it.each(['account', 'event', 'logout'] as const)('discards delayed private data after %s change', async change => {
    const pending = deferred<Response>()
    const { client, session, fetch } = setup(() => pending.promise)
    const result = errorOf(client.request(read()))
    await vi.waitFor(() => expect(fetch).toHaveBeenCalledTimes(1))
    session.replace(1, change === 'logout' ? null : { userId: change === 'account' ? OTHER : USER,
      eventId: change === 'event' ? OTHER_EVENT : EVENT, accessToken: 'new-token' })
    expect(await result).toMatchObject({ kind: 'session-changed' })
    pending.resolve(json())
    await new Promise(resolve => setTimeout(resolve, 0))
    expect(session.read().generation).toBe(2)
    expect(session.read().userId).toBe(change === 'logout' ? null : change === 'account' ? OTHER : USER)
  })

  it.each(['/api/auth/login', '/api/auth/logout'])('never installs late %s results', async path => {
    const pending = deferred<Response>()
    const { client, session, fetch } = setup(() => pending.promise, true)
    const result = errorOf(client.request({ path, method: 'POST', access: 'public', errorFamily: 'auth', decode: grant }))
    await vi.waitFor(() => expect(fetch).toHaveBeenCalledTimes(1))
    session.replace(0, null)
    pending.resolve(json(authSuccess))
    expect(await result).toMatchObject({ kind: 'session-changed', uncertain: true })
    expect(session.read().accessToken).toBeNull()
  })

  it('returns a scope for consumer CAS and never installs login credentials itself', async () => {
    const { client, session } = setup(() => json(authSuccess), true)
    const result = await client.request({ path: '/api/auth/login', method: 'POST', access: 'public', errorFamily: 'auth', decode: grant })
    expect(session.read().accessToken).toBeNull()
    session.replace(0, null)
    expect(session.replace(result.scope.generation, { ...result.data, eventId: EVENT })).toBeNull()
  })

  it('rejects cross-context DTOs through the explicit decoder', async () => {
    const { client } = setup(() => json({ user_id: OTHER, event_id: OTHER_EVENT }))
    expect(await errorOf(client.request(read({ decode: (value, context) => {
      const dto = value as { user_id: string; event_id: string }
      if (dto.user_id !== context.scope.userId || dto.event_id !== context.scope.eventId) throw new Error('Context mismatch')
      return true
    } })))).toMatchObject({ kind: 'invalid-response' })
  })
})

describe('explicit single-flight refresh for protected reads', () => {
  it('shares one refresh across two clients, parallel/staggered 401s and explicit resume', async () => {
    const rotation = deferred<Response>(); const late401 = deferred<Response>()
    let oldReads = 0
    const { client, session, fetch } = setup(request => {
      if (request.url.endsWith('/auth/refresh')) return rotation.promise
      if (request.headers.get('Authorization') === 'Bearer old-token') {
        oldReads++
        return oldReads === 3 ? late401.promise : json(authUnauthenticated, 401)
      }
      expect(request.headers.get('Authorization')).toBe(`Bearer ${authSuccess.accessToken}`)
      return json()
    })
    const second = createHttpClient({ session, fetch, refresh: { decode: grant } })
    const requests = [client.request(read({ refresh: 'once' })), second.request(read({ refresh: 'once' })), client.request(read({ refresh: 'once' }))]
    await vi.waitFor(() => expect(fetch).toHaveBeenCalledTimes(4))
    const resume = second.refreshSession()
    rotation.resolve(json(authSuccess))
    await resume
    late401.resolve(json(authUnauthenticated, 401))
    expect(await Promise.all(requests)).toEqual(Array.from({ length: 3 }, () => ({ data: true, scope: { generation: 1, userId: USER, eventId: EVENT } })))
    expect(fetch).toHaveBeenCalledTimes(7)
    expect(fetch.mock.calls.map(([input]) => input as Request).filter(r => r.url.endsWith('/auth/refresh'))).toHaveLength(1)
    expect(session.read()).toMatchObject({ accessToken: authSuccess.accessToken, tokenVersion: 1, generation: 1 })
  })

  it('requires explicit permission on reads even when refresh is configured', async () => {
    const { client, fetch } = setup(() => json(authUnauthenticated, 401))
    expect(await errorOf(client.request(read()))).toMatchObject({ status: 401 })
    expect(fetch).toHaveBeenCalledTimes(1)
  })

  it('does not turn opt-in read recovery into implicit refresh configuration', async () => {
    const { session, fetch } = setup(() => json(authUnauthenticated, 401))
    const client = createHttpClient({ session, fetch })
    expect(await errorOf(client.request(read({ refresh: 'once' })))).toMatchObject({ status: 401 })
    expect(await errorOf(client.refreshSession())).toMatchObject({ kind: 'invalid-request' })
    expect(fetch).toHaveBeenCalledTimes(1)
  })

  it('never refreshes or replays a second 401', async () => {
    const { client, fetch } = setup(request => request.url.endsWith('/auth/refresh') ? json(authSuccess) : json(authUnauthenticated, 401))
    expect(await errorOf(client.request(read({ refresh: 'once' })))).toMatchObject({ kind: 'http', status: 401 })
    expect(fetch).toHaveBeenCalledTimes(3)
  })

  it.each([401, 409, 429, 503])('latches failed rotation HTTP %s for the token version', async status => {
    const { client, fetch } = setup(request => json(authUnauthenticated, request.url.endsWith('/auth/refresh') ? status : 401))
    expect(await errorOf(client.request(read({ refresh: 'once' })))).toMatchObject({ kind: 'http', status })
    expect(await errorOf(client.request(read({ refresh: 'once' })))).toMatchObject({ kind: 'http', status })
    expect(await errorOf(client.refreshSession())).toMatchObject({ status })
    expect(fetch.mock.calls.map(([r]) => r as Request).filter(r => r.url.endsWith('/auth/refresh'))).toHaveLength(1)
  })

  it('latches network-failed refresh without retry and marks uncertain rotation', async () => {
    const { client, fetch } = setup(request => {
      if (request.url.endsWith('/auth/refresh')) throw new TypeError('secret diagnostic')
      return json(authUnauthenticated, 401)
    })
    const error = await errorOf(client.request(read({ refresh: 'once' })))
    expect(error).toMatchObject({ kind: 'network', uncertain: true })
    expect(JSON.stringify(error)).not.toContain('secret diagnostic')
    await errorOf(client.refreshSession())
    expect(fetch).toHaveBeenCalledTimes(2)
  })

  it('cancels one waiter without cancelling the shared rotation or another read', async () => {
    const rotation = deferred<Response>()
    const { client, session, fetch } = setup(request => request.url.endsWith('/auth/refresh') ? rotation.promise
      : request.headers.get('Authorization') === 'Bearer old-token' ? json(authUnauthenticated, 401) : json())
    const controller = new AbortController()
    const cancelled = errorOf(client.request(read({ refresh: 'once', signal: controller.signal })))
    const remaining = client.request(read({ refresh: 'once' }))
    await vi.waitFor(() => expect(fetch).toHaveBeenCalledTimes(3))
    controller.abort()
    expect(await cancelled).toMatchObject({ kind: 'aborted' })
    rotation.resolve(json(authSuccess))
    expect(await remaining).toMatchObject({ data: true })
    expect(session.read().accessToken).toBe(authSuccess.accessToken)
    expect(fetch).toHaveBeenCalledTimes(4)
  })

  it.each([false, true])('late refresh cannot restore identity after logout (anonymous=%s)', async anonymous => {
    const rotation = deferred<Response>()
    const { client, session, fetch } = setup(() => rotation.promise, anonymous)
    const result = errorOf(client.refreshSession())
    await vi.waitFor(() => expect(fetch).toHaveBeenCalledTimes(1))
    session.replace(session.read().generation, null)
    rotation.resolve(json(authSuccess))
    expect(await result).toMatchObject({ kind: 'session-changed', uncertain: true })
    expect(session.read().accessToken).toBeNull()
  })

  it.each(['account', 'event'] as const)('rejects refresh after an active %s switch', async change => {
    const rotation = deferred<Response>()
    const { client, session, fetch } = setup(() => rotation.promise)
    const result = errorOf(client.refreshSession())
    await vi.waitFor(() => expect(fetch).toHaveBeenCalledTimes(1))
    session.replace(1, { userId: change === 'account' ? OTHER : USER,
      eventId: change === 'event' ? OTHER_EVENT : EVENT, accessToken: 'replacement-token' })
    rotation.resolve(json(authSuccess))
    expect(await result).toMatchObject({ kind: 'session-changed', uncertain: true })
    expect(session.read().accessToken).toBe('replacement-token')
  })

  it('anonymous resume returns an uninstalled candidate; cancellation cannot install it', async () => {
    const rotation = deferred<Response>()
    const { client, session, fetch } = setup(() => rotation.promise, true)
    const abort = new AbortController()
    const cancelled = errorOf(client.refreshSession(abort.signal))
    await vi.waitFor(() => expect(fetch).toHaveBeenCalledTimes(1))
    abort.abort()
    expect(await cancelled).toMatchObject({ kind: 'aborted', uncertain: true })
    rotation.resolve(json(authSuccess))
    const candidate = await client.refreshSession()
    expect(session.read().accessToken).toBeNull()
    expect(candidate.scope.generation).toBe(0)
    expect(session.replace(candidate.scope.generation, { ...candidate.data, eventId: EVENT })).not.toBeNull()
    expect(session.read().userId).toBe(USER)
  })

  it('does not overwrite a newer token if a refresh CAS loses', async () => {
    const rotation = deferred<Response>()
    const { client, session, fetch } = setup(() => rotation.promise)
    const expected = session.read()
    const result = client.refreshSession()
    await vi.waitFor(() => expect(fetch).toHaveBeenCalledTimes(1))
    session.commitRefresh(expected, { userId: USER, accessToken: 'winner-token' })
    rotation.resolve(json(authSuccess))
    expect(await result).toMatchObject({ data: { accessToken: 'winner-token' } })
    expect(session.read().accessToken).toBe('winner-token')
  })

  it.each([null, { ...authSuccess, userId: OTHER }, { ...authSuccess, accessToken: 'bad token' }, { ...authSuccess, refreshToken: 'leaked' }])('rejects malformed/cross-user refresh grants %#', async value => {
    const { client, session } = setup(() => json(value))
    expect(await errorOf(client.refreshSession())).toMatchObject({ kind: 'invalid-response', uncertain: true })
    expect(session.read().accessToken).toBe('old-token')
  })

  it('normalizes a misbehaving null decoder without retaining a TypeError', async () => {
    const { session, fetch } = setup(() => json())
    const client = createHttpClient({ session, fetch, refresh: { decode: () => null as unknown as RefreshCredentials } })
    expect(await errorOf(client.refreshSession())).toMatchObject({ kind: 'invalid-response', uncertain: true })
  })
})

describe('auth/public/mutation requests never automatically replay', () => {
  it.each([
    ['/api/auth/capabilities', 'GET', 'public', 'core'],
    ['/api/auth/login', 'POST', 'public', 'auth'],
    ['/api/auth/email/signup', 'POST', 'public', 'auth'],
    ['/api/auth/password/reset/request', 'POST', 'public', 'auth'],
    ['/api/auth/account', 'GET', 'protected', 'auth'],
    ['/api/auth/account/deletion', 'GET', 'protected', 'auth'],
  ] as const)('%s sends once without refresh', async (path, method, access, errorFamily) => {
    const { client, fetch } = setup(() => json(authUnauthenticated, 401))
    expect(await errorOf(client.request(read({ path, method, access, errorFamily })))).toMatchObject({ status: 401 })
    expect(fetch).toHaveBeenCalledTimes(1)
  })

  it.each([401, 403, 409, 429, 503])('never replays mutation HTTP %s; returns bounded errors', async status => {
    const value = status === 403 ? originDenied : status === 409 ? conflict : status === 429 ? authLimited : authUnauthenticated
    const { client, fetch } = setup(() => json(value, status, { 'Retry-After': '2' }))
    expect(await errorOf(client.request(read({ method: 'PATCH', body: { json: { expected_revision: 1 } } })))).toMatchObject({
      kind: 'http', status, retryAfterMs: 2000, uncertain: status >= 500,
    })
    expect(fetch).toHaveBeenCalledTimes(1)
  })

  it.each([408, 409, 500])('auth mutation %s distinguishes uncertainty', async status => {
    const { client, fetch } = setup(() => json(authUnauthenticated, status))
    expect(await errorOf(client.request(read({ path: '/api/auth/login', method: 'POST', access: 'public', errorFamily: 'auth' })))).toMatchObject({ uncertain: true })
    expect(fetch).toHaveBeenCalledTimes(1)
  })

  it.each(['GET', 'POST'] as const)('network failure is bounded and not retried (%s)', async method => {
    const { client, fetch } = setup(() => { throw new TypeError('contains private secret') })
    const error = await errorOf(client.request(read({ method })))
    expect(error).toMatchObject({ kind: 'network', uncertain: method === 'POST', status: null })
    expect(error.message).not.toContain('secret'); expect(error).not.toHaveProperty('cause')
    expect(fetch).toHaveBeenCalledTimes(1)
  })
})

describe('cancellation, deadlines and response streams', () => {
  it('pre-aborted requests do not dispatch', async () => {
    const { client, fetch } = setup(() => json())
    const controller = new AbortController(); controller.abort()
    expect(await errorOf(client.request(read({ method: 'POST', signal: controller.signal })))).toMatchObject({ kind: 'aborted', uncertain: false })
    expect(fetch).not.toHaveBeenCalled()
    expect(await errorOf(client.refreshSession(controller.signal))).toMatchObject({ kind: 'aborted', uncertain: false })
    expect(fetch).not.toHaveBeenCalled()
  })

  it('cancels non-cooperative fetch, reports uncertainty, and disposes its late stream', async () => {
    const pending = deferred<Response>(); const cancel = vi.fn()
    const { client, fetch } = setup(() => pending.promise)
    const controller = new AbortController()
    const result = errorOf(client.request(read({ method: 'POST', signal: controller.signal })))
    await vi.waitFor(() => expect(fetch).toHaveBeenCalledTimes(1))
    controller.abort()
    expect(await result).toMatchObject({ kind: 'aborted', uncertain: true })
    pending.resolve(new Response(new ReadableStream({ cancel }), { headers: { 'Content-Type': 'application/json' } }))
    await vi.waitFor(() => expect(cancel).toHaveBeenCalledTimes(1))
    expect(fetch).toHaveBeenCalledTimes(1)
  })

  it.each(['fetch', 'body'] as const)('deadline covers stalled %s and never retries mutations', async stalled => {
    vi.useFakeTimers()
    const cancel = vi.fn()
    const { client, fetch } = setup(() => stalled === 'fetch' ? new Promise<Response>(() => undefined)
      : new Response(new ReadableStream({ cancel }), { headers: { 'Content-Type': 'application/json' } }))
    const result = errorOf(client.request(read({ method: 'POST', timeoutMs: 50 })))
    await vi.advanceTimersByTimeAsync(51)
    expect(await result).toMatchObject({ kind: 'timeout', uncertain: true })
    expect(fetch).toHaveBeenCalledTimes(1)
    if (stalled === 'body') expect(cancel).toHaveBeenCalledTimes(1)
  })

  it('aborts a stalled error body without losing the cancellation signal', async () => {
    const cancel = vi.fn()
    const { client, fetch } = setup(() => new Response(new ReadableStream({ cancel }), {
      status: 503, headers: { 'Content-Type': 'application/json' },
    }))
    const controller = new AbortController()
    const result = errorOf(client.request(read({ signal: controller.signal })))
    await vi.waitFor(() => expect(fetch).toHaveBeenCalledTimes(1))
    controller.abort()
    expect(await result).toMatchObject({ kind: 'aborted' })
    expect(cancel).toHaveBeenCalledTimes(1)
  })

  it('uses safe core cooldown details as a hint and preserves typed 403/409 failures', async () => {
    const responses = [json(originDenied, 403), json(conflict, 409),
      json({ code: 'LIMIT', message: 'Wait.', details: { reason: 'RATE_LIMITED', retry_after_ms: 1200 } }, 429)]
    const { client } = setup(() => responses.shift()!)
    expect(await errorOf(client.request(read()))).toMatchObject({ server: { reason: 'ORIGIN_NOT_ALLOWED' } })
    expect(await errorOf(client.request(read()))).toMatchObject({ server: { core: { reason: 'VERSION_CONFLICT' } } })
    expect(await errorOf(client.request(read()))).toMatchObject({ retryAfterMs: 1200, server: { reason: 'RATE_LIMITED' } })
  })

  it('rejects redirected/opaque responses without inspecting their payloads', async () => {
    const response = json()
    Object.defineProperty(response, 'redirected', { value: true })
    const { client } = setup(() => response)
    expect(await errorOf(client.request(read()))).toMatchObject({ kind: 'invalid-response' })
  })

  it('detects an elapsed deadline after synchronous decoding before the timer can fire', async () => {
    let elapsed = 0
    vi.spyOn(performance, 'now').mockImplementation(() => elapsed)
    const { client } = setup(() => json())
    expect(await errorOf(client.request(read({ timeoutMs: 100, decode: () => { elapsed = 101; return true } })))).toMatchObject({ kind: 'timeout' })
  })

  it('rejects ambiguous JSON plus multipart bodies before dispatch', async () => {
    const { client, fetch } = setup(() => json())
    expect(await errorOf(client.request(read({ method: 'POST', body: { json: { value: 1 }, form: new FormData() } })))).toMatchObject({ kind: 'invalid-request', uncertain: false })
    expect(fetch).not.toHaveBeenCalled()
  })

  it('checks scope again after a decoder changes generation', async () => {
    const { client, session } = setup(() => json())
    expect(await errorOf(client.request(read({ decode: () => { session.replace(1, null); return true } })))).toMatchObject({ kind: 'session-changed' })
  })

  it.each([
    () => new Response('<html>private proxy content</html>', { status: 502 }),
    () => new Response('{broken', { status: 502, headers: { 'Content-Type': 'application/json' } }),
  ])('preserves non-JSON/malformed HTTP status without raw content %#', async response => {
    const { client } = setup(response)
    const error = await errorOf(client.request(read()))
    expect(error).toMatchObject({ kind: 'http', status: 502, server: null })
    expect(JSON.stringify(error)).not.toContain('private proxy')
  })

  it.each([
    () => new Response('ok'),
    () => new Response('{broken', { headers: { 'Content-Type': 'application/json' } }),
    () => new Response(new Uint8Array([0xff]), { headers: { 'Content-Type': 'application/json' } }),
    () => json({ unexpected: true }),
    () => new Response('"' + 'x'.repeat(4 * 1024 * 1024) + '"', { headers: { 'Content-Type': 'application/json' } }),
  ])('rejects invalid/oversized success payload %#', async response => {
    const { client } = setup(response)
    expect(await errorOf(client.request(read()))).toMatchObject({ kind: 'invalid-response' })
  })

  it('supports explicit 204 decoding and rejects asynchronous decoders', async () => {
    const { client } = setup(() => new Response(null, { status: 204 }))
    expect(await client.request(read({ decode: value => value === undefined }))).toMatchObject({ data: true })
    expect(await errorOf(client.request(read({ decode: (() => Promise.resolve(true)) as unknown as Decoder<boolean> })))).toMatchObject({ kind: 'invalid-response' })
    expect(await errorOf(client.request(read({ decode: (() => Promise.reject(new Error('Rejected decoder'))) as unknown as Decoder<boolean> })))).toMatchObject({ kind: 'invalid-response' })
  })
})

describe('same-origin URL and header boundaries', () => {
  it.each(['https://evil.example/api/test', '//evil.example/api/test', '/v1/auth/login', '/api/v1/test', '/api/../auth/login',
    '/api/%2e%2e/auth', '/api/a%2fb', '/api/a\\b', '/api/a?x=1', '/api/a#token', '/api//a', '/api/a/'])('rejects %s before fetch', async path => {
    const { client, fetch } = setup(() => json())
    expect(await errorOf(client.request(read({ path })))).toMatchObject({ kind: 'invalid-request' })
    expect(fetch).not.toHaveBeenCalled()
  })

  it.each(['Authorization', 'Cookie', 'Origin', 'Host', 'Content-Type', 'Accept', 'X-MCR-Intent', 'X-Forwarded-For', 'X-CSRF-Token'])('rejects caller-owned %s header', async header => {
    const { client, fetch } = setup(() => json())
    expect(await errorOf(client.request(read({ headers: { [header]: 'override' } })))).toMatchObject({ kind: 'invalid-request' })
    expect(fetch).not.toHaveBeenCalled()
  })

  it.each([
    { path: '/api/auth/oauth/google', method: 'GET', access: 'public', errorFamily: 'auth' },
    { path: '/api/auth/email/availability', access: 'public', errorFamily: 'auth' },
    { path: '/api/auth/login', method: 'POST', access: 'public', errorFamily: 'auth', refresh: 'once' },
    { path: '/api/auth/account', errorFamily: 'auth', refresh: 'once' },
    { path: '/api/auth/refresh', method: 'POST', access: 'public', errorFamily: 'auth' },
    { method: 'PATCH', refresh: 'once' }, { access: 'public', refresh: 'once' },
    { path: `/api/concert/events/${OTHER_EVENT}/me` },
    { query: { access_token: 'secret' } }, { query: { state: 'secret' } },
    { query: { page: Infinity } }, { headers: { 'X-Request-Id': 'x'.repeat(129) } },
    { body: { json: { invalid: 'GET body' } } }, { timeoutMs: 0 },
  ] satisfies Partial<HttpRequest<boolean>>[])('rejects invalid request before dispatch %#', async patch => {
    const { client, fetch } = setup(() => json())
    expect(await errorOf(client.request(read(patch)))).toMatchObject({ kind: 'invalid-request' })
    expect(fetch).not.toHaveBeenCalled()
  })

  it('requires a canonical active identity for protected requests', async () => {
    const { client, fetch } = setup(() => json(), true)
    expect(await errorOf(client.request(read()))).toMatchObject({ kind: 'authentication-required' })
    expect(fetch).not.toHaveBeenCalled()
  })

  it('sets exact safe Request properties and JSON/browser intent on mutation', async () => {
    let body: unknown
    const { client, fetch } = setup(async request => { body = await request.json(); return json() })
    await client.request(read({ method: 'PATCH', query: { page: 2, text: 'hello & world' }, headers: { 'X-Request-Id': 'request-1' }, body: { json: { expected_revision: 1 } } }))
    const request = fetch.mock.calls[0]![0] as Request
    expect(request.url).toBe(`https://mcr.example${PATH}?page=2&text=hello+%26+world`)
    expect(request).toMatchObject({ method: 'PATCH', credentials: 'same-origin', cache: 'no-store', redirect: 'error', referrerPolicy: 'no-referrer' })
    expect(request.headers.get('Authorization')).toBe('Bearer old-token')
    expect(request.headers.get('X-MCR-Intent')).toBe('browser')
    expect(request.headers.get('Content-Type')).toBe('application/json')
    expect(request.headers.get('X-Request-Id')).toBe('request-1')
    expect(body).toEqual({ expected_revision: 1 })
  })

  it('does not expose bearer tokens on public commands/capabilities or send GET intent', async () => {
    const { client, fetch } = setup(() => json())
    await client.request(read({ path: '/api/auth/capabilities', access: 'public' }))
    await client.request(read({ path: '/api/auth/logout', method: 'POST', access: 'public', errorFamily: 'auth' }))
    const requests = fetch.mock.calls.map(([input]) => input as Request)
    expect(requests[0]!.headers.has('Authorization')).toBe(false)
    expect(requests[0]!.headers.has('X-MCR-Intent')).toBe(false)
    expect(requests[1]!.headers.has('Authorization')).toBe(false)
    expect(requests[1]!.headers.get('X-MCR-Intent')).toBe('browser')
  })

  it('lets native FormData construct the multipart boundary and transmits the file', async () => {
    let received: FormData | undefined
    const { client, fetch } = setup(async request => { received = await request.formData(); return json() })
    const form = new FormData(); form.append('image', new Blob(['pixels'], { type: 'image/png' }), 'avatar.png')
    await client.request(read({ method: 'POST', body: { form } }))
    const request = fetch.mock.calls[0]![0] as Request
    expect(request.headers.get('Content-Type')).toMatch(/^multipart\/form-data; boundary=/)
    expect(request.headers.get('X-MCR-Intent')).toBe('browser')
    const uploaded = received!.get('image') as File
    expect(uploaded.name).toBe('avatar.png'); expect(await uploaded.text()).toBe('pixels')
  })
})
