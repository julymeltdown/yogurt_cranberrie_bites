import type { HttpClient, SessionPort } from '../../../shared/api/http'
import { TransportError } from '../../../shared/api/errors'
import type { createQueryPrivacy } from '../../../shared/query/privacy'
import type { CookiePolicy } from './cookie-policy'
import { decodeResetResponse, resetPaths, validRecoveryEmail, validResetPassword, validResetToken } from './recovery-contract'

export type RecoveryResult = 'received' | 'confirmed' | 'invalid-input' | 'invalid-token' | 'rate-limited'
  | 'unavailable' | 'uncertain' | 'cancelled' | 'busy'
interface RecoveryOptions {
  session: SessionPort
  http: HttpClient
  privacy: ReturnType<typeof createQueryPrivacy>
  cookiePolicy: CookiePolicy
  canStart(): boolean
  beforeConfirm(): void
}
/** Uses the canonical controller's existing session, transport and privacy owner.
 * No credentials, response bodies or operation inputs are retained in its state.
 */
export function createRecovery(options: RecoveryOptions) {
  let active: AbortController | null = null, disposed = false
  async function run(kind: 'request' | 'confirm', input: { email: string } | { token: string; newPassword: string }, signal?: AbortSignal): Promise<RecoveryResult> {
    if (active) return 'busy'
    if (disposed || !options.canStart()) return 'unavailable'
    if (signal?.aborted) return 'cancelled'
    if ('email' in input ? !validRecoveryEmail(input.email) : !validResetPassword(input.newPassword)) return 'invalid-input'
    if ('token' in input && !validResetToken(input.token)) return 'invalid-token'
    if (kind === 'confirm' && !options.cookiePolicy.available) return 'unavailable'
    const controller = new AbortController()
    active = controller
    let expected = options.session.read(), unregister = () => undefined as void
    const abort = () => controller.abort()
    signal?.addEventListener('abort', abort, { once: true })
    const current = () => !disposed && options.session.isCurrent(expected)
    try {
      if (kind === 'confirm') {
        // Reserve the operation before notifying observers, preventing reentrant login.
        // Revoke local authority, then capture the new anonymous request scope.
        options.beforeConfirm()
        expected = options.session.read()
      }
      if (!options.canStart() || disposed || signal?.aborted || controller.signal.aborted) return 'cancelled'
      unregister = options.privacy.registerCleanup(abort)
      const execute = async () => {
        if (!current() || controller.signal.aborted) throw new TransportError('aborted')
        await options.http.request({ path: resetPaths[kind], method: 'POST', access: 'public', errorFamily: 'auth',
          body: { json: input }, decode: decodeResetResponse, signal: controller.signal })
        if (!current() || controller.signal.aborted) throw new TransportError('session-changed')
      }
      if (kind === 'confirm') await options.cookiePolicy.run('recovery', execute, controller.signal)
      else await execute()
      if (!current() || controller.signal.aborted) return 'cancelled'
      return kind === 'request' ? 'received' : 'confirmed'
    } catch (error) {
      if (!current() || controller.signal.aborted) return 'cancelled'
      if (error instanceof TransportError) {
        if (error.uncertain) return 'uncertain'
        if (error.status === 429) return 'rate-limited'
        if (kind === 'confirm' && [400, 401, 404, 422].includes(error.status ?? 0)) return 'invalid-token'
      }
      return 'unavailable'
    } finally {
      signal?.removeEventListener('abort', abort); unregister()
      if (active === controller) active = null
    }
  }
  return {
    get pending() { return active !== null },
    request: (email: string, signal?: AbortSignal) => run('request', { email }, signal),
    confirm: (token: string, newPassword: string, signal?: AbortSignal) => run('confirm', { token, newPassword }, signal),
    cancel() { active?.abort() },
    dispose() { disposed = true; active?.abort() },
  }
}
