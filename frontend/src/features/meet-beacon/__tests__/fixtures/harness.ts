// Conspicuously synthetic fixtures. No production venue, audience, photo or policy authority.
import fixture from '../../../../../../contracts/fixtures/domain/private-beacon.json'
import { createSessionController } from '../../../auth/model/session'
import { isolatedCookiePolicy } from '../../../auth/__tests__/fixtures/data'
import { admission } from '../../../auth/__tests__/enrollment-consent/fixtures'
import { createPlacesController } from '../../../places/model/controller'
import { places, release, eventId, userId } from '../../../places/__tests__/fixtures/data'
import { createMeetingBeaconController } from '../../model/controller'
import type { MeetingAuthority, QualifiedMeetingPhoto } from '../../model/ports'
import type { MeetBeacon, Receipt } from '../../model/contract'

export { eventId, userId }
export const beaconId = fixture.beacon_id, partyId = '00000000-0000-4000-8000-000000000250'
export const key = '00000000-0000-4000-8000-000000000551'
export function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(yes => { resolve = yes }); return { promise, resolve } }
export function sampleBeacon(): MeetBeacon {
  return { ...structuredClone(fixture), event_id: eventId, owner_id: userId, state: 'ACTIVE',
    meeting: { ...structuredClone(fixture.meeting), place_id: places()[1]!.place_id, place_revision: 3,
      instructions: 'SYNTHETIC: wait at a fictional marker. Not venue guidance.', identification: 'SYNTHETIC red scarf', audience: { kind: 'PARTY_MEMBERS', party_id: partyId } } }
}
export function sampleReceipt(operation = 'createConcertBeacon', state: Receipt['state'] = 'PENDING'): Receipt {
  return { receipt_id: '00000000-0000-4000-8000-000000000552', event_id: eventId, actor_id: userId, operation_id: operation,
    idempotency_key: key, state, resource_id: state === 'SUCCEEDED' ? beaconId : null, resource_revision: state === 'SUCCEEDED' ? 1 : null,
    failure_reason: state === 'FAILED' ? 'PERMISSION_DENIED' : null, created_at: '2026-11-07T08:00:00Z', expires_at: '2026-11-08T08:00:00Z', result_status: state === 'SUCCEEDED' ? 200 : null }
}
export async function harness(photoClient?: import('../../model/photo-client').MeetingPhotoConfiguration) {
  let time: number | null = Date.parse('2026-11-07T08:00:00Z'), beacon = sampleBeacon(), authority: MeetingAuthority | null = null, photo: QualifiedMeetingPhoto | null = null
  let mode: 'ok' | 'lost' | 'pending' | 'denied' | 'conflict' | 'missing' | 'server' = 'ok'
  let status: Receipt = sampleReceipt()
  let override: ((request: Request) => Response | Promise<Response> | null) | null = null
  const requests: Request[] = [], authorityListeners = new Set<() => void>(), photoListeners = new Set<() => void>()
  const items = places(), map = await release(items)
  const auth = createSessionController({ eventId, cookiePolicy: isolatedCookiePolicy(), fetch: async input => {
    const r = input as Request, path = new URL(r.url).pathname
    if (r.method === 'GET' && path === '/api/auth/enrollment/admission') return Response.json(admission())
    requests.push(r.clone())
    const custom = override?.(r); if (custom) return custom
    if (path.endsWith('/places')) return Response.json({ event_id: eventId, revision: 7, items })
    if (path.endsWith('/commands')) return Response.json(status)
    if (mode === 'denied') return Response.json({ code: 'DENIED', message: 'Synthetic denial', details: { reason: 'PERMISSION_DENIED' } }, { status: 403 })
    if (mode === 'missing') return Response.json({ code: 'UNAVAILABLE', message: 'Synthetic missing', details: { reason: 'RESOURCE_UNAVAILABLE' } }, { status: 404 })
    if (r.method === 'GET') return Response.json(beacon)
    if (mode === 'lost') throw new Error('Synthetic lost response')
    if (mode === 'server') return Response.json({}, { status: 503 })
    if (mode === 'conflict') return Response.json({ code: 'CONFLICT', message: 'Synthetic conflict', details: { reason: 'VERSION_CONFLICT' } }, { status: 409 })
    if (mode === 'pending') return Response.json(sampleReceipt(), { status: 202 })
    if (r.method === 'DELETE') { beacon = { ...beacon, state: 'REVOKED', revision: beacon.revision + 1 }; const { beacon_id, event_id, state, revision } = beacon; return Response.json({ beacon_id, event_id, state, revision }) }
    const body = await r.clone().json()
    beacon = { ...beacon, revision: r.method === 'POST' ? 1 : beacon.revision + 1, meeting: body.meeting }
    return Response.json(beacon)
  } })
  auth.session.replace(0, { userId, eventId, accessToken: 'synthetic-memory-token' })
  authority = { scope: auth.privacy.capture(), expiresAt: '2026-11-07T09:00:00Z', canRead: true, canWrite: true, maxTtlSeconds: 900,
    eventCutoff: '2026-11-07T10:00:00Z', audiences: [{ id: 'synthetic-party', label: 'SYNTHETIC approved party members', audience: { kind: 'PARTY_MEMBERS', party_id: partyId } }],
    placeRevisions: [{ placeId: items[1]!.place_id, revision: 3 }] }
  const placesController = createPlacesController({ session: auth.session, http: auth, map: { read: () => map, subscribe: () => () => {} } })
  placesController.start()
  for (let i = 0; i < 100 && placesController.read().list.phase !== 'ready'; i++) await new Promise(resolve => setTimeout(resolve, 0))
  if (placesController.read().list.phase !== 'ready') throw new Error('Synthetic FE19 initialization failed')
  const controller = createMeetingBeaconController({ auth, places: placesController, now: () => time, newKey: () => key,
    ...(photoClient ? { photoClient } : {}),
    authority: { read: () => authority, subscribe: fn => { authorityListeners.add(fn); return () => { authorityListeners.delete(fn) } } },
    photos: { read: () => photo, subscribe: fn => { photoListeners.add(fn); return () => { photoListeners.delete(fn) } } } })
  controller.start()
  return { controller, auth, placesController, requests, items, map,
    get beacon() { return beacon }, get authority() { return authority }, get time() { return time },
    now: () => time,
    setTime(value: number | null) { time = value; controller.refreshTime() },
    advanceClock(value: number | null) { time = value },
    setBeacon(value: MeetBeacon) { beacon = value }, setMode(value: typeof mode) { mode = value },
    setAuthority(value: MeetingAuthority | null) { authority = value; for (const fn of authorityListeners) fn() },
    setPhoto(value: QualifiedMeetingPhoto | null) { photo = value; for (const fn of photoListeners) fn() },
    setStatus(value: Receipt) { status = value }, setOverride(value: typeof override) { override = value },
    fill() { controller.setDraft({ placeId: items[1]!.place_id, audienceId: 'synthetic-party', instructions: beacon.meeting.instructions,
      identification: beacon.meeting.identification, ttlSeconds: 900, acknowledged: true }) },
    dispose() { controller.dispose(); placesController.dispose(); auth.dispose() },
  }
}
