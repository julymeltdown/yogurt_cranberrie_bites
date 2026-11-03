import { QueryClient } from '@tanstack/react-query'
import { createHttpClient, createMemorySession, type HttpRequest, type SessionScope } from '../../../shared/api/http'
import { TransportError } from '../../../shared/api/errors'
import { createQueryPrivacy } from '../../../shared/query/privacy'
import { ShellLoadError, type ActiveScope, type SessionReadiness } from '../../../app/ports'
import { canonicalUuid, decodeAuth, decodeCapabilities, decodeContract, validLogin } from './contract'
import { decodeBootstrap } from './bootstrap'
import { createBrowserCookiePolicy, type CookiePolicy } from './cookie-policy'
import { createRecovery } from './recovery'
import { createEnrollment } from './enrollment'

type Notice = 'invalid-input' | 'credentials' | 'rate-limited' | 'unavailable' | 'expired' | 'changed' | null
export interface AuthState {
  readonly readiness: SessionReadiness
  readonly pending: 'login' | 'logout' | null
  readonly capabilities: 'loading' | 'ready' | 'error'
  readonly emailEnabled: boolean
  readonly notice: Notice
  readonly logout: 'none' | 'confirmed' | 'unconfirmed'
}
export interface SessionOptions {
  eventId: string | null
  fetch?: typeof fetch
  cookiePolicy?: CookiePolicy
  /** Test/qualification seam only. Production composition never enables it. */
  qualifiedRefresh?: boolean
  timeoutMs?: number
  /** Test-only synthetic policy qualification; never authorizes production consent. */
  syntheticEnrollment?: boolean
}
export function createSessionController(options: SessionOptions) {
  const eventId = canonicalUuid(options.eventId) ? options.eventId : null
  const session = createMemorySession(), queryClient = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } })
  const privacy = createQueryPrivacy({ client: queryClient, session })
  const cookiePolicy = options.cookiePolicy ?? createBrowserCookiePolicy()
  const http = createHttpClient({ session, refresh: { decode: decodeAuth },
    ...(options.fetch ? { fetch: options.fetch } : {}), ...(options.timeoutMs ? { timeoutMs: options.timeoutMs } : {}) })
  let state: AuthState = Object.freeze({ readiness: eventId ? 'anonymous' : 'unavailable', pending: null,
    capabilities: 'loading', emailEnabled: false, notice: null, logout: 'none' })
  let started = false, disposed = false, intent = 0, expiresAt = 0, enrollmentEmailAvailable = false
  let expiry: ReturnType<typeof setTimeout> | undefined
  let refreshFlight: Promise<void> | undefined
  let capabilitiesFlight: Promise<void> | undefined
  const listeners = new Set<() => void>()
  const publish = (patch: Partial<AuthState>) => {
    if (disposed) return
    state = Object.freeze({ ...state, ...patch }); const snapshot = [...listeners]; snapshot.forEach(listener => listener())
  }
  function clear(notice: Notice = null) {
    ++intent; clearTimeout(expiry); expiresAt = 0; refreshFlight = undefined
    session.replace(session.read().generation, null)
    publish({ readiness: eventId ? 'anonymous' : 'unavailable', pending: null, notice })
  }
  const recovery = createRecovery({ session, http, privacy, cookiePolicy,
    canStart: () => !disposed && state.pending === null && !enrollment.pending, beforeConfirm: () => clear() })
  const enrollment = createEnrollment({ session, http, privacy, syntheticQualification: options.syntheticEnrollment === true,
    canStart: () => !disposed && state.pending === null && !recovery.pending && privacy.capture().gate === 'open', available: () => enrollmentEmailAvailable })
  const stopPeer = cookiePolicy.onInvalidate(() => {
    const interruptedLogout = state.pending === 'logout'
    clear('changed')
    publish({ ...(interruptedLogout ? { logout: 'unconfirmed' } : {}), ...(!cookiePolicy.available ? { emailEnabled: false } : {}) })
  })
  function expireIn(seconds: number) {
    expiresAt = Math.min(Number.MAX_SAFE_INTEGER, Date.now() + seconds * 1000)
    clearTimeout(expiry)
    const tick = () => {
      const remaining = expiresAt - Date.now()
      if (remaining <= 0) clear('expired')
      else expiry = setTimeout(tick, Math.min(remaining, 2_147_483_647))
    }
    tick()
  }
  const current = (attempt: number, scope: SessionScope) => !disposed && intent === attempt && session.isCurrent(scope)
  function notice(error: unknown): Notice {
    if (error instanceof TransportError) {
      if (error.status === 429) return 'rate-limited'
      if (error.status === 401 || error.status === 403) return 'credentials'
    }
    return 'unavailable'
  }
  async function capabilities(forEnrollment = false) {
    if (disposed) return
    if (capabilitiesFlight) return capabilitiesFlight
    if (!eventId && !forEnrollment) { publish({ capabilities: 'error' }); return }
    // Reserve before notifying observers: public enrollment discovery and FE09 share one flight.
    const flight = Promise.resolve().then(async () => {
      if (disposed) return
      try {
        const result = await http.request({ path: '/api/auth/capabilities', method: 'GET', access: 'public', errorFamily: 'core', decode: decodeCapabilities })
        if (!disposed) {
          enrollmentEmailAvailable = result.data.email
          publish({ capabilities: 'ready', emailEnabled: Boolean(eventId && result.data.email && cookiePolicy.available) })
        }
      } catch { publish({ capabilities: 'error', emailEnabled: false }) }
    })
    capabilitiesFlight = flight; enrollmentEmailAvailable = false
    publish({ capabilities: 'loading', emailEnabled: false })
    try { await flight } finally { if (capabilitiesFlight === flight) capabilitiesFlight = undefined }
  }
  async function refresh(): Promise<void> {
    if (!options.qualifiedRefresh || disposed || !session.read().userId) throw new TransportError('authentication-required')
    if (refreshFlight) return refreshFlight
    const expected = session.read(), attempt = intent
    const flight = cookiePolicy.run('refresh', async () => {
      if (!current(attempt, expected)) throw new TransportError('session-changed')
      const result = await http.refreshSession()
      if (!current(attempt, expected)) throw new TransportError('session-changed', { uncertain: true })
      // FE01 preserves decoder fields on the returned result while committing only its two credentials.
      const auth = result.data as ReturnType<typeof decodeAuth>
      if (!Number.isSafeInteger(auth.expiresIn) || auth.expiresIn < 1) throw new TransportError('invalid-response', { uncertain: true })
      expireIn(auth.expiresIn)
    })
    refreshFlight = flight
    try { await flight } catch (error) { if (current(attempt, expected)) clear('expired'); throw error }
    finally { if (refreshFlight === flight) refreshFlight = undefined }
  }
  async function request<T>(input: Omit<HttpRequest<T>, 'refresh'>) {
    // Features cannot opt into FE01's uncoordinated refresh path through this composition.
    if ('refresh' in input || input.path.startsWith('/api/auth/')) throw new TransportError('invalid-request')
    const expected = session.read()
    const execute = async () => {
      const result = await http.request(input)
      if (!session.isCurrent(expected)) throw new TransportError('session-changed')
      return result
    }
    try {
      try { return await execute() }
      catch (error) {
        if (error instanceof TransportError && error.status === 401 && input.method === 'GET'
          && input.access === 'protected' && session.isCurrent(expected) && options.qualifiedRefresh) {
          if (session.read().tokenVersion === expected.tokenVersion) await refresh()
          if (!session.isCurrent(expected)) throw new TransportError('session-changed')
          return await execute() // one authorized read replay; mutations never enter here
        }
        throw error
      }
    } catch (error) {
      if (input.access === 'protected' && session.isCurrent(expected)
        && error instanceof TransportError && error.status === 401) clear('expired')
      throw error
    }
  }
  return {
    session, queryClient, privacy, request, enrollment,
    checkEnrollmentAvailability: () => capabilities(true),
    requestPasswordReset: recovery.request,
    async confirmPasswordReset(token: string, newPassword: string, signal?: AbortSignal) {
      const result = await recovery.confirm(token, newPassword, signal)
      if (!cookiePolicy.available) publish({ emailEnabled: false })
      return result
    },
    cancelRecovery: recovery.cancel,
    read: () => state,
    subscribe(listener: () => void) { listeners.add(listener); return () => { listeners.delete(listener) } },
    readiness: { read: () => state.readiness, subscribe(listener: () => void) { listeners.add(listener); return () => { listeners.delete(listener) } } },
    start() { if (!started && !disposed) { started = true; void capabilities() } },
    retryCapabilities() { if (state.capabilities !== 'loading') void capabilities() },
    async login(email: string, password: string): Promise<boolean> {
      if (disposed || state.pending || recovery.pending || enrollment.pending || !eventId || !state.emailEnabled) return false
      if (!validLogin(email, password)) { publish({ notice: 'invalid-input', logout: 'none' }); return false }
      clear(); const attempt = intent
      publish({ pending: 'login', logout: 'none' })
      try {
        return await cookiePolicy.run('login', async () => {
          const expected = session.read()
          if (!current(attempt, expected)) return false
          const result = await http.request({ path: '/api/auth/login', method: 'POST', access: 'public', errorFamily: 'auth',
            body: { json: { email, password } }, decode: decodeAuth })
          if (!current(attempt, expected)) return false
          const installed = session.replace(expected.generation, { userId: result.data.userId, eventId, accessToken: result.data.accessToken })
          if (!installed || !session.isCurrent(installed)) return false
          expireIn(result.data.expiresIn)
          publish({ readiness: 'ready', pending: null, notice: null })
          return true
        })
      } catch (error) {
        if (intent === attempt) { clear(notice(error)); publish({ pending: null, emailEnabled: state.emailEnabled && cookiePolicy.available }) }
        return false
      } finally { if (intent === attempt) publish({ pending: null }) }
    },
    async logout(): Promise<boolean> {
      if (disposed || state.pending === 'logout') return false
      // Local revocation is immediate, including pending login/refresh, before waiting on a lock/network.
      clear(); const attempt = intent
      publish({ pending: 'logout', logout: 'none' })
      try {
        await cookiePolicy.run('logout', async () => {
          if (disposed || intent !== attempt) throw new TransportError('session-changed')
          await http.request({ path: '/api/auth/logout', method: 'POST', access: 'public', errorFamily: 'auth',
            decode: (value, context) => { if (context.status !== 204 || value !== undefined) throw new Error('Invalid logout'); return undefined } })
        })
        if (disposed || intent !== attempt) return false
        publish({ pending: null, logout: 'confirmed' }); return true
      } catch {
        if (intent === attempt) publish({ pending: null, logout: 'unconfirmed', emailEnabled: state.emailEnabled && cookiePolicy.available })
        return false
      }
    },
    refresh,
    async loadBootstrap(scope: ActiveScope, signal: AbortSignal) {
      try {
        if (!session.isCurrent(scope)) throw new TransportError('session-changed')
        const response = await request({ path: `/api/concert/events/${scope.eventId}/bootstrap`, method: 'GET', access: 'protected', errorFamily: 'core', signal,
          decode: value => decodeContract('Bootstrap', value) })
        const result = await decodeBootstrap(response.data, scope)
        if (signal.aborted || !session.isCurrent(scope)) throw new TransportError('session-changed')
        return result
      } catch (error) {
        if (signal.aborted || !session.isCurrent(scope)) throw new ShellLoadError('unauthenticated')
        if (!(error instanceof TransportError)) { clear('unavailable'); throw new ShellLoadError('unavailable') }
        if (error.kind === 'invalid-response') clear('unavailable')
        const reason = error.status === 401 ? 'unauthenticated' : error.status === 403 ? 'denied' : error.status === 404 ? 'not-found'
          : error.status === 410 ? 'gone' : error.status === 429 ? 'rate-limited' : 'unavailable'
        throw new ShellLoadError(reason)
      }
    },
    dispose() {
      if (disposed) return
      enrollment.dispose(); recovery.dispose(); clear(); privacy.dispose(); queryClient.clear(); stopPeer(); cookiePolicy.dispose(); disposed = true; listeners.clear()
    },
  }
}
export type SessionController = ReturnType<typeof createSessionController>
