import type { Decoder, HttpClient, SessionPort } from '../../../shared/api/http'
import { TransportError, type JsonValue } from '../../../shared/api/errors'
import type { createQueryPrivacy } from '../../../shared/query/privacy'
import { decodePolicy, decodeAttempt, decodeAccepted, decodeVerified, decodeAdmission, enrollmentPaths, enrollmentUuid,
  exactAcknowledgements, validEnrollmentEmail, validEnrollmentPassword, validVerificationCode,
  type EnrollmentPolicy, type EnrollmentAttempt, type EnrollmentAccepted, type EnrollmentAdmission } from './enrollment-contract'

export type EnrollmentKind = 'ready' | 'created' | 'verified' | 'received' | 'admitted' | 'consent-required'
  | 'invalid-input' | 'invalid-code' | 'rate-limited' | 'unavailable' | 'uncertain' | 'cancelled' | 'busy'
  | 'policy-unavailable' | 'policy-stale' | 'attempt-expired' | 'attempt-invalid' | 'acknowledgements-required'
  | 'idempotency-conflict' | 'account-inactive'
export interface EnrollmentResult { kind: EnrollmentKind }
export interface EnrollmentState {
  readonly policy: EnrollmentPolicy | null
  readonly attempt: EnrollmentAttempt | null
  readonly accepted: EnrollmentAccepted | null
  readonly admission: EnrollmentAdmission | null
  readonly result: EnrollmentResult | null
  readonly pending: boolean
  /** Monotonic deadlines, bounded at request START, never local wall-clock timestamps. */
  readonly policyDeadline: number
  readonly attemptDeadline: number
  readonly resendDeadline: number
  readonly retryDeadline: number
  readonly unknown: 'signup' | 'resend' | 'consent' | null
}
interface Options {
  session: SessionPort; http: HttpClient; privacy: ReturnType<typeof createQueryPrivacy>
  canStart(): boolean; available(): boolean
  /** Compatibility only until Root removes the obsolete fixture option. Never grants consent. */
  syntheticQualification?: boolean
}
type Command = { kind: 'signup' | 'resend' | 'consent'; body: JsonValue; started: number }
const initial = (): EnrollmentState => ({ policy: null, attempt: null, accepted: null, admission: null, result: null,
  pending: false, policyDeadline: 0, attemptDeadline: 0, resendDeadline: 0, retryDeadline: 0, unknown: null })
/** One canonical Auth/HTTP/privacy owner. No browser storage, identity, token or cookie owner. */
export function createEnrollment(options: Options) {
  let state = initial(), active: AbortController | null = null, disposed = false, command: Command | null = null
  const listeners = new Set<() => void>(), now = () => performance.now()
  const publish = (patch: Partial<EnrollmentState>) => {
    state = Object.freeze({ ...state, ...patch }); [...listeners].forEach(listener => listener())
  }
  const finish = (kind: EnrollmentKind) => { const result = { kind }; publish({ result }); return result }
  const deadline = (expiry: string, server: string, started: number) => started + Math.max(0, Date.parse(expiry) - Date.parse(server))
  function clear() { command = null; publish(initial()) }
  const unregister = options.privacy.registerCleanup(() => { active?.abort(); clear() })
  function invalidate(kind: EnrollmentKind) {
    command = null
    if (kind === 'policy-stale' || kind === 'policy-unavailable') publish({ policy: null, attempt: null, accepted: null,
      admission: null, policyDeadline: 0, attemptDeadline: 0, resendDeadline: 0, unknown: null })
    if (kind === 'attempt-expired' || kind === 'attempt-invalid') publish({ attempt: null, accepted: null, attemptDeadline: 0, resendDeadline: 0, unknown: null })
  }
  async function run<T>(key: keyof typeof enrollmentPaths, decode: Decoder<T>, apply: (data: T, started: number) => EnrollmentKind,
    body?: JsonValue, signal?: AbortSignal): Promise<EnrollmentResult> {
    if (active || !options.canStart()) { if (!state.unknown) command = null; return { kind: 'busy' } }
    if (disposed || signal?.aborted) { command = null; return { kind: 'cancelled' } }
    const protectedAction = key === 'admission' || key === 'consent', identity = options.session.read().userId
    if (protectedAction ? !identity : key !== 'policy' && !!identity) { command = null; return finish('unavailable') }
    if (key !== 'policy' && !protectedAction && !options.available()) { command = null; return finish('unavailable') }
    if (state.retryDeadline > now()) { if (!state.unknown) command = null; return finish('rate-limited') }
    const controller = new AbortController(), expected = options.session.read(), started = now()
    active = controller
    const abort = () => controller.abort()
    signal?.addEventListener('abort', abort, { once: true })
    const current = () => !disposed && !controller.signal.aborted && options.session.isCurrent(expected)
    publish({ pending: true, result: null })
    try {
      const response = await options.http.request({ path: enrollmentPaths[key], method: key === 'policy' || key === 'admission' ? 'GET' : 'POST',
        access: protectedAction ? 'protected' : 'public', errorFamily: 'auth', decode,
        ...(body === undefined ? {} : { body: { json: body } }), signal: controller.signal })
      if (!current()) { command = null; return { kind: 'cancelled' } }
      const outcome = apply(response.data, started)
      if (command?.kind === key) { command = null; publish({ unknown: null }) }
      return finish(outcome)
    } catch (error) {
      if (!current()) { command = null; return { kind: 'cancelled' } }
      if (error instanceof TransportError) {
        if (error.retryAfterMs !== null && [429, 503].includes(error.status ?? 0)) publish({ retryDeadline: now() + error.retryAfterMs })
        // Match authoritative reason AND status. Never render server prose or guess from HTTP409.
        const reason = error.server?.envelope.details?.reason
        const reasons: Record<string, [number, EnrollmentKind]> = {
          POLICY_UNAVAILABLE: [503, 'policy-unavailable'], POLICY_STALE: [409, 'policy-stale'],
          ATTEMPT_EXPIRED: [410, 'attempt-expired'], ATTEMPT_INVALID: [404, 'attempt-invalid'],
          ACKNOWLEDGEMENTS_REQUIRED: [422, 'acknowledgements-required'], IDEMPOTENCY_CONFLICT: [409, 'idempotency-conflict'],
          VERIFICATION_FAILED: [422, 'invalid-code'], INVALID_INPUT: [422, 'invalid-input'], ACCOUNT_INACTIVE: [403, 'account-inactive'],
        }
        const known = typeof reason === 'string' ? reasons[reason] : undefined
        if (known && known[0] === error.status) {
          command = null; publish({ unknown: null }); invalidate(known[1]); return finish(known[1])
        }
        if (error.status === 429) { command = null; publish({ unknown: null }); return finish('rate-limited') }
        if (error.uncertain) {
          if (command?.kind === key) publish({ unknown: command.kind })
          return finish('uncertain')
        }
      }
      command = null; publish({ unknown: null }); return finish('unavailable')
    } finally {
      signal?.removeEventListener('abort', abort)
      if (active === controller) { active = null; publish({ pending: false }) }
    }
  }
  function policyReady() {
    if (!state.policy) return false
    if (state.policyDeadline <= now()) { invalidate('policy-unavailable'); return false }
    return true
  }
  function attemptReady(id: string) {
    if (!enrollmentUuid.safeParse(id).success) return 'invalid-input' as const
    if ((state.attempt?.attemptId === id || state.accepted?.attemptId === id) && state.attemptDeadline <= now()) { invalidate('attempt-expired'); return 'attempt-expired' as const }
    return null
  }
  function acceptReceipt(data: EnrollmentAccepted, started: number, id: string) {
    if (data.attemptId !== id || Date.parse(data.expiresAt) <= Date.parse(data.serverTime)) throw new TransportError('invalid-response', { uncertain: true })
    const previous = state.attempt?.attemptId === id || state.accepted?.attemptId === id ? state.attemptDeadline : 0
    const bound = Math.min(previous || Infinity, deadline(data.expiresAt, data.serverTime, started))
    if (bound <= now()) throw new TransportError('invalid-response', { uncertain: true })
    publish({ accepted: data, attemptDeadline: bound,
      resendDeadline: deadline(data.resendAvailableAt, data.serverTime, started) })
    return 'received' as const
  }
  function dispatch(saved: Command, signal?: AbortSignal) {
    const body = saved.body as Readonly<Record<string, JsonValue>>
    if (saved.kind === 'consent') return run('consent', decodeAdmission, data => {
      if (data.decision === 'ADMITTED' && data.policyRevision !== body.policyRevision) throw new TransportError('invalid-response', { uncertain: true })
      publish({ admission: data }); return admissionKind(data)
    }, body, signal)
    const id = body.attemptId as string
    return run(saved.kind, decodeAccepted, (data, started) => {
      acceptReceipt(data, Math.min(started, saved.started), id); return saved.kind === 'signup' ? 'created' : 'received'
    }, body, signal)
  }
  function admissionKind(data: EnrollmentAdmission): EnrollmentKind {
    return ({ ADMITTED: 'admitted', CONSENT_REQUIRED: 'consent-required', POLICY_UNAVAILABLE: 'policy-unavailable', ACCOUNT_INACTIVE: 'account-inactive' } as const)[data.decision]
  }
  const port = {
    read: () => state,
    subscribe(listener: () => void) { listeners.add(listener); return () => { listeners.delete(listener) } },
    get pending() { return active !== null },
    available: options.available,
    loadPolicy: (signal?: AbortSignal) => run('policy', decodePolicy, (data, started) => {
      if (deadline(data.expiresAt, data.serverTime, started) <= now()) { invalidate('policy-unavailable'); return 'policy-unavailable' }
      if (state.policy?.revision !== data.revision) { command = null; publish({ attempt: null, accepted: null, unknown: null, attemptDeadline: 0, resendDeadline: 0 }) }
      publish({ policy: data, policyDeadline: deadline(data.expiresAt, data.serverTime, started) }); return 'ready'
    }, undefined, signal),
    beginAttempt(signal?: AbortSignal) {
      if (active || state.unknown) return Promise.resolve({ kind: 'busy' as const })
      if (!policyReady()) return Promise.resolve(finish('policy-unavailable'))
      const revision = state.policy!.revision
      return run('attempts', decodeAttempt, (data, started) => {
        if (data.policyRevision !== revision || !policyReady()) throw new TransportError('invalid-response', { uncertain: true })
        const expires = Math.min(state.policyDeadline, deadline(data.expiresAt, data.serverTime, started))
        if (expires <= now()) return 'attempt-expired'
        publish({ attempt: data, accepted: null, attemptDeadline: expires, resendDeadline: 0 }); return 'ready'
      }, { policyRevision: revision }, signal)
    },
    signup(email: string, password: string, ids: readonly string[], signal?: AbortSignal) {
      if (active || state.unknown) return Promise.resolve({ kind: 'busy' as const })
      if (!policyReady()) return Promise.resolve(finish('policy-unavailable'))
      if (!validEnrollmentEmail(email) || !validEnrollmentPassword(password)) return Promise.resolve(finish('invalid-input'))
      if (!state.attempt) return Promise.resolve(finish('attempt-invalid'))
      const invalid = attemptReady(state.attempt.attemptId)
      if (invalid) return Promise.resolve(finish(invalid))
      if (!exactAcknowledgements(state.policy!, ids)) return Promise.resolve(finish('acknowledgements-required'))
      if (state.accepted) return Promise.resolve(finish('idempotency-conflict'))
      command = { kind: 'signup', started: now(), body: { attemptId: state.attempt.attemptId, policyRevision: state.policy!.revision,
        acknowledgementIds: [...ids], idempotencyKey: crypto.randomUUID(), email, password } }
      return dispatch(command, signal)
    },
    verify(id: string, code: string, signal?: AbortSignal) {
      if (active) return Promise.resolve({ kind: 'busy' as const })
      const invalid = attemptReady(id)
      if (invalid) return Promise.resolve(finish(invalid))
      if (!validVerificationCode(code)) return Promise.resolve(finish('invalid-input'))
      return run('verify', decodeVerified, data => {
        if (data.attemptId !== id) throw new TransportError('invalid-response', { uncertain: true })
        command = null; publish({ unknown: null }); return 'verified'
      }, { attemptId: id, code }, signal)
    },
    resend(id: string, signal?: AbortSignal) {
      if (active || state.unknown) return Promise.resolve({ kind: 'busy' as const })
      const invalid = attemptReady(id)
      if (invalid) return Promise.resolve(finish(invalid))
      if (state.accepted?.attemptId === id && state.resendDeadline > now()) return Promise.resolve(finish('rate-limited'))
      command = { kind: 'resend', started: now(), body: { attemptId: id, idempotencyKey: crypto.randomUUID() } }
      return dispatch(command, signal)
    },
    checkAdmission: (signal?: AbortSignal) => run('admission', decodeAdmission, data => {
      publish({ admission: data }); return admissionKind(data)
    }, undefined, signal),
    acceptPolicy(ids: readonly string[], signal?: AbortSignal) {
      if (active || state.unknown) return Promise.resolve({ kind: 'busy' as const })
      if (!policyReady()) return Promise.resolve(finish('policy-unavailable'))
      if (!exactAcknowledgements(state.policy!, ids)) return Promise.resolve(finish('acknowledgements-required'))
      command = { kind: 'consent', started: now(), body: { policyRevision: state.policy!.revision, acknowledgementIds: [...ids], idempotencyKey: crypto.randomUUID() } }
      return dispatch(command, signal)
    },
    retryUnknown(signal?: AbortSignal) {
      if (!command || !state.unknown) return Promise.resolve(finish('unavailable'))
      if (state.retryDeadline > now()) return Promise.resolve(finish('rate-limited'))
      if (command.kind !== 'consent') {
        const invalid = attemptReady((command.body as { attemptId: string }).attemptId)
        if (invalid) return Promise.resolve(finish(invalid))
      }
      return dispatch(command, signal)
    },
    /** Explicit abandonment loses only the local result; it never claims rollback of a server action. */
    reset() { active?.abort(); clear() },
    cancel() {
      active?.abort()
      if (command) { command = null; publish({ unknown: null, attempt: null, accepted: null, attemptDeadline: 0, resendDeadline: 0, result: { kind: 'cancelled' } }) }
    },
    dispose() { disposed = true; active?.abort(); command = null; unregister(); listeners.clear(); state = initial() },
  }
  return port
}
