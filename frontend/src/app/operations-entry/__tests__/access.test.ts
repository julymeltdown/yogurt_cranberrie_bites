// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import operation from './operation.fixture.json'
import { createSessionController } from '../../../features/auth/model/session'
import { isolatedCookiePolicy, deferred } from '../../../features/auth/__tests__/fixtures/data'
import { createReportReview } from '../../../features/admin/reports'
import { createOperatorGuard } from '../../../features/admin/event/model/lease'
import { createAuthorityTimeline } from '../../community-authority/timeline'
import { DOMAIN_OPERATIONS } from '../../../shared/api/domain-operations.generated'
import { createOperationsEntry } from '..'
import { decodeOperationsAccess, operationsAccessPath } from '../contract'

// TEST ONLY: exact generated transport record from contract3a089a3b, never a production registration.
vi.mock('../../../shared/api/domain-operations.generated', async original => {
  const actual = await original<typeof import('../../../shared/api/domain-operations.generated')>()
  const fixture = await import('./operation.fixture.json')
  return { ...actual, DOMAIN_OPERATIONS: { ...actual.DOMAIN_OPERATIONS,
    [fixture.default.operationId]: fixture.default.definition } }
})
const id = (n: number) => `00000000-0000-4000-8000-${String(n).padStart(12, '0')}`
// Same closed positive wire fixture as backend205 operations_public_contract_test.py@3a089a3b.
const wire = () => ({ server_time: '2026-11-04T17:00:00.000Z', access: {
  actor_id: id(1), event_id: id(2), role: 'OPERATOR', role_revision: 7, decision_id: id(3),
  checked_at: '2026-11-04T17:00:00.000Z', expires_at: '2026-11-04T17:01:00.000Z' } })
const owners: { dispose(): void }[] = []
function setup() {
  let elapsed = 0, value: unknown = wire(), response: (() => Promise<Response>) | null = null, invalidClock = false
  const requests: Request[] = [], lifetime = new AbortController()
  const auth = createSessionController({ eventId: id(2), cookiePolicy: isolatedCookiePolicy(), fetch: async input => {
    requests.push(input as Request); return response ? response() : Response.json(value)
  } })
  auth.session.replace(0, { userId: id(1), eventId: id(2), accessToken: 'TEST_ONLY_memory' })
  let interrupt = () => {}
  const entry = createOperationsEntry({ auth, signal: lifetime.signal, timeline: signal => {
    return createAuthorityTimeline({ observe: () => invalidClock ? null : { nowMs: elapsed, origin: 100 }, maxObservationGapMs: 120_000,
      onInterruption: invalidate => { interrupt = invalidate; signal.addEventListener('abort', invalidate, { once: true });
        return () => signal.removeEventListener('abort', invalidate) } })
  } })
  const owner = { auth, entry, requests, lifetime, elapsed: (n: number) => { elapsed = n }, invalidClock: () => { invalidClock = true },
    interrupt: () => interrupt(), value: (v: unknown) => { value = v }, response: (fn: () => Promise<Response>) => { response = fn },
    dispose() { entry.dispose(); auth.dispose() } }
  owners.push(owner); return owner
}
afterEach(() => { owners.splice(0).forEach(owner => owner.dispose()); vi.restoreAllMocks(); vi.useRealTimers()
  Reflect.set(DOMAIN_OPERATIONS, operation.operationId, operation.definition) })

it('uses the actual canonical bodyless SELF GET and shares one immutable decision across ports', async () => {
  const h = setup(); await h.entry.refresh()
  expect(h.requests).toHaveLength(1)
  const request = h.requests[0]!
  expect(request.method).toBe('GET'); expect(new URL(request.url).pathname).toBe(operationsAccessPath(id(2)))
  expect(new URL(request.url).search).toBe(''); expect(request.headers.get('Idempotency-Key')).toBeNull()
  expect(request.cache).toBe('no-store'); expect(await request.clone().text()).toBe('')
  expect(h.entry.read()).toMatchObject({ decisionId: id(3), role: 'OPERATOR', roleRevision: 7 })
  expect(h.entry.authority.read()).toMatchObject({ actorId: id(1), eventId: id(2), revision: 7 })
  expect(h.entry.moderator.read()).toMatchObject({ decisionId: id(3), scope: h.auth.privacy.capture() })
  expect(Object.isFrozen(h.entry.read())).toBe(true); expect(Object.isFrozen(h.entry.read()!.scope)).toBe(true)
  expect(h.entry.clock.read()!.nowMs).toBe(Date.parse(wire().server_time) + 1)
  expect(h.entry.moderatorNow()).toBe(h.entry.clock.read()!.nowMs)
  expect(h.entry.read()).not.toHaveProperty('eventRevision'); expect(h.entry.read()).not.toHaveProperty('phase')
})

it('does not issue a fallback request when the generated operation is absent', async () => {
  Reflect.deleteProperty(DOMAIN_OPERATIONS, operation.operationId)
  const h = setup(); await h.entry.refresh(); expect(h.requests).toHaveLength(0); expect(h.entry.read()).toBeNull()
})

it('rejects a malformed generated transport binding before dispatch', async () => {
  Reflect.set(DOMAIN_OPERATIONS, operation.operationId, { ...operation.definition, idempotency: true })
  const h = setup(); await h.entry.refresh(); expect(h.requests).toHaveLength(0); expect(h.entry.read()).toBeNull()
})

it('accepts MODERATOR without manufacturing OPERATOR authority', async () => {
  const h = setup(), value = wire(); value.access.role = 'MODERATOR'; h.value(value); await h.entry.refresh()
  expect(h.entry.moderator.read()!.role).toBe('MODERATOR'); expect(h.entry.authority.read()!.role).toBe('MODERATOR')
  const scrub = vi.fn(), guard = createOperatorGuard({ auth: h.auth, authority: h.entry.authority, clock: h.entry.clock }, scrub)
  expect(guard.valid()).toBe(false); expect(scrub).toHaveBeenCalledTimes(1)
})

it.each([
  ['outer grant', (v: ReturnType<typeof wire>) => ({ ...v, grant: true })],
  ['private field', (v: ReturnType<typeof wire>) => ({ ...v, access: { ...v.access, token: 'PRIVATE' } })],
  ['invented event context', (v: ReturnType<typeof wire>) => ({ ...v, access: { ...v.access, event_revision: 4 } })],
  ['foreign actor', (v: ReturnType<typeof wire>) => ({ ...v, access: { ...v.access, actor_id: id(99) } })],
  ['foreign event', (v: ReturnType<typeof wire>) => ({ ...v, access: { ...v.access, event_id: id(99) } })],
  ['invalid role', (v: ReturnType<typeof wire>) => ({ ...v, access: { ...v.access, role: 'MEMBER' } })],
  ['zero revision', (v: ReturnType<typeof wire>) => ({ ...v, access: { ...v.access, role_revision: 0 } })],
  ['unsafe revision', (v: ReturnType<typeof wire>) => ({ ...v, access: { ...v.access, role_revision: Number.MAX_SAFE_INTEGER + 1 } })],
  ['fractional revision', (v: ReturnType<typeof wire>) => ({ ...v, access: { ...v.access, role_revision: 1.5 } })],
  ['invalid decision', (v: ReturnType<typeof wire>) => ({ ...v, access: { ...v.access, decision_id: 'not-a-uuid' } })],
  ['non-UTC', (v: ReturnType<typeof wire>) => ({ ...v, server_time: '2026-11-04T17:00:00+09:00' })],
  ['invalid calendar date', (v: ReturnType<typeof wire>) => ({ ...v, server_time: '2026-02-30T17:00:00.000Z' })],
  ['different SQL sample', (v: ReturnType<typeof wire>) => ({ ...v, server_time: '2026-11-04T17:00:00.001Z' })],
  ['expired', (v: ReturnType<typeof wire>) => ({ ...v, access: { ...v.access, expires_at: v.server_time } })],
  ['over60 seconds', (v: ReturnType<typeof wire>) => ({ ...v, access: { ...v.access, expires_at: '2026-11-04T17:01:00.001Z' } })],
  ['submillisecond excess', (v: ReturnType<typeof wire>) => ({ ...v, access: { ...v.access, expires_at: '2026-11-04T17:01:00.000001Z' } })],
] as const)('rejects %s without any usable access or clock', async (_name, mutate) => {
  const h = setup(); h.value(mutate(wire())); await h.entry.refresh()
  expect(h.requests).toHaveLength(1)
  expect(h.entry.read()).toBeNull(); expect(h.entry.moderator.read()).toBeNull()
  expect(h.entry.authority.read()).toBeNull(); expect(h.entry.clock.read()).toBeNull(); expect(Number.isNaN(h.entry.moderatorNow())).toBe(true)
})

it('fences decoder status, path and canonical generation', () => {
  const h = setup(), scope = h.auth.privacy.capture(), context = { scope, status: 200, path: operationsAccessPath(id(2)) }
  for (const wrong of [{ ...context, status: 202 }, { ...context, path: operationsAccessPath(id(99)) },
    { ...context, scope: { ...scope, generation: scope.generation + 1 } }]) expect(() => decodeOperationsAccess(wire(), wrong, scope)).toThrow()
})

it('charges the full RTT to request-start expiry and ignores the wall clock', async () => {
  const h = setup(); h.response(async () => { h.elapsed(40_000); return Response.json(wire()) }); await h.entry.refresh()
  expect(h.entry.read()).not.toBeNull(); expect(h.entry.moderatorNow()).toBe(Date.parse(wire().server_time) + 40_001)
  vi.spyOn(Date, 'now').mockReturnValue(0); h.elapsed(59_998); expect(h.entry.read()).not.toBeNull()
  h.elapsed(59_999); expect(h.entry.read()).toBeNull()
})

it('rejects a response whose full RTT exhausted its bounded TTL', async () => {
  const h = setup(); h.response(async () => { h.elapsed(60_000); return Response.json(wire()) }); await h.entry.refresh()
  expect(h.entry.read()).toBeNull(); expect(h.entry.clock.read()).toBeNull()
})

it('withdraws synchronously before equal-field replacement and permanently retires original FE38 guard', async () => {
  const h = setup(); await h.entry.refresh()
  const scrub = vi.fn(), guard = createOperatorGuard({ auth: h.auth, authority: h.entry.authority, clock: h.entry.clock }, scrub)
  expect(guard.valid()).toBe(true)
  const transitions: (string | null)[] = []; h.entry.subscribe(() => transitions.push(h.entry.read()?.decisionId ?? null))
  const next = wire(); next.access.decision_id = id(4); h.value(next)
  const pending = h.entry.refresh(); expect(h.entry.read()).toBeNull(); expect(scrub).toHaveBeenCalledTimes(1)
  await pending; expect(transitions).toEqual([null, id(4)]); expect(guard.valid()).toBe(false)
  const fresh = createOperatorGuard({ auth: h.auth, authority: h.entry.authority, clock: h.entry.clock }, () => {})
  expect(fresh.valid()).toBe(true); fresh.withdraw()
})

it('propagates explicit server no-assignment to FE37 without exposing the old decision', async () => {
  const h = setup(); await h.entry.refresh()
  const review = createReportReview({ auth: h.auth, access: h.entry.moderator, now: h.entry.moderatorNow })
  expect(review.read().access).toBe(true)
  h.value({ server_time: wire().server_time, access: null }); await h.entry.refresh()
  expect(review.read()).toMatchObject({ access: false, rows: [], detail: null, confirm: false }); review.dispose()
})

it.each([403, 503])('withdraws old access after actual HTTP%s and never retains diagnostic details', async status => {
  const h = setup(); await h.entry.refresh(); h.response(async () => new Response('PRIVATE authority diagnostic', { status }))
  await h.entry.refresh(); expect(h.entry.read()).toBeNull(); expect(h.entry.authority.read()).toBeNull()
})

it.each(['policy', 'session', 'abort', 'dispose'] as const)('aborts a pending read on %s and drops late bytes', async kind => {
  const h = setup(), delayed = deferred<Response>(); h.response(() => delayed.promise)
  const pending = h.entry.refresh(); await vi.waitFor(() => expect(h.requests).toHaveLength(1))
  if (kind === 'policy') h.auth.privacy.changePolicy(h.auth.privacy.capture(), 'block')
  if (kind === 'session') h.auth.session.replace(h.auth.session.read().generation, null)
  if (kind === 'abort') h.lifetime.abort()
  if (kind === 'dispose') h.entry.dispose()
  expect(h.requests[0]!.signal.aborted).toBe(true); delayed.resolve(Response.json(wire())); await pending
  expect(h.entry.read()).toBeNull(); await h.entry.refresh(); expect(h.requests).toHaveLength(1)
})

it('drops a superseded late response without withdrawing the newer decision', async () => {
  const h = setup(), delayed = deferred<Response>(); h.response(() => delayed.promise)
  const first = h.entry.refresh(); await vi.waitFor(() => expect(h.requests).toHaveLength(1))
  const next = wire(); next.access.decision_id = id(4); h.response(async () => Response.json(next))
  await h.entry.refresh(); expect(h.entry.read()?.decisionId).toBe(id(4))
  delayed.resolve(Response.json(wire())); await first; expect(h.entry.read()?.decisionId).toBe(id(4))
})

it.each(['backwards', 'unavailable', 'interrupted'] as const)('withdraws when the monotonic clock becomes %s', async kind => {
  const h = setup(); h.elapsed(100); await h.entry.refresh(); const notify = vi.fn(); h.entry.subscribe(notify)
  if (kind === 'backwards') h.elapsed(50)
  if (kind === 'unavailable') h.invalidClock()
  if (kind === 'interrupted') h.interrupt()
  expect(h.entry.read()).toBeNull(); expect(notify).toHaveBeenCalledTimes(1)
})

it('withdraws on the expiry timer before any consumer reread', async () => {
  vi.useFakeTimers(); const h = setup(); await h.entry.refresh(); const notify = vi.fn(); h.entry.subscribe(notify)
  h.elapsed(60_000); await vi.advanceTimersByTimeAsync(60_000)
  expect(notify).toHaveBeenCalledTimes(1); expect(h.entry.read()).toBeNull()
})

it('rejects a reused decision or a backwards server anchor after refresh', async () => {
  const h = setup(); await h.entry.refresh(); await h.entry.refresh(); expect(h.entry.read()).toBeNull()
  const old = wire(); old.access.decision_id = id(4); old.server_time = old.access.checked_at = '2026-11-04T16:59:59.000Z'
  old.access.expires_at = '2026-11-04T17:00:59.000Z'; h.value(old); await h.entry.refresh(); expect(h.entry.read()).toBeNull()
})
