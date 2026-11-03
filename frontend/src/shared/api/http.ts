import ky from 'ky'
import { domainOperation, matchesDomainRequest } from './domain'
import type { DomainOperationId } from './domain-operations.generated'
import { parseRetryAfter, parseServerError, TransportError, type ErrorFamily, type JsonValue } from './errors'

export interface SessionScope {
  readonly generation: number
  readonly userId: string | null
  readonly eventId: string | null
}
export interface SessionSnapshot extends SessionScope {
  readonly accessToken: string | null
  readonly tokenVersion: number
  readonly signal: AbortSignal
}
export interface SessionCredentials {
  readonly userId: string
  readonly eventId: string | null
  readonly accessToken: string
}
export interface RefreshCredentials {
  readonly userId: string
  readonly accessToken: string
}
export interface SessionPort {
  read(): SessionSnapshot
  isCurrent(scope: SessionScope): boolean
  /** Login, logout, event/account changes and revocation start a new generation atomically. */
  replace(expectedGeneration: number, credentials: SessionCredentials | null): SessionScope | null
  /** A refresh cannot change canonical identity, event, or overwrite a newer token. */
  commitRefresh(expected: SessionSnapshot, credentials: RefreshCredentials): boolean
}
const uuid = (value: unknown): value is string => typeof value === 'string'
  && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(value)
const token = (value: unknown): value is string => typeof value === 'string' && value.length > 0
  && value.length <= 8192 && /^[A-Za-z0-9._~+/-]+=*$/.test(value)
const scopeOf = ({ generation, userId, eventId }: SessionScope): SessionScope => Object.freeze({ generation, userId, eventId })
const matches = (a: SessionScope, b: SessionScope) => a.generation === b.generation
  && a.userId === b.userId && a.eventId === b.eventId

/** Memory only. FE09 owns cookie journeys, private-cache eviction and cross-tab coordination. */
export function createMemorySession(): SessionPort {
  let controller = new AbortController()
  let state: SessionSnapshot = Object.freeze({ generation: 0, userId: null, eventId: null,
    accessToken: null, tokenVersion: 0, signal: controller.signal })
  return {
    read: () => state,
    isCurrent: scope => matches(state, scope),
    replace(expectedGeneration, credentials) {
      if (state.generation !== expectedGeneration) return null
      if (credentials && (!uuid(credentials.userId) || !token(credentials.accessToken)
        || (credentials.eventId !== null && !uuid(credentials.eventId)))) throw new TransportError('invalid-request')
      const previous = controller
      controller = new AbortController()
      state = Object.freeze({ generation: state.generation + 1, tokenVersion: 0, signal: controller.signal,
        userId: credentials?.userId ?? null, eventId: credentials?.eventId ?? null, accessToken: credentials?.accessToken ?? null })
      const installed = scopeOf(state)
      // Publish the new generation before abort listeners run (listeners may re-enter).
      previous.abort()
      return installed
    },
    commitRefresh(expected, credentials) {
      if (!matches(state, expected) || state.tokenVersion !== expected.tokenVersion
        || credentials.userId !== state.userId || !uuid(credentials.userId) || !token(credentials.accessToken)) return false
      state = Object.freeze({ ...state, accessToken: credentials.accessToken, tokenVersion: state.tokenVersion + 1 })
      return true
    },
  }
}

export interface DecodeContext {
  readonly scope: SessionScope
  readonly status: number
  readonly path: string
}
/** Synchronous, side-effect-free schema decoder. Throw to reject invalid or cross-context data. */
export type Decoder<T> = (value: unknown, context: DecodeContext) => T
export interface HttpResult<T> {
  readonly data: T
  readonly scope: SessionScope
}
export interface HttpRequest<T> {
  readonly path: string
  readonly method: 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE'
  readonly access: 'public' | 'protected'
  readonly errorFamily: ErrorFamily
  /** Explicit candidate C04 registry binding, required for the domain error family. */
  readonly domainOperation?: DomainOperationId
  /** Only registered idempotent commands accept this canonical UUID; never generated or replayed here. */
  readonly idempotencyKey?: string
  readonly decode: Decoder<T>
  /** Explicit frozen-contract permission; never applies to any /api/auth operation. */
  readonly refresh?: 'once'
  readonly query?: Readonly<Record<string, string | number | boolean>>
  readonly body?: { readonly json: JsonValue } | { readonly form: FormData }
  /** Only bounded X-Correlation-Id, X-Request-Id, X-Client-Id are accepted. */
  readonly headers?: HeadersInit
  readonly signal?: AbortSignal
  readonly timeoutMs?: number
}
export interface HttpClientOptions {
  readonly session: SessionPort
  /** Test/embedding seam; use the native fetch in production. No arbitrary Ky options. */
  readonly fetch?: typeof globalThis.fetch
  readonly timeoutMs?: number
  /** Opt-in only after FE09 qualifies its refresh/cross-tab policy. */
  readonly refresh?: { readonly decode: Decoder<RefreshCredentials> }
}
export interface HttpClient {
  request<T>(request: HttpRequest<T>): Promise<HttpResult<T>>
  /** The only refresh entry: shared with read recovery, never retried after uncertain rotation. */
  refreshSession(signal?: AbortSignal): Promise<HttpResult<RefreshCredentials>>
}

interface RefreshFlight { generation: number; tokenVersion: number; promise: Promise<HttpResult<RefreshCredentials>> }
const refreshFlights = new WeakMap<SessionPort, RefreshFlight>()
const publicAuthCommands = new Set([
  '/api/auth/email/signup', '/api/auth/email/verify', '/api/auth/email/resend', '/api/auth/login',
  '/api/auth/password/reset/request', '/api/auth/password/reset/confirm', '/api/auth/refresh', '/api/auth/logout',
])

function prepare<T>(request: HttpRequest<T>, snapshot: SessionSnapshot) {
  const { path, method, access } = request
  // Plain route segments only: no normalization, encoded traversal, query, fragment or old prefix.
  if (!/^\/api\/[A-Za-z0-9_-]+(?:\/[A-Za-z0-9_-]+)*$/.test(path)
    || path.split('/').includes('v1') || !['GET', 'POST', 'PUT', 'PATCH', 'DELETE'].includes(method)
    || !['public', 'protected'].includes(access) || !['auth', 'core', 'domain'].includes(request.errorFamily)
    || typeof request.decode !== 'function') throw new TransportError('invalid-request')
  if (path.startsWith('/api/auth/')) {
    const allowed = path === '/api/auth/capabilities' ? method === 'GET' && access === 'public'
      : publicAuthCommands.has(path) ? method === 'POST' && access === 'public'
        : path === '/api/auth/account' ? ['GET', 'DELETE'].includes(method) && access === 'protected'
          : path === '/api/auth/account/deletion' && method === 'GET' && access === 'protected'
    if (!allowed || request.errorFamily !== (path === '/api/auth/capabilities' ? 'core' : 'auth') || request.refresh) {
      throw new TransportError('invalid-request')
    }
  }
  if (request.refresh && (request.refresh !== 'once' || method !== 'GET' || access !== 'protected')) throw new TransportError('invalid-request')
  if (access === 'protected' && (!snapshot.userId || !snapshot.accessToken)) throw new TransportError('authentication-required')
  const event = /^\/api\/concert\/events\/([^/]+)(?:\/|$)/.exec(path)?.[1]
  if (event && (access !== 'protected' || !uuid(event) || event !== snapshot.eventId)) throw new TransportError('invalid-request')
  const operation = domainOperation(request.domainOperation)
  if (request.errorFamily === 'domain') {
    if (!operation || access !== 'protected' || !matchesDomainRequest(operation, method, path, request.query)
      || (operation.idempotency ? !uuid(request.idempotencyKey) : request.idempotencyKey !== undefined)) {
      throw new TransportError('invalid-request')
    }
  } else if (request.domainOperation !== undefined || request.idempotencyKey !== undefined) throw new TransportError('invalid-request')
  const origin = globalThis.location?.origin
  if (!origin || !/^https?:\/\//.test(origin)) throw new TransportError('invalid-request')
  const url = new URL(path, origin)
  for (const [key, value] of Object.entries(request.query ?? {})) {
    if ((/(?:password|token|secret|authorization|^code$|^state$)/i.test(key) && !(operation && key === 'state'))
      || !['string', 'number', 'boolean'].includes(typeof value)
      || (typeof value === 'number' && !Number.isFinite(value))) throw new TransportError('invalid-request')
    url.searchParams.append(key, String(value))
  }
  const headers = new Headers(request.headers)
  for (const [name, value] of headers) {
    if (!['x-correlation-id', 'x-request-id', 'x-client-id'].includes(name)
      || value.length > 128 || !/^[\x20-\x7e]+$/.test(value)) throw new TransportError('invalid-request')
  }
  headers.set('Accept', 'application/json')
  if (operation?.idempotency) headers.set('Idempotency-Key', request.idempotencyKey!)
  if (method !== 'GET') headers.set('X-MCR-Intent', 'browser')
  let body: string | FormData | undefined
  if (request.body) {
    if (method === 'GET') throw new TransportError('invalid-request')
    if ('json' in request.body && !('form' in request.body)) {
      body = JSON.stringify(request.body.json)
      if (body === undefined) throw new TransportError('invalid-request')
      headers.set('Content-Type', 'application/json')
    } else if ('form' in request.body && !('json' in request.body) && request.body.form instanceof FormData) body = request.body.form
    else throw new TransportError('invalid-request')
  }
  return { url, headers, body, operation }
}

function deadline(milliseconds: number, sessionSignal: AbortSignal, callerSignal?: AbortSignal) {
  const controller = new AbortController()
  let kind: 'timeout' | 'aborted' | 'session-changed' = 'aborted'
  const abort = (next: typeof kind) => {
    if (!controller.signal.aborted) { kind = next; controller.abort() }
  }
  const changed = () => abort('session-changed')
  const cancelled = () => abort('aborted')
  sessionSignal.addEventListener('abort', changed, { once: true })
  callerSignal?.addEventListener('abort', cancelled, { once: true })
  if (sessionSignal.aborted) changed()
  else if (callerSignal?.aborted) cancelled()
  const expiresAt = performance.now() + milliseconds
  const timer = setTimeout(() => abort('timeout'), milliseconds)
  return { signal: controller.signal, kind: () => kind,
    checkDeadline() { if (performance.now() >= expiresAt) abort('timeout') },
    close() { clearTimeout(timer); sessionSignal.removeEventListener('abort', changed); callerSignal?.removeEventListener('abort', cancelled) } }
}

/** Abort promptly even when an injected fetch/body stream ignores its signal. */
function cancellable<T>(promise: PromiseLike<T>, signal: AbortSignal): Promise<T> {
  return new Promise((resolve, reject) => {
    const abort = () => reject(new TransportError('aborted'))
    signal.addEventListener('abort', abort, { once: true })
    Promise.resolve(promise).then(resolve, reject).finally(() => signal.removeEventListener('abort', abort))
    if (signal.aborted) abort()
  })
}

async function readJson(response: Response, signal: AbortSignal): Promise<unknown> {
  if (response.status === 204) return undefined
  if (!/^application\/(?:[\w.-]+\+)?json(?:\s*;|$)/i.test(response.headers.get('Content-Type') ?? '')) {
    void response.body?.cancel().catch(() => undefined)
    throw new TransportError('invalid-response')
  }
  // Bounded collection also protects non-JSON/proxy and pathological error payloads.
  const reader = response.body?.getReader()
  if (!reader) throw new TransportError('invalid-response')
  const chunks: Uint8Array[] = []
  let length = 0
  try {
    for (;;) {
      const { done, value } = await cancellable(reader.read(), signal)
      if (done) break
      length += value.length
      if (length > 4 * 1024 * 1024) throw new TransportError('invalid-response')
      chunks.push(value)
    }
    const bytes = new Uint8Array(length)
    let offset = 0
    for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.length }
    try { return JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes)) as unknown }
    catch { throw new TransportError('invalid-response') }
  } finally { void reader.cancel().catch(() => undefined); reader.releaseLock() }
}

export function createHttpClient(options: HttpClientOptions): HttpClient {
  const { session } = options
  const timeoutMs = options.timeoutMs ?? 15_000
  const validTimeout = (value: number) => Number.isInteger(value) && value >= 1 && value <= 120_000
  if (!validTimeout(timeoutMs)) throw new TransportError('invalid-request')

  async function run<T>(request: HttpRequest<T>, expected = session.read()): Promise<HttpResult<T>> {
    const mutation = request.method !== 'GET'
    let prepared: ReturnType<typeof prepare<T>>
    try { prepared = prepare(request, expected) } catch (error) {
      throw error instanceof TransportError ? error : new TransportError('invalid-request')
    }
    const duration = request.timeoutMs ?? timeoutMs
    if (!validTimeout(duration)) throw new TransportError('invalid-request')
    const lifetime = deadline(duration, expected.signal, request.signal)
    let dispatched = false
    let activeResponse: Response | undefined
    const assertCurrent = () => {
      lifetime.checkDeadline()
      if (!session.isCurrent(expected)) throw new TransportError('session-changed')
      if (lifetime.signal.aborted) throw new TransportError(lifetime.kind())
    }
    const attempt = async (snapshot: SessionSnapshot): Promise<Response> => {
      assertCurrent()
      const headers = new Headers(prepared.headers)
      if (request.access === 'protected') headers.set('Authorization', `Bearer ${snapshot.accessToken}`)
      dispatched = true
      const pending = ky(prepared.url, {
        method: request.method, headers, ...(prepared.body === undefined ? {} : { body: prepared.body }),
        credentials: 'same-origin', cache: 'no-store', redirect: 'error', referrerPolicy: 'no-referrer',
        retry: 0, throwHttpErrors: false, timeout: false, signal: lifetime.signal,
        ...(options.fetch ? { fetch: options.fetch } : {}),
      }).then(response => {
        // A non-cooperative injected fetch can finish after cancellation. Dispose its stream.
        if (lifetime.signal.aborted || !session.isCurrent(expected)) {
          void response.body?.cancel().catch(() => undefined)
        }
        return response
      })
      activeResponse = await cancellable(pending, lifetime.signal)
      return activeResponse
    }
    try {
      let response = await attempt(expected)
      assertCurrent()
      if (response.status === 401 && request.refresh === 'once' && options.refresh) {
        void response.body?.cancel().catch(() => undefined)
        await cancellable(refresh(expected), lifetime.signal)
        assertCurrent()
        response = await attempt(session.read()) // exactly one replay; no recursive request/hook
        assertCurrent()
      }
      if (response.redirected || response.type === 'opaqueredirect') throw new TransportError('invalid-response')
      let value: unknown
      try { value = await readJson(response, lifetime.signal) } catch (error) {
        assertCurrent()
        if (response.ok) throw error
        // HTML, malformed JSON and oversized errors retain their HTTP status, never raw content.
      }
      assertCurrent()
      if (!response.ok) {
        let server = parseServerError(value, request.errorFamily, response.status)
        if (prepared.operation && (!server?.reason || !prepared.operation.errors[String(response.status)]?.includes(server.reason))) server = null
        const details = request.errorFamily === 'domain' ? server?.envelope.details : server?.core
        const hint = details && 'retry_after_ms' in details && typeof details.retry_after_ms === 'number' ? details.retry_after_ms : null
        throw new TransportError('http', { status: response.status, server,
          retryAfterMs: parseRetryAfter(response.headers.get('Retry-After')) ?? hint,
          uncertain: mutation && (response.status >= 500 || response.status === 408 || (request.errorFamily === 'auth' && response.status === 409)) })
      }
      let data: T
      try {
        if (prepared.operation && !prepared.operation.success.includes(response.status)) throw new TransportError('invalid-response')
        data = request.decode(value, { scope: scopeOf(expected), status: response.status, path: request.path })
        if (data && (typeof data === 'object' || typeof data === 'function') && 'then' in data) {
          void Promise.resolve(data).catch(() => undefined)
          throw new TransportError('invalid-response')
        }
      } catch { throw new TransportError('invalid-response') }
      assertCurrent()
      return Object.freeze({ data, scope: scopeOf(expected) })
    } catch (error) {
      const kind = !session.isCurrent(expected) ? 'session-changed'
        : lifetime.signal.aborted ? lifetime.kind() : error instanceof TransportError ? error.kind : 'network'
      if (error instanceof TransportError && error.kind === kind
        && (kind === 'http' || !mutation || !dispatched || error.uncertain)) throw error
      throw new TransportError(kind, { uncertain: mutation && dispatched })
    } finally {
      lifetime.close()
      if (activeResponse?.body && !activeResponse.body.locked) void activeResponse.body.cancel().catch(() => undefined)
    }
  }

  async function refresh(expected: SessionSnapshot): Promise<HttpResult<RefreshCredentials>> {
    if (!options.refresh) throw new TransportError('invalid-request')
    if (!session.isCurrent(expected)) throw new TransportError('session-changed')
    const current = session.read()
    if (current.tokenVersion !== expected.tokenVersion && current.userId && current.accessToken) {
      return { data: { userId: current.userId, accessToken: current.accessToken }, scope: scopeOf(current) }
    }
    let flight = refreshFlights.get(session)
    if (!flight || flight.generation !== expected.generation || flight.tokenVersion !== expected.tokenVersion) {
      const promise = (async () => {
        const result = await run({ path: '/api/auth/refresh', method: 'POST', access: 'public', errorFamily: 'auth',
          decode: options.refresh!.decode }, expected)
        if (!session.isCurrent(expected)) throw new TransportError('session-changed')
        if (result.data === null || typeof result.data !== 'object' || !uuid(result.data.userId) || (expected.userId !== null && result.data.userId !== expected.userId)
          || !token(result.data.accessToken)) {
          throw new TransportError('invalid-response', { uncertain: true })
        }
        if (expected.userId !== null && !session.commitRefresh(expected, result.data)) {
          if (!session.isCurrent(expected)) throw new TransportError('session-changed', { uncertain: true })
          const winner = session.read()
          if (winner.tokenVersion === expected.tokenVersion || !winner.userId || !winner.accessToken) {
            throw new TransportError('session-changed', { uncertain: true })
          }
          return { data: { userId: winner.userId, accessToken: winner.accessToken }, scope: scopeOf(winner) }
        }
        return result
      })()
      flight = { generation: expected.generation, tokenVersion: expected.tokenVersion, promise }
      // Keep failures latched for this token version; a later 401 must not retry uncertain rotation.
      refreshFlights.set(session, flight)
    }
    return flight.promise
  }
  return {
    request: request => request.path === '/api/auth/refresh'
      ? Promise.reject(new TransportError('invalid-request')) : run(request),
    async refreshSession(signal) {
      if (signal?.aborted) throw new TransportError('aborted')
      const expected = session.read()
      try {
        const result = await cancellable(refresh(expected), signal ?? expected.signal)
        if (!session.isCurrent(expected)) throw new TransportError('session-changed')
        if (signal?.aborted) throw new TransportError('aborted')
        return result
      } catch (error) {
        if (!session.isCurrent(expected)) throw new TransportError('session-changed', { uncertain: true })
        if (signal?.aborted) throw new TransportError('aborted', { uncertain: true })
        throw error
      }
    },
  }
}
