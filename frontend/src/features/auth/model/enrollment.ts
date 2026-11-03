import type { HttpClient, SessionPort } from '../../../shared/api/http'
import { TransportError } from '../../../shared/api/errors'
import type { createQueryPrivacy } from '../../../shared/query/privacy'
import { decodeEnrollment, decodeResend, enrollmentPaths, validEnrollmentEmail, validEnrollmentPassword, validVerificationCode, type EnrollmentAcknowledgements } from './enrollment-contract'
export type EnrollmentKind = 'created' | 'verified' | 'received' | 'invalid-input' | 'invalid-code' | 'not-created' | 'rate-limited'
  | 'unavailable' | 'uncertain' | 'cancelled' | 'busy' | 'policy-unavailable'
export interface EnrollmentResult { kind: EnrollmentKind; retryAt?: number }
interface Options {
  session: SessionPort; http: HttpClient; privacy: ReturnType<typeof createQueryPrivacy>
  canStart(): boolean; available(): boolean
  /** Synthetic qualification only. No approved eligibility or consent-persistence contract exists. */
  syntheticQualification: boolean
}
/** Commands share the canonical session/HTTP/privacy owners; no identity or credential store. */
export function createEnrollment(options: Options) {
  let active: AbortController | null = null, disposed = false, retryAt = 0
  async function run(kind: keyof typeof enrollmentPaths, input: { email: string; password?: string; code?: string }, signal?: AbortSignal): Promise<EnrollmentResult> {
    if (active || !options.canStart()) return { kind: 'busy' }
    if (disposed || !options.available() || options.session.read().userId) return { kind: 'unavailable' }
    if (signal?.aborted) return { kind: 'cancelled' }
    if (Date.now() < retryAt) return { kind: 'rate-limited', retryAt }
    if (!validEnrollmentEmail(input.email) || kind === 'signup' && !validEnrollmentPassword(input.password ?? '')
      || kind === 'verify' && !validVerificationCode(input.code ?? '')) return { kind: 'invalid-input' }
    const controller = new AbortController(), expected = options.session.read()
    active = controller
    const abort = () => controller.abort()
    signal?.addEventListener('abort', abort, { once: true })
    const unregister = options.privacy.registerCleanup(abort)
    const current = () => !disposed && !controller.signal.aborted && options.session.isCurrent(expected)
    try {
      if (!current()) return { kind: 'cancelled' }
      if (kind === 'resend') {
        await options.http.request({ path: enrollmentPaths[kind], method: 'POST', access: 'public', errorFamily: 'auth', body: { json: input }, decode: decodeResend, signal: controller.signal })
        return { kind: current() ? 'received' : 'cancelled' }
      }
      const response = await options.http.request({ path: enrollmentPaths[kind], method: 'POST', access: 'public', errorFamily: 'auth', body: { json: input }, decode: decodeEnrollment, signal: controller.signal })
      if (!current()) return { kind: 'cancelled' }
      return { kind: response.data.status === (kind === 'signup' ? 'PENDING_VERIFICATION' : 'VERIFIED') ? kind === 'signup' ? 'created' : 'verified' : 'uncertain' }
    } catch (error) {
      if (!current()) return { kind: 'cancelled' }
      if (error instanceof TransportError) {
        if ((error.status === 429 || error.status === 503) && error.retryAfterMs !== null) retryAt = Date.now() + error.retryAfterMs
        if (error.status === 429) return { kind: 'rate-limited', ...(retryAt > Date.now() ? { retryAt } : {}) }
        if (kind === 'signup' && error.status === 409) return { kind: 'not-created' }
        if (error.uncertain) return { kind: 'uncertain', ...(retryAt > Date.now() ? { retryAt } : {}) }
        if (kind === 'verify' && [400, 401, 404, 410, 422].includes(error.status ?? 0)) return { kind: 'invalid-code' }
      }
      return { kind: 'unavailable' }
    } finally {
      signal?.removeEventListener('abort', abort); unregister()
      if (active === controller) active = null
    }
  }
  return {
    get pending() { return active !== null },
    get syntheticQualification() { return options.syntheticQualification },
    available: options.available,
    retryAt: () => retryAt,
    signup(email: string, password: string, acknowledgements: EnrollmentAcknowledgements, signal?: AbortSignal): Promise<EnrollmentResult> {
      if (!options.syntheticQualification) return Promise.resolve({ kind: 'policy-unavailable' })
      if (!acknowledgements.eligibility || !acknowledgements.consent) return Promise.resolve({ kind: 'invalid-input' })
      return run('signup', { email, password }, signal)
    },
    verify: (email: string, code: string, signal?: AbortSignal) => run('verify', { email, code }, signal),
    resend: (email: string, signal?: AbortSignal) => run('resend', { email }, signal),
    cancel() { active?.abort() },
    dispose() { disposed = true; active?.abort() },
  }
}
