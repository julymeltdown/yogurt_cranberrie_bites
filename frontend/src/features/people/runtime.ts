import type { SessionController } from '../auth/model/session'
import type { PrivacyScope } from '../../shared/query/privacy'
import type { HttpRequest } from '../../shared/api/http'
import { TransportError } from '../../shared/api/errors'
export interface AccessPort { read(): { available: boolean; readOnly: boolean }; subscribe(listener: () => void): () => void }
export interface SocialOptions { auth: Pick<SessionController, 'privacy' | 'request'>; access: AccessPort; now?: () => number; createId?: () => string }
export type Failure = 'unavailable' | 'invalid' | 'offline' | 'rate-limited' | 'read-only' | 'conflict'
export interface Slot<T> { phase: 'idle' | 'loading' | 'ready' | 'error'; data: T | null; failure: Failure | null }
export const blank = <T>(): Slot<T> => ({ phase: 'idle', data: null, failure: null })
export function failure(error: unknown): Failure {
  if (!(error instanceof TransportError)) return 'invalid'
  if ([401, 403, 404].includes(error.status ?? 0)) return 'unavailable'
  if (error.server?.reason === 'EVENT_READ_ONLY') return 'read-only'
  if (error.status === 409) return 'conflict'
  if (error.status === 429) return 'rate-limited'
  if (error.kind === 'network' || error.kind === 'timeout') return 'offline'
  return 'unavailable'
}
export const messages: Record<Failure, string> = { unavailable: 'This content is unavailable.', invalid: 'The response could not be verified.', offline: 'Connection unavailable. Retry explicitly.', 'rate-limited': 'Too many requests. Wait before trying again.', 'read-only': 'This event is read-only. Your draft is preserved.', conflict: 'This content changed. Your draft is preserved. Load the latest version.' }
/** Newly scoped orchestration following committed FE02/FE27 cancellation discipline. */
export function createSocialRuntime(options: SocialOptions, clear: () => void, changed: () => void) {
  let scope: PrivacyScope | null = null, started = false, disposed = false, epoch = 0
  let access = { available: false, readOnly: true }, removers: (() => void)[] = []
  const flights = new Map<string, AbortController>()
  const usable = () => started && !disposed && scope !== null && !!scope.eventId && options.auth.privacy.isCurrent(scope) && access.available
  const current = (captured: PrivacyScope) => usable() && scope === captured
  function cancel(lane?: string) { if (lane) { flights.get(lane)?.abort(); flights.delete(lane) } else { epoch++; flights.forEach(c => c.abort()); flights.clear() } }
  function sync() {
    if (!started || disposed) return
    const next = options.auth.privacy.capture(), nextAccess = { ...options.access.read() }
    if (scope && JSON.stringify(next) === JSON.stringify(scope)) { access = nextAccess; if (!access.available) cancel(); changed(); return }
    cancel(); scope = next; access = nextAccess; clear(); changed()
  }
  return { scope: () => scope, usable, current, writable: () => usable() && !access.readOnly, readOnly: () => access.readOnly, cancel,
    start() { if (started || disposed) return; started = true; removers = [options.auth.privacy.registerCleanup(() => { cancel(); scope = null; clear() }), options.auth.privacy.subscribe(sync), options.access.subscribe(sync)]; sync() },
    async run<T>(lane: string, build: (scope: PrivacyScope) => Omit<HttpRequest<T>, 'refresh'>): Promise<T> {
      if (!usable() || !scope) throw new TransportError('authentication-required')
      cancel(lane); const captured = scope, version = epoch, flight = new AbortController(); flights.set(lane, flight)
      try {
        const result = await options.auth.privacy.run(captured, ({ signal }) => options.auth.request({ ...build(captured), signal }), flight.signal)
        if (!current(captured) || version !== epoch || flights.get(lane) !== flight) throw new TransportError('session-changed')
        return result.data
      } finally { if (flights.get(lane) === flight) flights.delete(lane) }
    },
    dispose() { if (disposed) return; disposed = true; started = false; cancel(); removers.forEach(remove => remove()); removers = []; scope = null; clear() },
  }
}
export type SocialRuntime = ReturnType<typeof createSocialRuntime>
