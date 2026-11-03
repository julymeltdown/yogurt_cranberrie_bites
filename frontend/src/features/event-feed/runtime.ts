import type { SessionController } from '../auth/model/session'
import { TransportError } from '../../shared/api/errors'
import type { HttpRequest } from '../../shared/api/http'
import type { PrivacyScope } from '../../shared/query/privacy'
import { freeze, uuid } from './contract'

export interface FeedAccess { readonly available: boolean; readonly readOnly: boolean }
export interface AccessPort { read(): FeedAccess; subscribe(listener: () => void): () => void }
export type FeedAuth = Pick<SessionController, 'privacy' | 'request'>
export interface FeedOptions { readonly auth: FeedAuth; readonly access: AccessPort }
export type Failure = 'offline' | 'denied' | 'unavailable' | 'conflict' | 'read-only' | 'rate-limited' | 'invalid' | 'failed'
export interface Slot<T> { readonly phase: 'idle' | 'loading' | 'ready' | 'error'; readonly data: T | null; readonly failure: Failure | null }
export const blank = <T>(): Slot<T> => ({ phase: 'idle', data: null, failure: null })
export function failure(error: unknown): Failure {
  if (!(error instanceof TransportError)) return 'invalid'
  if (error.kind === 'network' || error.kind === 'timeout') return 'offline'
  if (error.server?.reason === 'EVENT_READ_ONLY') return 'read-only'
  if (error.status === 401 || error.status === 403) return 'denied'
  if (error.status === 404) return 'unavailable'
  if (error.status === 409) return 'conflict'
  if (error.status === 429) return 'rate-limited'
  if (error.kind === 'invalid-response') return 'invalid'
  return 'failed'
}
export const messages: Record<Failure, string> = {
  offline: 'Connection unavailable. Reconnect and retry explicitly.', denied: 'You do not have access to this content.',
  unavailable: 'This content was deleted, hidden, withdrawn, or is no longer available.', conflict: 'This content changed. Reload before editing again.',
  'read-only': 'This event is read-only.', 'rate-limited': 'Too many requests. Wait before retrying.',
  invalid: 'The response could not be verified. No content was displayed.', failed: 'The request failed. Try again when the service is available.',
}
/** One scoped lifetime; cancellation and private-state eviction happen before observers run. */
export function createFeedRuntime(options: FeedOptions, clear: () => void, changed: () => void) {
  let started = false, disposed = false, epoch = 0
  let scope: PrivacyScope | null = null, access: FeedAccess = { available: false, readOnly: true }
  const flights = new Map<string, AbortController>()
  let removers: (() => void)[] = []
  function cancel(lane?: string) {
    if (lane) { flights.get(lane)?.abort(); flights.delete(lane) }
    else { flights.forEach(c => c.abort()); flights.clear(); epoch++ }
  }
  function usable() { return started && !disposed && scope !== null && options.auth.privacy.isCurrent(scope) && !!scope.eventId && uuid.test(scope.eventId) && access.available }
  function sync() {
    if (!started || disposed) return
    cancel(); scope = options.auth.privacy.capture(); access = freeze({ ...options.access.read() }); clear(); changed()
  }
  return {
    scope: () => scope, access: () => access, usable, writable: () => usable() && !access.readOnly,
    start() {
      if (started || disposed) return
      started = true
      removers = [options.auth.privacy.registerCleanup(() => { cancel(); scope = null; clear() }), options.auth.privacy.subscribe(sync), options.access.subscribe(sync)]
      sync()
    },
    cancel,
    async run<T>(lane: string, request: (scope: PrivacyScope) => Omit<HttpRequest<T>, 'refresh'>): Promise<T> {
      if (!usable() || !scope) throw new TransportError('authentication-required')
      cancel(lane)
      const captured = scope, version = epoch, controller = new AbortController(); flights.set(lane, controller)
      try {
        const result = await options.auth.privacy.run(captured, ({ signal }) => options.auth.request({ ...request(captured), signal }), controller.signal)
        if (!usable() || epoch !== version || flights.get(lane) !== controller) throw new TransportError('session-changed')
        return result.data
      } finally { if (flights.get(lane) === controller) flights.delete(lane) }
    },
    dispose() { if (disposed) return; disposed = true; started = false; cancel(); removers.forEach(remove => remove()); removers = []; scope = null; clear() },
  }
}
