import type { SessionController } from '../../auth/model/session'
import type { HttpRequest, HttpResult } from '../../../shared/api/http'
import type { PrivacyScope } from '../../../shared/query/privacy'
import { TransportError } from '../../../shared/api/errors'
export type SocialAuth = Pick<SessionController, 'request' | 'privacy'>
export interface SocialAccess { readonly available: boolean; readonly readOnly: boolean }
export interface AccessPort { read(): SocialAccess; subscribe(listener: () => void): () => void }
/** Must be supplied by the canonical session owner: one attempt, no refresh/replay. */
export type SensitiveRead = <T>(input: Omit<HttpRequest<T>, 'refresh'>) => Promise<HttpResult<T>>
export interface SocialOptions { readonly auth: SocialAuth; readonly access: AccessPort; readonly sensitiveRead?: SensitiveRead; readonly now?: () => number }
export type Failure = 'denied' | 'unavailable' | 'offline' | 'conflict' | 'read-only' | 'rate-limited' | 'invalid' | 'failed'
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
  if (error.kind === 'invalid-response' || error.kind === 'invalid-request') return 'invalid'
  return 'failed'
}
export const messages: Record<Failure, string> = {
  denied: 'You do not have permission for this action.', unavailable: 'Sharing information is no longer available.', offline: 'Connection unavailable. Retry reads explicitly after reconnecting.',
  conflict: 'This information changed. Reload and confirm your choices again.', 'read-only': 'This event is read-only. You can still remove or revoke existing sharing.',
  'rate-limited': 'Too many requests. Wait before trying again.', invalid: 'The response could not be verified. No private information was displayed.', failed: 'The request failed. No success is assumed.',
}
export function createRuntime(options: SocialOptions, clear: () => void, changed: () => void) {
  let started = false, disposed = false, scope: PrivacyScope | null = null, access: SocialAccess = { available: false, readOnly: true }, epoch = 0
  const flights = new Map<string, AbortController>(); let removers: (() => void)[] = []
  function cancel(lane?: string) { if (lane) { flights.get(lane)?.abort(); flights.delete(lane) } else { epoch++; flights.forEach(c => c.abort()); flights.clear() } }
  const usable = () => started && !disposed && !!scope?.eventId && !!scope.userId && options.auth.privacy.isCurrent(scope) && access.available
  function sync() {
    if (!started || disposed) return
    const next = options.auth.privacy.capture(), nextAccess = options.access.read()
    const scopeChanged = JSON.stringify(scope) !== JSON.stringify(next)
    if (!scopeChanged && access.available === nextAccess.available && access.readOnly === nextAccess.readOnly) return
    if (scopeChanged) { cancel(); scope = next; clear() }
    else if (!nextAccess.available) cancel()
    access = { ...nextAccess }; changed()
  }
  return {
    scope: () => scope, access: () => access, usable, cancel,
    start() { if (started || disposed) return; started = true; removers = [options.auth.privacy.registerCleanup(() => { cancel(); scope = null; clear() }), options.auth.privacy.subscribe(sync), options.access.subscribe(sync)]; sync() },
    async run<T>(lane: string, make: (scope: PrivacyScope) => Omit<HttpRequest<T>, 'refresh'>, sensitive = false): Promise<T> {
      if (!usable() || !scope || (sensitive && !options.sensitiveRead)) throw new TransportError('authentication-required')
      cancel(lane); const captured = scope, version = epoch, controller = new AbortController(); flights.set(lane, controller)
      try {
        const result = await options.auth.privacy.run(captured, ({ signal }) => (sensitive ? options.sensitiveRead! : options.auth.request)({ ...make(captured), signal }), controller.signal)
        if (version !== epoch || flights.get(lane) !== controller || !usable()) throw new TransportError('session-changed')
        return result.data
      } finally { if (flights.get(lane) === controller) flights.delete(lane) }
    },
    dispose() { if (disposed) return; disposed = true; started = false; cancel(); removers.forEach(remove => remove()); removers = []; scope = null; clear() },
  }
}
