import type { AccountDeletionPort } from '../../account/controller'
import type { HttpClient, SessionPort, SessionScope } from '../../../shared/api/http'
import { TransportError } from '../../../shared/api/errors'
import type { PrivacyScope, createQueryPrivacy } from '../../../shared/query/privacy'

type Privacy = ReturnType<typeof createQueryPrivacy>
const sameIdentity = (a: SessionScope, b: SessionScope) => a.generation === b.generation && a.userId === b.userId && a.eventId === b.eventId
const sameScope = (a: PrivacyScope, b: PrivacyScope) => sameIdentity(a, b) && a.policyGeneration === b.policyGeneration && a.gate === b.gate
interface CredentialLease { readonly identity: SessionScope; readonly deadline: number; readonly tokenVersion: number }
/** Uses the Auth owner's existing transport. Recovery is a narrow read capability, not a reopened private lease. */
export function createAccountDeletionOwner({ http, session, privacy, usable, credentialExpiry, unauthorized }: {
  http: HttpClient; session: SessionPort; privacy: Privacy
  usable(): boolean; credentialExpiry(): number; unauthorized(): void
}) {
  let disposed = false, lease: CredentialLease | null = null, attempted: SessionScope | null = null
  let awaitingRestriction: PrivacyScope | null = null, recovery: PrivacyScope | null = null
  let handoff: ReturnType<typeof setTimeout> | undefined
  const flights = new Set<AbortController>()
  const remove = privacy.registerCleanup(change => {
    for (const flight of flights) flight.abort()
    clearTimeout(handoff)
    const candidate = awaitingRestriction
    awaitingRestriction = null; recovery = null
    if (candidate && change.reason === 'suspended' && sameScope(candidate, change.previous)
      && sameIdentity(candidate, change.current) && change.current.policyGeneration === candidate.policyGeneration + 1
      && change.current.gate === 'suspended') recovery = change.current
    if (lease && !session.isCurrent(lease.identity)) { lease = null; attempted = null }
  })
  function assert(scope: PrivacyScope, kind: 'request' | 'status', signal: AbortSignal) {
    if (disposed || !usable() || signal.aborted) throw new TransportError('aborted')
    const current = privacy.capture(), credentials = session.read()
    if (!scope.userId || !scope.eventId || !session.isCurrent(scope) || !sameScope(current, scope)
      || !credentials.accessToken || credentials.signal.aborted) throw new TransportError('session-changed')
    const open = scope.gate === 'open' && privacy.isCurrent(scope)
    if (!open && !(kind === 'status' && recovery && sameScope(recovery, scope))) throw new TransportError('authentication-required')
    if (!lease) lease = { identity: scope, deadline: credentialExpiry(), tokenVersion: credentials.tokenVersion }
    if (!sameIdentity(lease.identity, scope) || lease.tokenVersion !== credentials.tokenVersion
      || lease.deadline <= Date.now() || credentialExpiry() <= Date.now()) throw new TransportError('authentication-required')
  }
  async function run<K extends 'request' | 'status'>(kind: K, scope: PrivacyScope, signal: AbortSignal) {
    assert(scope, kind, signal)
    if (kind === 'request' && attempted && sameIdentity(attempted, scope)) throw new TransportError('invalid-request')
    clearTimeout(handoff); awaitingRestriction = null
    const expected = Object.freeze({ ...scope }), controller = new AbortController()
    const abort = () => controller.abort()
    signal.addEventListener('abort', abort, { once: true }); flights.add(controller)
    const path = kind === 'request' ? '/api/auth/account' : '/api/auth/account/deletion'
    try {
      assert(expected, kind, controller.signal)
      if (kind === 'request') attempted = expected // Reserve once before any asynchronous transport work.
      const result = await http.request({ path, method: kind === 'request' ? 'DELETE' : 'GET', access: 'protected', errorFamily: 'auth', signal: controller.signal,
        decode(value, context) {
          assert(expected, kind, controller.signal)
          if (context.path !== path || context.status !== (kind === 'request' ? 202 : 200) || !sameIdentity(context.scope, expected)) throw new TransportError('invalid-response')
          return value
        },
      })
      assert(expected, kind, controller.signal)
      if (!sameIdentity(result.scope, expected)) throw new TransportError('session-changed')
      return { scope: expected, status: (kind === 'request' ? 202 : 200) as K extends 'request' ? 202 : 200, data: result.data }
    } catch (error) {
      if (session.isCurrent(expected) && sameScope(privacy.capture(), expected) && error instanceof TransportError && error.status === 401) unauthorized()
      throw error
    } finally {
      flights.delete(controller); signal.removeEventListener('abort', abort)
      // The consumer performs its restrictive transition immediately after settlement, including an uncertain DELETE.
      if (!disposed && session.isCurrent(expected) && sameScope(privacy.capture(), expected) && expected.gate === 'open') {
        awaitingRestriction = expected
        // Only the consumer's immediate post-settlement transition may transfer this capability.
        handoff = setTimeout(() => { awaitingRestriction = null }, 0)
      }
    }
  }
  const port = Object.freeze<AccountDeletionPort>({ request: (scope, signal) => run('request', scope, signal), status: (scope, signal) => run('status', scope, signal) })
  return { port, dispose() { if (disposed) return; disposed = true; clearTimeout(handoff); remove(); for (const flight of flights) flight.abort(); flights.clear(); lease = null; attempted = null; awaitingRestriction = recovery = null } }
}
