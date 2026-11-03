import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { createOperationsController, dataFromNotice } from '../model/controller'
import { ROLE_MAX_AGE_MS } from '../model/lease'
import { boundReceipt } from '../model/contract'
import { actorId, deferred, errorResponse, eventId, fixture, key, noticeInput, replaceIdentity, settingsInput, time, uid } from './fixtures/data'

const active: ReturnType<typeof fixture>[] = []
const make = (options?: Parameters<typeof fixture>[0]) => { const f = fixture(options); active.push(f); return f }
beforeEach(() => { vi.stubGlobal('location', { origin: 'https://mcr.example.invalid' }) })
afterEach(() => { active.splice(0).forEach(f => f.dispose()); vi.unstubAllGlobals() })
const load = async (f: ReturnType<typeof fixture>) => { await f.controller.loadEvent(); expect(f.controller.read().settings).not.toBeNull() }
const prepare = (f: ReturnType<typeof fixture>) => f.controller.prepareSettings(settingsInput(f.current().settings))

it.each(['MEMBER', 'MODERATOR'] as const)('blocks %s direct access before any read or mutation', async role => {
  const f = make({ role }); await f.controller.loadEvent(); await f.controller.loadPlaces(); await f.controller.loadAudit()
  expect(f.requests).toHaveLength(0); expect(prepare(f)).toBe(false); expect(f.controller.read().access).not.toBe('ready')
  expect(f.subscriptions()).toEqual({ authority: 0, clock: 0, places: 0 })
})
it('requires actual external authority and clock, never bootstrap or a boolean admin flag', async () => {
  const f = make({ absent: true }); await f.controller.loadEvent(); expect(f.requests).toHaveLength(0)
  const controller = createOperationsController({ auth: f.auth, authority: null, clock: null })
  await controller.loadPlaces(); expect(controller.read().access).not.toBe('ready'); controller.dispose()
})
it.each(['actor', 'event', 'policy', 'future', 'expiry', 'stale'] as const)('scrubs and permanently fences %s role mismatches', async kind => {
  const f = make(); await load(f)
  const lease = { ...f.lease()! }
  if (kind === 'actor') lease.actorId = uid(90)
  if (kind === 'event') lease.eventId = uid(90)
  if (kind === 'policy') lease.scope = { ...lease.scope, policyGeneration: 90 }
  if (kind === 'future') lease.checkedAt = time(1000)
  if (kind === 'expiry') lease.expiresAt = time()
  if (kind === 'stale') lease.checkedAt = time(-ROLE_MAX_AGE_MS)
  f.setLease(lease)
  expect(f.controller.read()).toMatchObject({ access: 'withdrawn', settings: null, command: null, notices: [], places: [], audit: [] })
  expect(f.subscriptions()).toEqual({ authority: 0, clock: 0, places: 0 }); expect(prepare(f)).toBe(false)
})
it('expires at the exclusive role deadline and frees every owned subscription', async () => {
  const f = make(); await load(f); f.advance(ROLE_MAX_AGE_MS - 1); expect(f.controller.read().access).toBe('ready')
  f.advance(1); expect(f.controller.read().access).toBe('withdrawn'); expect(f.subscriptions()).toEqual({ authority: 0, clock: 0, places: 0 })
})
it.each(['policy', 'identity', 'logout'] as const)('scrubs private snapshots and pending review on %s change', async change => {
  const f = make(); await load(f); await f.controller.loadPlaces(); await f.controller.loadAudit(); f.controller.selectAudit(uid(50)); prepare(f)
  if (change === 'policy') f.auth.privacy.changePolicy(f.scope, 'block')
  if (change === 'identity') replaceIdentity(f)
  if (change === 'logout') f.auth.session.replace(f.auth.session.read().generation, null)
  expect(f.controller.read()).toMatchObject({ access: 'withdrawn', settings: null, overview: null, places: [], audit: [], auditDetail: null, command: null })
  await f.controller.confirm(); expect(f.writes()).toHaveLength(0)
})
it('requires explicit review and confirm, supports cancellation, and sends exactly one bound command', async () => {
  const f = make(); await load(f); expect(prepare(f)).toBe(true); expect(f.writes()).toHaveLength(0)
  expect(f.controller.read().command).toMatchObject({ target: eventId, expectedRevision: 2, stage: 'review' })
  f.controller.cancel(); expect(f.controller.read().command).toBeNull(); expect(f.writes()).toHaveLength(0)
  prepare(f); await Promise.all([f.controller.confirm(), f.controller.confirm()]); expect(f.writes()).toHaveLength(1)
  expect(f.controller.read().command?.stage).toBe('applied'); expect(f.controller.read().settings?.features.mural).toBe(false)
  const request = f.writes()[0]!; expect(request.headers.get('Idempotency-Key')).toBe(uid(6))
  expect(await request.clone().json()).toMatchObject({ expected_revision: 2, reason: 'SYNTHETIC reviewed kill switch' })
})
it.each(['READ_ONLY', 'ARCHIVED', 'OPEN', 'LIVE'] as const)('rejects uncontracted phase target %s without closure bypass', async phase => {
  const f = make(); await load(f)
  // DRAFT only permits OPEN, so OPEN is exercised after replacing the current phase.
  if (phase === 'OPEN') { f.serverSettings({ ...f.current().settings, event: { ...f.current().settings.event, phase: 'OPEN' } }); await load(f) }
  expect(f.controller.preparePhase({ expected_revision: 2, phase, reason: 'SYNTHETIC phase review' })).toBe(false)
  expect(f.writes()).toHaveLength(0)
})
it('executes only the two contracted phase edges with current CAS', async () => {
  const f = make(); await load(f)
  expect(f.controller.preparePhase({ expected_revision: 2, phase: 'OPEN', reason: 'SYNTHETIC open review' })).toBe(true)
  await f.controller.confirm(); expect(f.controller.read().settings?.event.phase).toBe('OPEN')
  await load(f); expect(f.controller.preparePhase({ expected_revision: 3, phase: 'LIVE', reason: 'SYNTHETIC live review' })).toBe(true)
  await f.controller.confirm(); expect(f.controller.read().settings?.event.phase).toBe('LIVE')
})
it.each(['conflict', 'read-only', 'rate', 'forbidden', 'unavailable'] as const)('handles server %s despite a previously visible control', async mode => {
  const f = make(); await load(f); prepare(f); f.setMode(mode); await f.controller.confirm()
  expect(f.controller.read().command?.stage).not.toBe('applied'); expect(f.writes()).toHaveLength(1)
  if (mode === 'forbidden') expect(f.controller.read().settings).toBeNull()
  else expect(prepare(f)).toBe(false)
  expect(JSON.stringify(f.controller.read())).not.toContain('Raw sensitive failure')
})
it('requires a successful explicit refresh after conflict before another review', async () => {
  const f = make(); await load(f); prepare(f); f.setMode('conflict'); await f.controller.confirm()
  expect(f.controller.read().command?.stage).toBe('conflict'); expect(prepare(f)).toBe(false)
  await f.controller.loadAudit(); expect(prepare(f)).toBe(false)
  f.serverSettings({ ...f.current().settings, revision: 3, event: { ...f.current().settings.event, revision: 3 } })
  await load(f); expect(prepare(f)).toBe(true); expect(f.controller.read().command?.expectedRevision).toBe(3)
})
it('keeps a lost response locked through reads, and resolves only against a bound receipt plus exact current state', async () => {
  const f = make(); await load(f); prepare(f); f.setMode('lost'); await f.controller.confirm()
  expect(f.controller.read().command?.stage).toBe('unknown'); expect(prepare(f)).toBe(false)
  await load(f); expect(f.controller.read().command?.stage).toBe('unknown')
  await f.controller.checkStatus(); expect(f.controller.read().command?.stage).toBe('applied'); expect(f.writes()).toHaveLength(1)
})
it('does not treat a pending receipt or status metadata alone as applied', async () => {
  const f = make(); await load(f); prepare(f); f.setMode('pending'); await f.controller.confirm()
  expect(f.controller.read().command?.stage).toBe('pending'); await f.controller.checkStatus(); expect(f.controller.read().command?.stage).toBe('pending')
  const r = f.getReceipt()!; f.setReceipt({ ...r, state: 'SUCCEEDED', resource_id: eventId, resource_revision: 3, result_status: 200 })
  await f.controller.checkStatus(); expect(f.controller.read().command?.stage).not.toBe('applied'); expect(prepare(f)).toBe(false)
  f.settle(); await f.controller.checkStatus(); expect(f.controller.read().command?.stage).toBe('applied'); expect(f.writes()).toHaveLength(1)
})
it.each(['actor_id', 'event_id', 'operation_id', 'idempotency_key', 'resource_id', 'resource_revision', 'result_status'] as const)('rejects receipt %s binding drift', async field => {
  const f = make(); await load(f); prepare(f); f.setMode('lost'); await f.controller.confirm()
  const r = f.getReceipt()!, bad = { ...r, [field]: field === 'resource_revision' ? 2 : field === 'result_status' ? 202 : field === 'operation_id' ? 'setConcertEventPhase' : uid(99) }
  f.setReceipt(bad); await f.controller.checkStatus(); expect(f.controller.read().command?.stage).toBe('unknown'); expect(prepare(f)).toBe(false)
})
it.each(['retired', 'expired', 'missing'] as const)('never unlocks replay for %s status', async kind => {
  const f = make(); await load(f); prepare(f); f.setMode('pending'); await f.controller.confirm()
  const r = f.getReceipt()!
  f.setReceipt(kind === 'missing' ? null : kind === 'retired' ? { ...r, state: 'RETIRED' } : { ...r, expires_at: time(-1), created_at: time(-2) })
  await f.controller.checkStatus(); await load(f); expect(prepare(f)).toBe(false); await f.controller.confirm(); expect(f.writes()).toHaveLength(1)
})
it('does not accept a fabricated direct success with the wrong revision or unapplied fields', async () => {
  const f = make(); await load(f); prepare(f); f.setMode('bad-result'); await f.controller.confirm()
  expect(f.controller.read().command?.stage).toBe('unknown'); expect(f.controller.read().settings?.revision).toBe(2)
})
it.each(['read', 'write'] as const)('fences a delayed %s result after permission/identity withdrawal', async kind => {
  const f = make(); await load(f)
  const gate = deferred<Response>(); let reached = false
  f.intercept(request => { if ((kind === 'read' && request.url.endsWith('/settings')) || (kind === 'write' && request.method !== 'GET')) { reached = true; return gate.promise }; return null })
  if (kind === 'write') prepare(f)
  const work = kind === 'read' ? f.controller.loadEvent() : f.controller.confirm()
  await vi.waitFor(() => expect(reached).toBe(true)); replaceIdentity(f)
  gate.resolve(Response.json({ ...f.current().settings, revision: 3, event: { ...f.current().settings.event, revision: 3 } })); await work
  expect(f.controller.read()).toMatchObject({ access: 'withdrawn', settings: null, command: null, notices: [] })
})
it.each(['future', 'http', 'credentials', 'invalid-date'] as const)('rejects %s notice source/confirmation before dispatch', async kind => {
  const f = make(); await load(f); const body = noticeInput()
  if (kind === 'future') body.confirmed_at = time(1000)
  if (kind === 'http') body.source_uri = 'http://notice.example.invalid'
  if (kind === 'credentials') body.source_uri = 'https://user:secret@notice.example.invalid'
  if (kind === 'invalid-date') body.confirmed_at = '2026-02-30T00:00:00Z'
  expect(f.controller.prepareNotice(body)).toBe(false); expect(f.writes()).toHaveLength(0)
})
it('publishes a source-stamped notice and explicitly withdraws an existing one with CAS', async () => {
  const f = make(); await load(f); expect(f.controller.prepareNotice(noticeInput())).toBe(true); await f.controller.confirm()
  expect(f.controller.read().command?.stage).toBe('applied'); await load(f)
  const n = f.current().notices[0]!
  expect(f.controller.prepareNotice({ ...dataFromNotice(n), expected_revision: n.revision, state: 'WITHDRAWN' }, n.notice_id)).toBe(true)
  await f.controller.confirm(); expect(f.controller.read().notices[0]?.state).toBe('WITHDRAWN'); expect(f.writes()).toHaveLength(2)
})
it('fails closed for absent place write producer and does not send a speculative transport', async () => {
  const f = make({ placePort: false }); await f.controller.loadPlaces(); const p = f.current().place
  expect(f.controller.preparePlace({ ...p, expected_revision: p.revision, reason: 'SYNTHETIC closure' })).toBe(false); expect(f.writes()).toHaveLength(0)
})
it('preserves immutable provenance for closure-only edits and requires a separately reviewed release for other edits', async () => {
  const f = make(); await f.controller.loadPlaces(); const p = f.current().place
  const changed = { ...p.place, accessibility_notes: 'SYNTHETIC revised access' }
  expect(f.controller.preparePlace({ place: changed, expected_revision: p.revision, reason: 'SYNTHETIC source review' })).toBe(false)
  f.setRelease({ ...f.release()!, targets: [{ placeId: p.place.place_id, expectedRevision: p.revision, reviewedPlace: changed }] })
  expect(f.controller.preparePlace({ place: changed, expected_revision: p.revision, reason: 'SYNTHETIC reviewed new release' })).toBe(true)
  await f.controller.confirm(); expect(f.controller.read().command?.stage).toBe('applied')
  expect(f.current().place.place.provenance).toEqual(p.place.provenance)
})
it('checks place scope/map expiry/current revision at confirmation, not only when the form opens', async () => {
  const f = make(); await f.controller.loadPlaces(); const p = f.current().place
  expect(f.controller.preparePlace({ place: { ...p.place, closure_state: 'CLOSED' }, expected_revision: p.revision, reason: 'SYNTHETIC closure' })).toBe(true)
  f.setRelease(null); await f.controller.confirm(); expect(f.writes()).toHaveLength(0)
})
it('fences place results after the qualified write release is withdrawn in flight', async () => {
  const f = make(); await f.controller.loadPlaces(); const p = f.current().place, gate = deferred<Response>(); let reached = false
  f.intercept(request => { if (request.method === 'PUT') { reached = true; return gate.promise }; return null })
  f.controller.preparePlace({ place: { ...p.place, closure_state: 'CLOSED' }, expected_revision: p.revision, reason: 'SYNTHETIC closure' })
  const work = f.controller.confirm(); await vi.waitFor(() => expect(reached).toBe(true)); f.setRelease(null)
  gate.resolve(Response.json({ revision: 5, place: { ...p.place, closure_state: 'CLOSED' } })); await work
  expect(f.controller.read().command?.stage).toBe('unknown'); expect(f.controller.read().places[0]?.place.closure_state).toBe('OPEN')
})
it('loads only minimal audit metadata, local detail, validated filters and no private URLs', async () => {
  const f = make(); await f.controller.loadAudit({ outcome: 'SUCCEEDED', actor_id: actorId }); f.controller.selectAudit(uid(50))
  expect(f.controller.read().auditDetail?.operation_id).toBe('setConcertSettings')
  expect(f.requests).toHaveLength(1); expect(f.requests[0]!.url).toContain('outcome=SUCCEEDED')
  expect(JSON.stringify(f.controller.read().audit)).not.toMatch(/text|source_uri|media/)
  await f.controller.loadAudit({ from: time(), until: time(-1) }); expect(f.requests).toHaveLength(1)
})
it.each(['extra-private', 'foreign-event', 'unknown-operation'] as const)('redacts or rejects %s audit payloads', async kind => {
  const f = make()
  f.intercept(request => request.url.includes('/audit') ? Response.json({ items: [{ audit_id: uid(50), event_id: kind === 'foreign-event' ? uid(99) : eventId, actor_id: actorId,
    operation_id: kind === 'unknown-operation' ? 'https://private.invalid/secret' : 'setConcertSettings', target_id: eventId, outcome: 'SUCCEEDED', receipt_id: uid(80), created_at: time(),
    ...(kind === 'extra-private' ? { report_text: 'PRIVATE report', media_uri: 'https://private.invalid/secret' } : {}) }], next_cursor: null }) : null)
  await f.controller.loadAudit(); expect(JSON.stringify(f.controller.read())).not.toContain('private.invalid')
  if (kind === 'unknown-operation') expect(f.controller.read().audit[0]?.operation_id).toBe('Other operation')
  else expect(f.controller.read().audit).toHaveLength(0)
})
it('rejects malformed receipt state invariants without retaining raw payloads', () => {
  const binding = { actor: actorId, event: eventId, operation: 'setConcertSettings' as const, key, target: eventId, revision: 2 }
  expect(() => boundReceipt({ receipt_id: uid(80), event_id: eventId, actor_id: actorId, operation_id: binding.operation, idempotency_key: key,
    state: 'PENDING', resource_id: eventId, resource_revision: null, failure_reason: null, result_status: null, created_at: time(), expires_at: time(1000) }, binding, Date.parse(time()))).toThrow()
})
it('keeps a recorded FAILED receipt locked after explicit refresh and sends no replacement command', async () => {
  const f = make(); await load(f); prepare(f); f.setMode('pending'); await f.controller.confirm()
  f.setReceipt({ ...f.getReceipt()!, state: 'FAILED', failure_reason: 'DEPENDENCY_UNAVAILABLE' })
  await f.controller.checkStatus(); await load(f); expect(f.controller.read().command?.stage).toBe('failed')
  expect(prepare(f)).toBe(false); expect(f.writes()).toHaveLength(1)
})
it('preserves uncertainty across a hidden/missing mutation target and never replaces its key', async () => {
  const f = make(); await load(f); prepare(f)
  f.intercept(request => request.method !== 'GET' ? errorResponse(404, 'RESOURCE_UNAVAILABLE') : null)
  await f.controller.confirm(); await load(f); expect(f.controller.read().command?.stage).toBe('unknown'); expect(prepare(f)).toBe(false)
})
it('uses explicit cursor-bound audit pages and rejects next-page filter changes and foreign rows', async () => {
  const f = make(); let page = 0
  f.intercept(request => {
    if (!request.url.includes('/audit')) return null
    const url = new URL(request.url); page++
    return Response.json({ items: [{ audit_id: uid(page === 1 ? 52 : 51), event_id: eventId, actor_id: actorId, operation_id: 'setConcertSettings', target_id: eventId,
      outcome: 'SUCCEEDED', receipt_id: uid(80), created_at: time(-1000 * page) }], next_cursor: page === 1 ? 'SYNTHETIC_cursor' : null,
      ...(url.searchParams.get('outcome') === 'FAILED' ? { invalid: true } : {}) })
  })
  await f.controller.loadAudit({ actor_id: actorId }); expect(f.controller.read().auditCursor).toBe('SYNTHETIC_cursor')
  await f.controller.loadAudit({ outcome: 'FAILED' }, true); expect(f.requests).toHaveLength(1)
  await f.controller.loadAudit({ actor_id: actorId }, true); expect(f.requests[1]!.url).toContain('cursor=SYNTHETIC_cursor')
  expect(f.controller.read().audit[0]?.audit_id).toBe(uid(51)); expect(f.controller.read().auditCursor).toBeNull()
})
it('rejects an audit row outside the requested actor/time/outcome filter', async () => {
  const f = make(); await f.controller.loadAudit({ actor_id: uid(99), outcome: 'FAILED', from: time() })
  expect(f.controller.read().audit).toHaveLength(0)
})
it('fails closed when an external authority producer throws and releases partial subscriptions', () => {
  const f = make(); let stopped = 0
  const controller = createOperationsController({ auth: f.auth, authority: { read: () => f.lease(), subscribe() { throw new Error('SYNTHETIC producer failure') } },
    clock: { read: () => ({ scope: f.scope, nowMs: Date.parse(time()) }), subscribe: () => () => { stopped++ } } })
  expect(controller.read().access).toBe('withdrawn'); controller.dispose(); expect(stopped).toBe(0)
})
it.each(['scope', 'expiry', 'map', 'revision'] as const)('rejects a mismatched %s place release without dispatch', async kind => {
  const f = make(); await f.controller.loadPlaces(); const p = f.current().place, release = f.release()!
  f.setRelease({ ...release, ...(kind === 'scope' ? { scope: { ...release.scope, eventId: uid(99) } } : kind === 'expiry' ? { expiresAt: time() }
    : kind === 'map' ? { mapRevision: 0 } : { targets: [{ placeId: p.place.place_id, expectedRevision: 99, reviewedPlace: null }] }) })
  expect(f.controller.preparePlace({ place: { ...p.place, closure_state: 'CLOSED' }, expected_revision: p.revision, reason: 'SYNTHETIC closure' })).toBe(false)
  expect(f.writes()).toHaveLength(0)
})
it('rejects future source review time even when a new-place release was supplied', async () => {
  const f = make(); await f.controller.loadPlaces(); const p = f.current().place
  const changed = { ...p.place, provenance: { ...p.place.provenance, reviewed_at: time(1000) } }
  f.setRelease({ ...f.release()!, targets: [{ placeId: p.place.place_id, expectedRevision: p.revision, reviewedPlace: changed }] })
  expect(f.controller.preparePlace({ place: changed, expected_revision: p.revision, reason: 'SYNTHETIC review' })).toBe(false)
})
it('fences a pending response after policy withdrawal and removes its safe input from snapshots', async () => {
  const f = make(); await load(f); prepare(f); const gate = deferred<Response>(); let reached = false
  f.intercept(request => { if (request.method !== 'GET') { reached = true; return gate.promise }; return null })
  const work = f.controller.confirm(); await vi.waitFor(() => expect(reached).toBe(true)); f.auth.privacy.changePolicy(f.scope, 'block')
  gate.resolve(errorResponse(503, 'DEPENDENCY_UNAVAILABLE')); await work
  expect(f.controller.read()).toMatchObject({ access: 'withdrawn', settings: null, command: null })
  expect(f.subscriptions()).toEqual({ authority: 0, clock: 0, places: 0 })
})
