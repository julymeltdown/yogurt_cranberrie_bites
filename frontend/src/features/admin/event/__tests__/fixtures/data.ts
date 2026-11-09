import { createSessionController } from '../../../../auth/model/session'
import { withAdmission } from '../../../../auth/__tests__/fixtures/admission'
import type { PrivacyScope } from '../../../../../shared/query/privacy'
import { createOperationsController } from '../../model/controller'
import { dataFromNotice } from '../../model/controller'
import type { CreateNotice, Notice, Operation, Receipt, Settings } from '../../model/contract'
import type { OperatorLease, PlaceWritePort, PlaceWriteRelease } from '../../model/ports'
import type { PlaceAdmin, SetPlace } from '../../../places/contract'
export { deferred } from '../../../../../test-support/fixtures'

export const uid = (n: number) => `00000000-0000-4000-8000-${n.toString(16).padStart(12, '0')}`
export const actorId = uid(1), eventId = uid(2), placeId = uid(3), noticeId = uid(4), key = uid(5)
export const now = Date.parse('2026-11-04T04:00:00Z')
export const time = (delta = 0) => new Date(now + delta).toISOString()
export function settings(): Settings { return { event: { event_id: eventId, slug: 'synthetic-event', title: 'SYNTHETIC operations event', phase: 'DRAFT', timezone: 'Asia/Seoul', revision: 2 }, revision: 2,
  features: { parties: true, giveaways: true, intros: true, social: true, mural: true }, beacon_ttl_seconds: 900, pickup_grace_seconds: 300, social_grant_ttl_seconds: 3600, mural_cooldown_ms: 1000 } }
export function place(): PlaceAdmin { return { revision: 4, place: { place_id: placeId, label: 'SYNTHETIC landmark', place_kind: 'PHYSICAL', verification_state: 'VERIFIED', closure_state: 'OPEN',
  accessibility_notes: 'SYNTHETIC access notes; no real venue claim.', provenance: { source_id: 'synthetic_review', source_kind: 'PLACE_REFERENCE', source_revision: 'SYNTHETIC release 1',
    source_uri: 'https://venue.example.invalid/review', author: 'SYNTHETIC author', license: 'SYNTHETIC fixture only', review_state: 'REVIEWED', reviewed_by: 'SYNTHETIC reviewer',
    reviewed_at: time(-86400000), photo_permission: 'DENIED' } } } }
export function notice(): Notice { return { notice_id: noticeId, event_id: eventId, revision: 3, title: 'SYNTHETIC public notice', text: 'SYNTHETIC reviewed public text.',
  source_uri: 'https://notice.example.invalid/source', confirmed_at: time(-60000), created_at: time(-120000), state: 'PUBLISHED' } }
export const errorResponse = (status: number, reason: string) => Response.json({ code: 'SYNTHETIC_FAILURE', message: 'Raw sensitive failure text must never render', details: { reason,
  ...(reason === 'RATE_LIMITED' ? { retry_after_ms: 5000 } : {}) } }, { status })
export type Mode = 'success' | 'pending' | 'lost' | 'forbidden' | 'conflict' | 'read-only' | 'rate' | 'unavailable' | 'bad-result'
export function fixture(options: { role?: OperatorLease['role']; absent?: boolean; placePort?: boolean } = {}) {
  let currentNow = now, mode: Mode = 'success', currentSettings = settings(), currentPlace = place(), currentNotices = [notice()], nextKey = 5
  const requests: Request[] = [], authorityListeners = new Set<() => void>(), clockListeners = new Set<() => void>(), placeListeners = new Set<() => void>()
  let lease: OperatorLease | null = null, release: PlaceWriteRelease | null = null, lastReceipt: Receipt | null = null
  let override: ((request: Request) => Promise<Response> | Response | null) | null = null
  let pending: { operation: Operation; body: Record<string, unknown>; target: string | null; key: string } | null = null
  function apply(operation: Operation, body: Record<string, unknown>, target: string | null): unknown {
    if (operation === 'setConcertSettings') {
      const { expected_revision: _rev, reason: _reason, ...values } = body
      currentSettings = { ...currentSettings, ...values, revision: currentSettings.revision + 1, event: { ...currentSettings.event, revision: currentSettings.revision + 1 } }
      return currentSettings
    }
    if (operation === 'setConcertEventPhase') {
      currentSettings = { ...currentSettings, revision: currentSettings.revision + 1,
        event: { ...currentSettings.event, revision: currentSettings.revision + 1, phase: body.phase as Settings['event']['phase'] } }; return currentSettings
    }
    if (operation === 'setConcertPlace') { currentPlace = { place: (body as unknown as SetPlace).place, revision: currentPlace.revision + 1 }; return currentPlace }
    const fields = dataFromNotice(body as unknown as Notice)
    const existing = currentNotices.find(n => n.notice_id === target)
    const result: Notice = { ...fields, notice_id: target ?? uid(40), event_id: eventId, created_at: existing?.created_at ?? time(), revision: (existing?.revision ?? 0) + 1 }
    currentNotices = [result, ...currentNotices.filter(n => n.notice_id !== result.notice_id)]; return result
  }
  function receipt(operation: Operation, correlation: string, target: string | null, state: Receipt['state'] = 'PENDING', revision: number | null = null): Receipt {
    return { receipt_id: uid(80), actor_id: actorId, event_id: eventId, operation_id: operation, idempotency_key: correlation, state,
      resource_id: state === 'SUCCEEDED' ? target : null, resource_revision: state === 'SUCCEEDED' ? revision : null, failure_reason: null,
      result_status: state === 'SUCCEEDED' ? 200 : null, created_at: time(), expires_at: time(86400000) }
  }
  const auth = createSessionController({ eventId, timeoutMs: 2000, cookiePolicy: { available: true, onInvalidate: () => () => {}, run: (_kind, work) => work(), dispose() {} },
    fetch: withAdmission(async input => {
      const request = input as Request; requests.push(request.clone())
      const overridden = override?.(request); if (overridden) return overridden
      const url = new URL(request.url), path = url.pathname
      if (request.method === 'GET') {
        if (path.endsWith('/settings')) return Response.json(currentSettings)
        if (path.endsWith('/overview')) return Response.json({ event_id: eventId, revision: 2, health: 'DEGRADED', open_reports: 1, pending_jobs: 2 })
        if (path.endsWith('/notices')) return Response.json({ items: currentNotices, next_cursor: null })
        if (path.endsWith('/places')) return Response.json({ items: [currentPlace], next_cursor: null })
        if (path.endsWith('/audit')) return Response.json({ items: [{ audit_id: uid(50), event_id: eventId, actor_id: actorId, operation_id: 'setConcertSettings',
          target_id: eventId, outcome: 'SUCCEEDED', receipt_id: uid(80), created_at: time(-1000) }], next_cursor: null })
        if (path.endsWith('/commands')) return lastReceipt ? Response.json(lastReceipt) : errorResponse(404, 'RESOURCE_UNAVAILABLE')
        throw new Error('Unexpected synthetic read')
      }
      if (mode === 'forbidden') return errorResponse(403, 'PERMISSION_DENIED')
      if (mode === 'conflict') return errorResponse(409, 'VERSION_CONFLICT')
      if (mode === 'read-only') return errorResponse(409, 'EVENT_READ_ONLY')
      if (mode === 'rate') return errorResponse(429, 'RATE_LIMITED')
      if (mode === 'unavailable') return errorResponse(503, 'DEPENDENCY_UNAVAILABLE')
      const operation: Operation = path.endsWith('/settings') ? 'setConcertSettings' : path.endsWith('/phase') ? 'setConcertEventPhase'
        : path.includes('/places/') ? 'setConcertPlace' : request.method === 'POST' ? 'createConcertNotice' : 'updateConcertNotice'
      const body = await request.json() as Record<string, unknown>, target = operation === 'setConcertSettings' || operation === 'setConcertEventPhase' ? eventId
        : operation === 'setConcertPlace' ? placeId : operation === 'updateConcertNotice' ? path.split('/').at(-1)! : null
      const correlation = request.headers.get('Idempotency-Key')!
      if (mode === 'pending') { pending = { operation, body, target, key: correlation }; lastReceipt = receipt(operation, correlation, target); return Response.json(lastReceipt, { status: 202 }) }
      const value = apply(operation, body, target)
      if (mode === 'lost') { lastReceipt = receipt(operation, correlation, target ?? uid(40), 'SUCCEEDED', (value as { revision: number }).revision); throw new TypeError('SYNTHETIC lost response') }
      return Response.json(mode === 'bad-result' ? { ...(value as object), revision: 999 } : value)
    }) })
  auth.session.replace(0, { userId: actorId, eventId, accessToken: 'synthetic-memory-token' })
  const scope = auth.privacy.capture()
  lease = options.absent ? null : { scope, actorId, eventId, role: options.role ?? 'OPERATOR', revision: 1, checkedAt: time(), expiresAt: time(60000) }
  release = { scope, mapRevision: 7, expiresAt: time(60000), targets: [{ placeId, expectedRevision: 4, reviewedPlace: null }] }
  const placePort: PlaceWritePort = {
    read: () => release, subscribe(listener) { placeListeners.add(listener); return () => { placeListeners.delete(listener) } },
    async write(input) {
      const result = await auth.request({ access: 'protected', errorFamily: 'domain', domainOperation: 'setConcertPlace', method: 'PUT', path: `/api/concert/events/${eventId}/admin/places/${input.placeId}`,
        idempotencyKey: input.key, body: { json: input.body }, signal: input.signal, decode: (data, context) => ({ data, status: context.status as 200 | 202 }) })
      return { ...result.data, scope: input.scope }
    },
  }
  const controller = createOperationsController({ auth, authority: { read: () => lease,
    subscribe(listener) { authorityListeners.add(listener); return () => { authorityListeners.delete(listener) } } },
    clock: { read: () => ({ scope, nowMs: currentNow }), subscribe(listener) { clockListeners.add(listener); return () => { clockListeners.delete(listener) } } },
    ...(options.placePort === false ? {} : { places: placePort }), newKey: () => uid(nextKey++) })
  return { auth, controller, requests, scope,
    writes: () => requests.filter(r => r.method !== 'GET'), setMode(value: Mode) { mode = value },
    intercept(handler: typeof override) { override = handler },
    current: () => ({ settings: structuredClone(currentSettings), place: structuredClone(currentPlace), notices: structuredClone(currentNotices) }),
    serverSettings(value: Settings) { currentSettings = value },
    setLease(value: OperatorLease | null) { lease = value; authorityListeners.forEach(l => l()) }, lease: () => lease,
    advance(ms: number) { currentNow += ms; clockListeners.forEach(l => l()) },
    setReceipt(value: Receipt | null) { lastReceipt = value }, getReceipt: () => lastReceipt,
    setRelease(value: PlaceWriteRelease | null) { release = value; placeListeners.forEach(l => l()) }, release: () => release,
    settle() {
      if (!pending) return
      const result = apply(pending.operation, pending.body, pending.target) as { revision: number }
      lastReceipt = receipt(pending.operation, pending.key, pending.target ?? uid(40), 'SUCCEEDED', result.revision); pending = null
    },
    subscriptions: () => ({ authority: authorityListeners.size, clock: clockListeners.size, places: placeListeners.size }),
    dispose() { controller.dispose(); auth.dispose() },
  }
}
export const settingsInput = (value = settings()) => ({ expected_revision: value.revision, features: { ...value.features, mural: false }, beacon_ttl_seconds: value.beacon_ttl_seconds,
  pickup_grace_seconds: value.pickup_grace_seconds, social_grant_ttl_seconds: value.social_grant_ttl_seconds, mural_cooldown_ms: value.mural_cooldown_ms, reason: 'SYNTHETIC reviewed kill switch' })
export const noticeInput = (): CreateNotice => ({ title: 'SYNTHETIC new notice', text: 'SYNTHETIC safe public text', source_uri: 'https://notice.example.invalid/new', confirmed_at: time(-1000), state: 'PUBLISHED' })
export const replaceIdentity = (f: ReturnType<typeof fixture>) => f.auth.session.replace(f.auth.session.read().generation, { userId: uid(99), eventId: uid(98), accessToken: 'synthetic-replacement-token' })
export type { PrivacyScope }
