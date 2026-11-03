// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { projectCapabilities } from '../../../app/capabilities'
import type { Activity } from '../model/contract'
import { activity, application, errorResponse, ids, party, pendingReceipt } from './fixtures/data'
import { harness, ready } from './fixtures/harness'
const owned: ReturnType<typeof harness>[] = []
function setup(handle?: Parameters<typeof harness>[0], limit?: number) { const h = harness(handle, limit); owned.push(h); return h }
afterEach(() => { owned.splice(0).forEach(h => h.dispose()); vi.useRealTimers() })
const commands = (h: ReturnType<typeof harness>) => h.requests.filter(r => r.method !== 'GET')
function fallback(request: Request) { const p = new URL(request.url).pathname; return Response.json(p.endsWith('/me/parties') ? activity() : p.endsWith('/parties') ? { items: [party()], next_cursor: null } : party()) }
function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(r => { resolve = r }); return { promise, resolve } }
describe('FE21 canonical request, privacy and command lifecycle', () => {
  it('uses protected no-store canonical requests with event binding and exact frozen filters', async () => {
    const h = setup(); await ready(h)
    await h.controller.setFilters({ ticket_section: 'Standing A', state: 'OPEN', has_space: true, starts_after: '2026-11-08T08:00:00Z', starts_before: '2026-11-08T10:00:00Z' })
    const r = h.requests.at(-1)!, u = new URL(r.url)
    expect(u.pathname).toBe(`/api/concert/events/${ids.event}/parties`); expect(u.searchParams.get('ticket_section')).toBe('Standing A'); expect(u.searchParams.get('has_space')).toBe('true')
    expect(r.headers.get('Authorization')).toBe('Bearer fixture-token-memory-only'); expect(r.cache).toBe('no-store'); expect(r.credentials).toBe('same-origin')
    expect(h.controller.applyReason()).toBeNull()
    const count = h.requests.length
    await h.controller.setFilters({ ...h.controller.read().filters, starts_before: '2026-11-08T07:00:00Z' }); expect(h.requests).toHaveLength(count); expect(h.controller.read().filterError).toBe(true)
  })
  it('suppresses duplicate clicks synchronously and sends one POST with captured revision/note/key', async () => {
    const pending = deferred<Response>(), h = setup(r => r.method === 'POST' ? pending.promise : fallback(r)); await ready(h)
    h.controller.setNote('A private note'); const first = h.controller.apply(), second = h.controller.apply(); await vi.waitFor(() => expect(commands(h)).toHaveLength(1))
    h.controller.setNote('must not change captured input'); h.controller.setView('plan'); await h.controller.loadDetail(ids.next)
    const r = commands(h)[0]!; expect(await r.clone().json()).toEqual({ party_revision: 3, note: 'A private note' }); expect(r.headers.get('Idempotency-Key')).toMatch(/^[0-9a-f-]{36}$/)
    expect(r.headers.get('X-MCR-Intent')).toBe('browser'); expect(r.headers.has('Origin')).toBe(false)
    pending.resolve(Response.json(application())); await Promise.all([first, second]); expect(commands(h)).toHaveLength(1)
    expect(h.controller.read().command).toMatchObject({ phase: 'succeeded', currentState: 'PENDING' })
    expect(JSON.stringify(h.controller.read())).not.toContain(r.headers.get('Idempotency-Key'))
  })
  it.each(['owned', 'memberships', 'applications'] as const)('never treats partial %s absence as eligibility and retains independent cursors', async collection => {
    let calls = 0
    const h = setup(r => {
      if (!new URL(r.url).pathname.endsWith('/me/parties')) return fallback(r)
      calls++; const a = activity()
      return Response.json(calls === 1 ? { owned: { ...a.owned, next_cursor: 'own_next' }, memberships: { ...a.memberships, next_cursor: 'member_next' }, applications: { ...a.applications, next_cursor: 'app_next' } } : a)
    }); await ready(h); expect(h.controller.applyReason()).toContain('remaining MY PLAN'); await h.controller.apply(); expect(commands(h)).toHaveLength(0)
    await h.controller.loadActivity(collection)
    const a = h.controller.read().activity.data!; expect(a[collection].next_cursor).toBeNull()
    const other = (['owned', 'memberships', 'applications'] as const).filter(k => k !== collection); expect(other.every(k => a[k].next_cursor !== null)).toBe(true)
    const q = new URL(h.requests.at(-1)!.url).searchParams; expect([...q.keys()]).toEqual(['limit', collection === 'owned' ? 'owned_cursor' : collection === 'memberships' ? 'membership_cursor' : 'application_cursor'])
  })
  it('admits memberships for one actor across different parties and blocks known active membership', async () => {
    const h = setup(r => {
      if (!new URL(r.url).pathname.endsWith('/me/parties')) return fallback(r)
      const m = { event_id: ids.event, party_id: ids.party, user_id: ids.actor, role: 'MEMBER', state: 'ACTIVE', revision: 1, joined_at: party().created_at }
      return Response.json({ ...activity(), memberships: { items: [m, { ...m, party_id: ids.next }], next_cursor: null } })
    }); await ready(h); expect(h.controller.read().activity.phase).toBe('ready'); expect(h.controller.applyReason()).toBe('You are already a member.')
  })
  it.each(['CLOSED', 'CANCELLED', 'COMPLETED'] as const)('disables apply for %s', async state => { const h = setup(r => new URL(r.url).pathname.endsWith(ids.party) ? Response.json(party({ state })) : fallback(r)); await ready(h); await h.controller.apply(); expect(h.controller.applyReason()).toContain(state.toLowerCase()); expect(commands(h)).toHaveLength(0) })
  it('full party, owner and existing pending application never enable apply', async () => {
    for (const change of [party({ capacity: 2 }), party({ owner: application().applicant })]) {
      const h = setup(r => new URL(r.url).pathname.endsWith(ids.party) ? Response.json(change) : fallback(r)); await ready(h); expect(h.controller.applyReason()).not.toBeNull(); await h.controller.apply(); expect(commands(h)).toHaveLength(0)
    }
    const h = setup(r => new URL(r.url).pathname.endsWith('/me/parties') ? Response.json({ ...activity(), applications: { items: [application()], next_cursor: null } }) : fallback(r)); await ready(h); expect(h.controller.applyReason()).toContain('already have')
  })
  it('normal mutations fail closed on missing/stale capabilities and ordinary deny; READ_ONLY safety cancellation remains available', async () => {
    const h = setup(r => r.method === 'DELETE' ? Response.json(application({ state: 'CANCELLED', revision: 2 })) : new URL(r.url).pathname.endsWith('/me/parties') ? Response.json({ ...activity(), applications: { items: [application()], next_cursor: null } }) : fallback(r)); await ready(h)
    const access = h.access()!; h.update({ ...access, phase: 'READ_ONLY', capabilities: projectCapabilities({ ...access.capabilities, mutations: { available: false, reason: 'EVENT_READ_ONLY' } }) })
    expect(h.controller.applyReason()).not.toBeNull(); expect(h.controller.cancelReason(ids.app)).toBeNull(); await h.controller.cancel(ids.app)
    const r = commands(h)[0]!, u = new URL(r.url); expect(r.method).toBe('DELETE'); expect(u.searchParams.get('expected_revision')).toBe('1'); expect(await r.text()).toBe(''); expect(r.headers.has('Content-Type')).toBe(false)
    expect(h.controller.read().command.currentState).toBe('CANCELLED')
    h.update({ ...access, capabilities: projectCapabilities({ ...access.capabilities, mutations: { available: false, reason: 'FEATURE_DISABLED' } }) }); expect(h.controller.cancelReason(ids.app)).not.toBeNull()
    h.update(null); expect(h.controller.read().list.data).toBeNull(); expect(h.controller.applyReason()).not.toBeNull()
  })
  it.each([403, 404, 429, 503])('presents bounded safe error %i and gates writes', async status => {
    const h = setup(() => errorResponse(status, status === 403 ? 'PERMISSION_DENIED' : status === 404 ? 'RESOURCE_UNAVAILABLE' : status === 429 ? 'RATE_LIMITED' : 'DEPENDENCY_UNAVAILABLE'))
    await ready(h); expect(h.controller.read().detail.phase).toBe('error'); expect(JSON.stringify(h.controller.read())).not.toContain('PRIVATE RAW'); expect(h.controller.applyReason()).not.toBeNull()
    const count = h.requests.length; if (status === 429) { await h.controller.loadDetail(); expect(h.requests.length).toBe(count) }
  })
  it('rejects cross-context successful data before committing private state', async () => {
    const h = setup(() => Response.json({ ...activity(), applications: { items: [application({ event_id: ids.other })], next_cursor: null } })); await h.controller.loadActivity(); expect(h.controller.read().activity.data).toBeNull(); expect(h.controller.read().activity.phase).toBe('error')
  })
  it('repeated/duplicate continuation pages fail closed without auto pagination or skipping other cursors', async () => {
    const h = setup(r => new URL(r.url).pathname.endsWith('/parties') && !new URL(r.url).pathname.includes('/me/') ? Response.json({ items: [party()], next_cursor: 'same' }) : fallback(r))
    await h.controller.loadList(); const count = h.requests.length; await Promise.resolve(); expect(h.requests.length).toBe(count)
    await h.controller.loadList(true); expect(h.controller.read().list.phase).toBe('error'); expect(h.controller.read().list.data?.items).toHaveLength(1)
  })
  it('keeps original key and body across network loss, panel changes, 404 and outage status checks; never resends', async () => {
    let status = 404
    const h = setup(r => r.method === 'POST' ? Promise.reject(new TypeError('offline')) : new URL(r.url).pathname.endsWith('/commands') ? errorResponse(status, status === 404 ? 'RESOURCE_UNAVAILABLE' : 'DEPENDENCY_UNAVAILABLE') : fallback(r)); await ready(h)
    h.controller.setNote('original'); await h.controller.apply(); expect(h.controller.read().command.phase).toBe('uncertain')
    const key = commands(h)[0]!.headers.get('Idempotency-Key'); h.controller.setView('plan'); h.controller.setNote('replacement'); await h.controller.loadDetail(ids.next)
    await h.controller.checkStatus(); expect(h.controller.read().command).toMatchObject({ phase: 'unavailable', error: 'missing' })
    status = 503; await h.controller.checkStatus(); expect(h.controller.read().command.phase).toBe('unavailable'); await h.controller.apply()
    expect(commands(h)).toHaveLength(1); const checks = h.requests.filter(r => new URL(r.url).pathname.endsWith('/commands'))
    expect(checks).toHaveLength(2); expect(checks.every(r => new URL(r.url).searchParams.get('idempotency_key') === key && new URL(r.url).searchParams.get('operation_id') === 'applyConcertParty' && !r.headers.has('Idempotency-Key'))).toBe(true)
    expect(h.controller.read().note).not.toBe('replacement')
  })
  it('PENDING receipt and bounded checks never poll or replay; SUCCEEDED needs fresh own application reconciliation', async () => {
    let current: Activity = activity(), successful = false
    const h = setup(r => {
      const url = new URL(r.url)
      if (r.method === 'POST') return Response.json(pendingReceipt(r.headers.get('Idempotency-Key')!), { status: 202 })
      if (url.pathname.endsWith('/commands')) return Response.json(pendingReceipt(url.searchParams.get('idempotency_key')!, successful ? { state: 'SUCCEEDED', resource_id: ids.app, resource_revision: 1, result_status: 200 } : {}))
      return url.pathname.endsWith('/me/parties') ? Response.json(current) : fallback(r)
    }); await ready(h); await h.controller.apply(); expect(h.controller.read().command.phase).toBe('pending')
    const count = h.requests.length; await new Promise(resolve => setTimeout(resolve, 20)); expect(h.requests).toHaveLength(count)
    successful = true; await h.controller.checkStatus(); expect(h.controller.read().command.phase).toBe('reconciling')
    current = { ...activity(), applications: { items: [application({ state: 'APPROVED', revision: 3 })], next_cursor: null } }
    await h.controller.loadActivity(); expect(h.controller.read().command).toMatchObject({ phase: 'succeeded', currentState: 'APPROVED' }); expect(commands(h)).toHaveLength(1)
  })
  it.each(['RETIRED', 'FAILED', 'EXPIRED'] as const)('retains safe %s recovery behavior with no new command', async state => {
    const h = setup(r => r.method === 'POST' ? Response.json(pendingReceipt(r.headers.get('Idempotency-Key')!, state === 'EXPIRED' ? { expires_at: '2026-11-03T01:01:00Z' } : {}), { status: 202 }) : new URL(r.url).pathname.endsWith('/commands') ? Response.json(pendingReceipt(new URL(r.url).searchParams.get('idempotency_key')!, state === 'FAILED' ? { state: 'FAILED', failure_reason: 'PARTY_FULL' } : { state: 'RETIRED' })) : fallback(r)); await ready(h); await h.controller.apply()
    if (state !== 'EXPIRED') await h.controller.checkStatus()
    expect(h.controller.read().command.phase).toBe(state === 'FAILED' ? 'failed' : state === 'EXPIRED' ? 'expired' : 'retired'); await h.controller.apply(); expect(commands(h)).toHaveLength(1)
  })
  it('malformed mutation success is uncertain and can only use original-key status recovery', async () => {
    const h = setup(r => r.method === 'POST' ? Response.json(application({ party_id: ids.next })) : fallback(r)); await ready(h); await h.controller.apply(); expect(h.controller.read().command.phase).toBe('uncertain'); expect(commands(h)).toHaveLength(1)
  })
  it('401 command invalidates canonical identity without refresh or replay', async () => {
    const h = setup(r => r.method === 'POST' ? errorResponse(401, 'AUTHENTICATION_REQUIRED') : fallback(r)); await ready(h); await h.controller.apply()
    expect(h.auth.session.read().userId).toBeNull(); expect(h.controller.read().command.phase).toBe('idle'); expect(h.requests.some(r => r.url.includes('/api/auth/'))).toBe(false); expect(commands(h)).toHaveLength(1)
  })
  it.each(['policy', 'identity', 'event', 'dispose'] as const)('synchronously clears private rows, draft and captured receipt on %s and fences noncooperative late completion', async kind => {
    const pending = deferred<Response>(), h = setup(r => r.method === 'POST' ? pending.promise : fallback(r)); await ready(h); h.controller.setNote('private draft'); const sending = h.controller.apply(); await vi.waitFor(() => expect(commands(h)).toHaveLength(1))
    const old = h.auth.privacy.capture(), oldAccess = h.access()!
    if (kind === 'policy') h.auth.privacy.changePolicy(old, 'block')
    else if (kind === 'dispose') h.controller.dispose()
    else h.auth.session.replace(old.generation, { userId: kind === 'identity' ? ids.other : ids.actor, eventId: kind === 'event' ? ids.other : ids.event, accessToken: 'replacement-token' })
    expect(h.controller.read()).toMatchObject({ note: '', list: { data: null }, detail: { data: null }, activity: { data: null }, command: { phase: 'idle' } })
    pending.resolve(Response.json(application())); await sending; expect(h.controller.read().command.phase).toBe('idle'); expect(h.controller.read().activity.data).toBeNull()
    if (kind === 'policy') { expect(h.auth.session.read().generation).toBe(old.generation); h.update(oldAccess); expect(h.controller.read().access).toBe(false) }
    await h.controller.checkStatus(); expect(h.requests.filter(r => r.url.includes('/commands'))).toHaveLength(0)
  })
  it('clears late read completions after selection replacement and uses only current selected party', async () => {
    const pending = deferred<Response>(), h = setup(r => new URL(r.url).pathname.endsWith(ids.party) ? pending.promise : fallback(r))
    const first = h.controller.loadDetail(ids.party); await vi.waitFor(() => expect(h.requests).toHaveLength(1)); await h.controller.loadDetail(ids.next)
    pending.resolve(Response.json(party())); await first; expect(h.controller.read().detail.data).toBeNull()
  })
})

describe('FE21 fresh-intent and immutable projection regression', () => {
  it('requires fresh party/activity reads after definitive failure and never replays the same captured input', async () => {
    const h = setup(r => r.method === 'POST' ? errorResponse(409, 'VERSION_CONFLICT') : fallback(r)); await ready(h)
    await h.controller.apply(); expect(h.controller.read().command.phase).toBe('failed'); expect(h.controller.read().activity.data).toBeNull(); expect(h.controller.read().detail.data).toBeNull()
    expect(h.controller.canReviewNewIntent()).toBe(false); h.controller.clearFailure(); expect(h.controller.read().command.phase).toBe('failed')
    await Promise.all([h.controller.loadActivity(), h.controller.loadDetail()]); expect(h.controller.canReviewNewIntent()).toBe(true); h.controller.clearFailure()
    await h.controller.apply(); expect(commands(h)).toHaveLength(1); h.controller.setNote('A different explicit intent'); await h.controller.apply(); expect(commands(h)).toHaveLength(2)
  })
  it('allows a later explicit application after a confirmed cancellation, without treating uncertain cancellation as completion', async () => {
    let current = activity()
    const h = setup(r => {
      const path = new URL(r.url).pathname
      if (r.method !== 'GET') { const a = application({ state: r.method === 'DELETE' ? 'CANCELLED' : 'PENDING', revision: r.method === 'DELETE' ? 2 : 1 }); current = { ...activity(), applications: { items: [a], next_cursor: null } }; return Response.json(a) }
      return path.endsWith('/me/parties') ? Response.json(current) : fallback(r)
    }); await ready(h); await h.controller.apply(); await h.controller.cancel(ids.app); expect(h.controller.applyReason()).toBeNull(); await h.controller.apply(); expect(commands(h).map(r => r.method)).toEqual(['POST', 'DELETE', 'POST'])
    expect(new Set(commands(h).map(r => r.headers.get('Idempotency-Key'))).size).toBe(3)
  })
  it('freezes merged pagination and all resource projections against accidental eligibility mutation', async () => {
    const h = setup(r => {
      const url = new URL(r.url)
      return url.pathname.endsWith('/parties') && !url.pathname.includes('/me/') ? Response.json({ items: [party({ party_id: url.searchParams.has('cursor') ? ids.next : ids.party })], next_cursor: url.searchParams.has('cursor') ? null : 'next_page' }) : fallback(r)
    }); await h.controller.loadList(); await h.controller.loadList(true); const data = h.controller.read().list.data!
    expect(Object.isFrozen(data)).toBe(true); expect(Object.isFrozen(data.items)).toBe(true); expect(Object.isFrozen(h.controller.read().list)).toBe(true)
  })
})

describe('FE21 cancellation uncertainty and capability admission', () => {
  it('keeps an uncertain safety cancellation locked and queries the same original cancel key without replay', async () => {
    const h = setup(r => r.method === 'DELETE' ? Promise.reject(new TypeError('lost cancellation response'))
      : new URL(r.url).pathname.endsWith('/commands') ? errorResponse(404, 'RESOURCE_UNAVAILABLE')
      : new URL(r.url).pathname.endsWith('/me/parties') ? Response.json({ ...activity(), applications: { items: [application()], next_cursor: null } }) : fallback(r))
    await ready(h); await h.controller.cancel(ids.app); expect(h.controller.read().command.phase).toBe('uncertain'); await h.controller.cancel(ids.app); await h.controller.apply(); await h.controller.checkStatus()
    const c = commands(h); expect(c).toHaveLength(1); const query = new URL(h.requests.at(-1)!.url).searchParams
    expect(query.get('operation_id')).toBe('cancelConcertApplication'); expect(query.get('idempotency_key')).toBe(c[0]!.headers.get('Idempotency-Key')); expect(h.controller.read().command.phase).toBe('unavailable')
  })
  it.each(['DRAFT', 'ARCHIVED'] as const)('never infers ordinary or safety write authority in %s', async phase => {
    const h = setup(); await ready(h); const access = h.access()!
    h.update({ ...access, phase, capabilities: projectCapabilities({ ...access.capabilities, mutations: { available: false, reason: 'EVENT_READ_ONLY' } }) })
    expect(h.controller.applyReason()).not.toBeNull(); expect(h.controller.cancelReason(ids.app)).not.toBeNull(); await h.controller.apply(); expect(commands(h)).toHaveLength(0)
    expect(h.auth.queryClient.getQueryCache().getAll()).toHaveLength(0); expect(h.auth.queryClient.getMutationCache().getAll()).toHaveLength(0)
  })
})

it('binds later status responses to an application ID already learned from a SUCCEEDED receipt', async () => {
  let resourceId = ids.app
  const h = setup(r => r.method === 'POST' ? Response.json(pendingReceipt(r.headers.get('Idempotency-Key')!), { status: 202 })
    : new URL(r.url).pathname.endsWith('/commands') ? Response.json(pendingReceipt(new URL(r.url).searchParams.get('idempotency_key')!, { state: 'SUCCEEDED', resource_id: resourceId, resource_revision: 1, result_status: 200 })) : fallback(r))
  await ready(h); await h.controller.apply(); await h.controller.checkStatus(); expect(h.controller.read().command.phase).toBe('reconciling')
  resourceId = ids.next; await h.controller.checkStatus(); expect(h.controller.read().command).toMatchObject({ phase: 'unavailable', error: 'invalid' }); expect(commands(h)).toHaveLength(1)
})

it('evicts all observers despite a throwing subscriber and starts no reads after feature disposal', async () => {
  const h = setup(); await ready(h); let notified = 0
  h.controller.subscribe(() => { throw new Error('faulty observer') }); h.controller.subscribe(() => { notified++ })
  h.auth.privacy.changePolicy(h.auth.privacy.capture(), 'block'); expect(notified).toBeGreaterThan(0); expect(h.controller.read().activity.data).toBeNull()
  h.update({ ...h.access()!, scope: h.auth.privacy.capture() }); h.controller.dispose(); const count = h.requests.length
  await Promise.all([h.controller.loadList(), h.controller.loadActivity(), h.controller.loadDetail(ids.party), h.controller.checkStatus()]); expect(h.requests).toHaveLength(count)
})
