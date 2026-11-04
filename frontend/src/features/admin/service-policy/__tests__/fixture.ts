// TEST ONLY: controlled HTTP, fictional actor/role and server time. Not production authority.
// The actual canonical SessionController, OperationsEntry, timeline, guard and transport run unchanged.
import { createSessionController } from '../../../auth/model/session'
import { createOperationsEntry } from '../../../../app/operations-entry'
import { createAuthorityTimeline } from '../../../../app/community-authority/timeline'
import { admission } from '../../../auth/__tests__/enrollment-consent/fixtures'
import { capabilities, authResponse } from '../../../auth/__tests__/fixtures/data'
import { createServicePolicyController } from '../controller'
import type { PolicyDecision, PolicyInput, PolicyReceipt } from '../contract'

export const uid = (n: number) => `00000000-0000-4000-8000-${n.toString(16).padStart(12, '0')}`
export const actor = uid(1), eventId = uid(2), epoch = Date.parse('2026-11-05T00:00:00Z')
export const time = (delta = 0) => new Date(epoch + delta).toISOString()
export type Mode = 'success' | 'pending' | 'lost' | 'conflict' | 'forbidden' | 'unavailable' | 'malformed' | 'read-only'
export function fixture() {
  let elapsed = 0, mode: Mode = 'success', role: 'OPERATOR' | 'MODERATOR' | null = 'OPERATOR'
  let policyRevision: number | null = 4, eventRevision = 19, kind: PolicyDecision['kind'] = 'NORMAL', minimum: string | null = '1.0.0'
  let last: PolicyReceipt | null = null, pending: PolicyInput | null = null, keyNumber = 100
  const requests: { path: string; method: string; key: string | null; body: unknown }[] = []
  let override: ((request: Request) => Response | Promise<Response> | null) | null = null
  const error = (status: number, reason: string) => Response.json({ code: 'SYNTHETIC_FAILURE', message: 'Private diagnostic must not render', details: { reason } }, { status })
  const decision = (): PolicyDecision => ({ event_revision: eventRevision, policy_revision: policyRevision, kind,
    minimum_client_version: minimum, server_time: time(elapsed), valid_until: time(elapsed + 30_000) } as PolicyDecision)
  const receipt = (key: string, state: PolicyReceipt['state'], failure: PolicyReceipt['failure_reason'] = null): PolicyReceipt => ({
    receipt_id: uid(80), event_id: eventId, actor_id: actor, operation_id: 'setConcertServicePolicy', idempotency_key: key, state,
    resource_id: state === 'SUCCEEDED' ? eventId : null, resource_revision: state === 'SUCCEEDED' ? eventRevision : null,
    failure_reason: failure, created_at: time(elapsed), expires_at: time(elapsed + 86_400_000), result_status: state === 'SUCCEEDED' ? 200 : null,
  })
  function apply(body: PolicyInput) {
    policyRevision = (policyRevision ?? 0) + 1; eventRevision += 3 // Other event writes make event revision independent from policy CAS.
    kind = body.change.action === 'WITHDRAW' ? 'UNAVAILABLE' : body.change.mode
    minimum = body.change.action === 'WITHDRAW' ? null : body.change.minimum_client_version
  }
  const auth = createSessionController({ eventId, timeoutMs: 3000,
    cookiePolicy: { available: true, onInvalidate: () => () => {}, run: (_kind, work) => work(), dispose() {} },
    fetch: async input => {
      const request = input as Request, path = new URL(request.url).pathname
      const body: unknown = request.method === 'GET' ? null : await request.clone().json()
      requests.push({ path, method: request.method, key: request.headers.get('Idempotency-Key'), body })
      const custom = override?.(request); if (custom) return custom
      if (path === '/api/auth/enrollment/admission') return Response.json(admission())
      if (path === '/api/auth/capabilities') return Response.json(capabilities())
      if (path === '/api/auth/login') return Response.json({ ...authResponse(), userId: actor })
      if (path.endsWith('/operations-access')) return Response.json({ server_time: time(elapsed), access: role ? { actor_id: actor, event_id: eventId,
        role, role_revision: 7, decision_id: uid(90), checked_at: time(elapsed), expires_at: time(elapsed + 60_000) } : null })
      if (path.endsWith('/commands')) return last ? Response.json(last) : error(404, 'RESOURCE_UNAVAILABLE')
      if (request.method === 'GET' && path.endsWith('/service-policy')) return mode === 'unavailable' ? error(503, 'DEPENDENCY_UNAVAILABLE') : Response.json(decision())
      if (request.method === 'PUT' && path.endsWith('/admin/service-policy')) {
        if (mode === 'forbidden') return error(403, 'PERMISSION_DENIED')
        if (mode === 'unavailable') return error(503, 'DEPENDENCY_UNAVAILABLE')
        if (mode === 'read-only') return error(409, 'EVENT_READ_ONLY')
        const input = body as PolicyInput, key = request.headers.get('Idempotency-Key')!
        if (mode === 'conflict' || input.expected_policy_revision !== (policyRevision ?? 0)) {
          policyRevision = (policyRevision ?? 0) + 1; eventRevision++
          last = receipt(key, 'FAILED', 'VERSION_CONFLICT'); return Response.json(last)
        }
        if (mode === 'pending') { pending = input; last = receipt(key, 'PENDING'); return Response.json(last, { status: 202 }) }
        apply(input); last = receipt(key, 'SUCCEEDED')
        if (mode === 'lost') throw TypeError('SYNTHETIC lost response after commit')
        if (mode === 'malformed') return Response.json({ ...last, actor_id: uid(999) })
        return Response.json(last)
      }
      throw Error('Unexpected synthetic route: ' + path)
    } })
  const lifetime = new AbortController()
  let started = false
  // Unit/component setup uses the existing canonical memory session port. Native preview logs in via UI instead.
  function installUnitIdentity() { auth.session.replace(auth.session.read().generation, { userId: actor, eventId, accessToken: 'synthetic-memory-token' }) }
  let entry: ReturnType<typeof createOperationsEntry> | null = null
  let controller: ReturnType<typeof createServicePolicyController> | null = null
  async function initialize() {
    if (started) return; started = true
    await auth.checkAdmission()
    entry = createOperationsEntry({ auth, signal: lifetime.signal, timeline: signal => createAuthorityTimeline({
      observe: () => ({ nowMs: elapsed, origin: 1 }), maxObservationGapMs: 120_000,
      onInterruption: notify => { signal.addEventListener('abort', notify); return () => signal.removeEventListener('abort', notify) },
    }) })
    await entry.refresh()
    controller = createServicePolicyController({ auth, authority: entry.authority, clock: entry.clock, newKey: () => uid(keyNumber++) })
  }
  return { auth, requests, error, decision, receipt, initialize, installUnitIdentity,
    get controller() { if (!controller) throw Error('Initialize fixture'); return controller },
    get entry() { if (!entry) throw Error('Initialize fixture'); return entry },
    mode(value: Mode) { mode = value }, role(value: typeof role) { role = value },
    advance(ms: number) { elapsed += ms },
    override(value: typeof override) { override = value },
    current(value: { policyRevision?: number | null; eventRevision?: number; kind?: PolicyDecision['kind']; minimum?: string | null }) {
      if (value.policyRevision !== undefined) policyRevision = value.policyRevision
      if (value.eventRevision !== undefined) eventRevision = value.eventRevision
      if (value.kind !== undefined) kind = value.kind
      if (value.minimum !== undefined) minimum = value.minimum
    },
    settle() { if (!pending || !last) return; apply(pending); last = { ...last, state: 'SUCCEEDED', resource_id: eventId, resource_revision: eventRevision, result_status: 200 }; pending = null },
    setReceipt(value: PolicyReceipt | null) { last = value },
    withdraw() { entry?.withdraw() },
    dispose() { controller?.dispose(); entry?.dispose(); lifetime.abort(); auth.dispose() },
  }
}
export const validDraft = { action: 'PUBLISH', mode: 'MAINTENANCE', minimumClientVersion: '2.0.0', expiresAt: time(300_000), reason: 'Scheduled service work' } as const
