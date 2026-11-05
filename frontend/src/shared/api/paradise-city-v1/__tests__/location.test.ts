import { describe, expect, it } from 'vitest'
import { TransportError } from '../../errors'
import { createLocationClient, decodeLocationSnapshot, decodeLocationError, decodeLandmarkGrant,
  decodeLandmarkList, locationRequests, locationOperations, LocationUnavailableError,
  type LocationCanonicalTransport, type LocationRequestDescriptor, type LocationLease,
  type LocationCommandReceipt, type LocationWriteOperation, type LocationSnapshot } from '../location'
import type { HttpResult } from '../../http'

const E = '00000000-0000-0000-0000-000000000001'
const A = '00000000-0000-0000-0000-000000000002'
const F = '00000000-0000-0000-0000-000000000003'
const K = '00000000-0000-0000-0000-000000000004'
const R = '00000000-0000-0000-0000-000000000005'
const T = '2026-11-05T02:30:00Z'
const X = '2026-11-05T02:40:00Z'
const scope = Object.freeze({ generation: 4, userId: A, eventId: E, policyGeneration: 7, gate: 'open' as const })
const context = { scope, status: 200, path: `/api/concert/events/${E}/venue/location` }
const selection = { landmark_id: 'Provider.Gate~4', catalog_revision: 2, landmark_revision: 3 }
function landmark() {
  return { event_id: E, selection: { ...selection }, provider_id: 'fixture-provider', source_revision: 'fixture-source-r1',
    venue_key: 'fixture-venue', map_id: 'fixture-map', map_revision: 4, policy_revision: 5, label: 'Fixture gate',
    review_reference: 'fixture-review', reviewed_at: '2026-11-04T00:00:00Z', server_time: T, valid_until: X,
    physical_binding: null, presentation_id: null, can_check_in: false, can_meet: false, can_note: false }
}
function friend() {
  return { user_id: F, source: 'UNAVAILABLE' as const, position: null, landmark: null, observed_at: null,
    fresh_until: null, expires_at: null, access_until: X, visibility_revision: 3,
    physical_arrival_verified: false as const, can_request_location: true }
}
function request() {
  return { request_id: R, event_id: E, requester_id: F, subject_id: A, revision: 6,
    state: 'PENDING' as const, created_at: T, expires_at: X }
}
function snapshot(): LocationSnapshot {
  return { event_id: E, actor_id: A, server_time: T, access_until: X,
    policy: { event_id: E, venue_key: 'fixture-venue', provider_id: 'fixture-provider', source_revision: 'fixture-r1',
      revision: 5, timezone: 'Asia/Seoul', valid_until: X, fresh_ms: 10000, last_known_ms: 60000,
      fix_max_age_ms: 30000, future_skew_ms: 0, sharing_lease_ms: 90000, request_ttl_ms: 90000,
      check_in_ttl_ms: 90000, coordinate_retention_ms: 120000, command_retention_ms: 120000,
      max_accuracy_m: 100, max_viewers: 50 },
    clock: { event_id: E, venue_key: 'fixture-venue', policy_revision: 5, timezone: 'Asia/Seoul',
      server_time: T, end_of_day: '2026-11-05T15:00:00Z', valid_until: X },
    sharing: { state: 'OFF', revision: 0, viewer_ids: [], expires_at: null, updated_at: null },
    self_location: null, friends: [friend()], capabilities: { can_share_phone: false, can_pause: false,
      can_revoke: false, check_in_landmark_ids: [], geographic_projection: 'UNAVAILABLE' },
    incoming_requests: [request()], outgoing_requests: [] }
}
function receipt(operation: LocationWriteOperation = 'requestVenueLocation'): LocationCommandReceipt {
  const kinds = { setVenueLocationSharing: 'SHARING', publishVenueLocationFix: 'PHONE_FIX',
    requestVenueLocation: 'LOCATION_REQUEST', decideVenueLocationRequest: 'LOCATION_REQUEST', setVenueLocationCheckIn: 'CHECK_IN' } as const
  return { command_key: K, operation, state: 'SUCCEEDED', resource_kind: kinds[operation], resource_id: R,
    revision: 7, created_at: T, expires_at: X }
}
function harness(payload: unknown = snapshot()) {
  let current = true, now = Date.parse(T)
  const abort = new AbortController()
  const calls: LocationRequestDescriptor<unknown>[] = []
  const signals: AbortSignal[] = []
  const lease: LocationLease = { scope, signal: abort.signal, isCurrent: s => current && s.policyGeneration === 7,
    serverNow: () => now }
  const transport: LocationCanonicalTransport = {
    async request<T>(req: LocationRequestDescriptor<T>, signal: AbortSignal): Promise<HttpResult<T>> {
      calls.push(req); signals.push(signal)
      return { data: req.decode(payload, { scope, status: 200, path: req.path }), scope }
    },
  }
  return { lease, transport, calls, signals, abort, revoke: () => { current = false }, setNow: (v: number) => { now = v } }
}

// All providers, dates, actors, coordinates and authorities below are deterministic
// synthetic wire fixtures, never a production clock/landmark/Friends permission.
describe('Location1.0.1 closed wire representations', () => {
  it('requires real inbox metadata and returns immutable copies', () => {
    const raw = snapshot(), parsed = decodeLocationSnapshot(raw, context)
    expect(parsed).toEqual(raw); expect(parsed).not.toBe(raw)
    expect(Object.isFrozen(parsed.incoming_requests[0])).toBe(true)
    expect(Object.isFrozen(parsed.policy)).toBe(true)
    expect(parsed.incoming_requests[0]?.request_id).toBe(R)
  })
  it('rejects either missing required request array and unknown root/nested fields', () => {
    for (const key of ['incoming_requests', 'outgoing_requests']) {
      const raw = { ...snapshot() } as Record<string, unknown>; delete raw[key]
      expect(() => decodeLocationSnapshot(raw, context)).toThrow(TransportError)
    }
    expect(() => decodeLocationSnapshot({ ...snapshot(), hidden_count: 12 }, context)).toThrow()
    expect(() => decodeLocationSnapshot({ ...snapshot(), policy: { ...snapshot().policy, token: 'secret' } }, context)).toThrow()
  })
  it('accepts exactly50 current-friend request rows, rejects51 and duplicate identity', () => {
    const rows = Array.from({ length: 50 }, (_, i) => ({ ...request(), state: 'DENIED' as const,
      request_id: `00000000-0000-0000-0000-${(200-i).toString(16).padStart(12,'0')}` }))
    expect(decodeLocationSnapshot({ ...snapshot(), incoming_requests: rows }, context).incoming_requests).toHaveLength(50)
    expect(() => decodeLocationSnapshot({ ...snapshot(), incoming_requests: [...rows, request()] }, context)).toThrow()
    expect(() => decodeLocationSnapshot({ ...snapshot(), incoming_requests: [request(),request()] }, context)).toThrow()
  })
  it('fences actor/event, inbox direction and returned authorized friend identities', () => {
    for (const raw of [ { ...snapshot(), actor_id: F }, { ...snapshot(), event_id: F },
      { ...snapshot(), friends: [] }, { ...snapshot(), incoming_requests: [{ ...request(), subject_id: F }] },
      { ...snapshot(), outgoing_requests: [request()], incoming_requests: [] },
      { ...snapshot(), friends: [friend(),friend()] }, { ...snapshot(), friends: [{ ...friend(), user_id: A }] } ]) {
      expect(() => decodeLocationSnapshot(raw, context)).toThrow()
    }
  })
  it('rejects expired PENDING, order violations and unsafe/coerced revisions', () => {
    expect(() => decodeLocationSnapshot({ ...snapshot(), incoming_requests: [{ ...request(), expires_at: T }] }, context)).toThrow()
    expect(() => decodeLocationSnapshot({ ...snapshot(), incoming_requests: [request(), { ...request(), request_id: K, created_at: '2026-11-05T02:31:00Z' }] }, context)).toThrow()
    expect(() => decodeLocationSnapshot({ ...snapshot(), incoming_requests: [{ ...request(), revision: '6' }] }, context)).toThrow()
    expect(() => decodeLocationSnapshot({ ...snapshot(), incoming_requests: [{ ...request(), revision: Number.MAX_SAFE_INTEGER+1 }] }, context)).toThrow()
    expect(decodeLocationSnapshot({ ...snapshot(), incoming_requests: [{ ...request(), state: 'EXPIRED' }] }, context).incoming_requests[0]?.state).toBe('EXPIRED')
  })
  it('never promotes phone/check-in/art to verified physical arrival or projection', () => {
    expect(() => decodeLocationSnapshot({ ...snapshot(), friends: [{ ...friend(), physical_arrival_verified: true }] }, context)).toThrow()
    expect(() => decodeLocationSnapshot({ ...snapshot(), capabilities: { ...snapshot().capabilities, geographic_projection: 'CALIBRATED' } }, context)).toThrow()
    expect(() => decodeLocationSnapshot({ ...snapshot(), friends: [{ ...friend(), source: 'PHONE' }] }, context)).toThrow()
    const phone = { ...friend(), source: 'PHONE', position: { latitude: 37, longitude: 127, accuracy_m: 2, captured_at: T, received_at: T } }
    expect(decodeLocationSnapshot({ ...snapshot(), friends: [phone] }, context).friends[0]?.position?.latitude).toBe(37)
    expect(() => decodeLocationSnapshot({ ...snapshot(), friends: [{ ...phone, position: { ...phone.position, latitude: Infinity } }] }, context)).toThrow()
  })
  it('rejects invalid RFC3339 calendar values, policy ordering and stale representations', () => {
    for (const timestamp of ['2026-02-30T00:00:00Z','2026-11-05','2026-11-05T02:30:00','2026-11-05T25:00:00Z']) {
      expect(() => decodeLocationSnapshot({ ...snapshot(), server_time: timestamp }, context)).toThrow()
    }
    expect(() => decodeLocationSnapshot({ ...snapshot(), policy: { ...snapshot().policy, fresh_ms: 70000 } }, context)).toThrow()
    expect(() => decodeLocationSnapshot({ ...snapshot(), access_until: T }, context)).toThrow()
  })
  it('preserves opaque provider IDs and rejects landmark traversal or cross-event grants', () => {
    const grant = decodeLandmarkGrant(landmark(), context)
    expect(grant.selection.landmark_id).toBe('Provider.Gate~4'); expect(grant.physical_binding).toBeNull()
    expect(() => decodeLandmarkGrant({ ...landmark(), event_id: F }, context)).toThrow()
    expect(() => decodeLandmarkList({ event_id: E, server_time:T, access_until:X, items:[landmark(),landmark()] },context)).toThrow()
    for (const id of ['.','..','../gate','gate/path','%2f','gate?x=1']) expect(() => locationRequests.landmark(E,id)).toThrow()
  })
  it('rejects getters, sparse arrays, symbols and implicit ID coercion', () => {
    let touched = false
    const raw = { ...snapshot() }; Object.defineProperty(raw,'actor_id',{enumerable:true,get(){touched=true;return A}})
    expect(() => decodeLocationSnapshot(raw,context)).toThrow(); expect(touched).toBe(false)
    const sparse: unknown[] = []; sparse.length = 1
    const a = snapshot(); expect(() => decodeLocationSnapshot({ ...a, incoming_requests: sparse },context)).toThrow()
    expect(() => decodeLocationSnapshot({ ...a, [Symbol('extra')]: 1 },context)).toThrow()
    const id = { toString(){touched=true;return E} }
    expect(() => locationRequests.snapshot(id as never)).toThrow(); expect(touched).toBe(false)
  })
  it('decodes the qualified error envelope without extending canonical auth/HTTP ownership', () => {
    const error = { code:'LOCATION_UNAVAILABLE',message:'Unavailable',traceId:K,timestamp:T,details:{reason:'RESOURCE_UNAVAILABLE'} }
    expect(decodeLocationError(error)).toEqual(error)
    expect(() => decodeLocationError({ ...error, details:{reason:'HIDDEN_LOCATION',latitude:37} })).toThrow()
  })
})

describe('exact declarative requests', () => {
  it('emits exactly4GET/5writes with protected no-store and no automatic refresh or invented keys', () => {
    const descriptors = [locationRequests.snapshot(E),locationRequests.landmarks(E),locationRequests.landmark(E,selection.landmark_id),
      locationRequests.command(E,'requestVenueLocation',K),locationRequests.setSharing(E,{expected_revision:0,state:'ACTIVE',viewer_ids:[F]},K),
      locationRequests.publishFix(E,{expected_sharing_revision:1,source:'DEVICE_GEOLOCATION_REPORTED',latitude:37,longitude:127,accuracy_m:2,captured_at:T},K),
      locationRequests.requestLocation(E,{subject_id:F},K),locationRequests.decideRequest(E,R,{expected_revision:6,decision:'APPROVE'},K),
      locationRequests.checkIn(E,{expected_sharing_revision:1,landmark:selection},K)]
    expect(Object.keys(locationOperations)).toHaveLength(9)
    expect(descriptors.filter(d=>d.method==='GET')).toHaveLength(4)
    for (const d of descriptors) {
      expect(d.access).toBe('protected');expect(d.cache).toBe('no-store');expect(d.errorFamily).toBe('domain')
      expect(Object.isFrozen(d)).toBe(true);expect(d).not.toHaveProperty('refresh');expect(d).not.toHaveProperty('domainOperation')
      expect(d.path).toContain(`/api/concert/events/${E}/venue/`)
      if(d.method!=='GET')expect(d.idempotencyKey).toBe(K)
    }
    expect(descriptors[3]?.query).toEqual({operation:'requestVenueLocation'})
    expect(descriptors[7]?.path.endsWith(`/requests/${R}/decision`)).toBe(true)
  })
  it('rejects unknown request fields, nonfinite fixes, consent conflation, duplicate viewers and absent keys', () => {
    expect(() => locationRequests.setSharing(E,{expected_revision:1,state:'PAUSED',viewer_ids:[F]},K)).toThrow()
    expect(() => locationRequests.setSharing(E,{expected_revision:0,state:'ACTIVE',viewer_ids:[F,F]},K)).toThrow()
    expect(() => locationRequests.publishFix(E,{expected_sharing_revision:1,source:'DEVICE_GEOLOCATION_REPORTED',latitude:NaN,longitude:127,accuracy_m:2,captured_at:T},K)).toThrow()
    expect(() => locationRequests.requestLocation(E,{subject_id:F,actor_id:A} as never,K)).toThrow()
    expect(() => locationRequests.requestLocation(E,{subject_id:F},undefined as never)).toThrow()
    expect(() => locationRequests.command(E,'invented' as never,K)).toThrow()
  })
  it('copies and seals bodies; status binds original exact operation/key; receipts are not inbox rows', () => {
    const body = {expected_revision:0,state:'ACTIVE' as const,viewer_ids:[F]}
    const d = locationRequests.setSharing(E,body,K);body.viewer_ids.length=0
    expect(d.body?.json).toEqual({expected_revision:0,state:'ACTIVE',viewer_ids:[F]})
    const status = locationRequests.command(E,'requestVenueLocation',K)
    const c = {...context,path:status.path}
    expect(status.decode(receipt(),c).command_key).toBe(K)
    expect(() => status.decode(receipt('decideVenueLocationRequest'),c)).toThrow()
    expect(() => status.decode({...receipt(),command_key:R},c)).toThrow()
    expect(() => decodeLocationSnapshot(receipt(),context)).toThrow()
  })
})

describe('injected canonical transport privacy and signal fences', () => {
  it('fails unbound transport closed without fetch or dispatch', async () => {
    const h=harness();const client=createLocationClient(null,h.lease)
    await expect(client.getSnapshot()).rejects.toBeInstanceOf(LocationUnavailableError)
    expect(h.calls).toHaveLength(0)
  })
  it('dispatches once, passes the exact caller signal and returns canonical scope only', async () => {
    const h=harness();const r=await createLocationClient(h.transport,h.lease).getSnapshot()
    expect(h.calls).toHaveLength(1);expect(h.signals[0]).toBe(h.abort.signal)
    expect(r.data.incoming_requests[0]?.revision).toBe(6)
    expect(r.scope).toEqual({generation:4,userId:A,eventId:E});expect(Object.isFrozen(r.data)).toBe(true)
  })
  it('rejects gate/policy/signal/clock failure before dispatch', () => {
    for(const lease of [{...harness().lease,scope:{...scope,gate:'suspended' as const}},
      {...harness().lease,isCurrent:()=>false},{...harness().lease,serverNow:()=>NaN}]) {
      const h=harness();expect(()=>createLocationClient(h.transport,lease).getSnapshot()).toThrow();expect(h.calls).toHaveLength(0)
    }
    const h=harness();h.abort.abort();expect(()=>createLocationClient(h.transport,h.lease).getSnapshot()).toThrow();expect(h.calls).toHaveLength(0)
  })
  it('rejects wrong returned scope and injected ports that skip closed decoding', async () => {
    const h=harness()
    const bad:LocationCanonicalTransport={async request<T>():Promise<HttpResult<T>>{return{data:snapshot() as T,scope:{generation:5,userId:A,eventId:E}}}}
    await expect(createLocationClient(bad,h.lease).getSnapshot()).rejects.toBeInstanceOf(TransportError)
    const extra:LocationCanonicalTransport={async request<T>():Promise<HttpResult<T>>{return{data:{...snapshot(),hidden_count:4} as T,scope}}}
    await expect(createLocationClient(extra,h.lease).getSnapshot()).rejects.toBeInstanceOf(TransportError)
  })
  it('fences delayed account/policy loss and preserves uncertain mutation semantics', async () => {
    const h=harness();let finish=()=>{}
    const delayed:LocationCanonicalTransport={request<T>():Promise<HttpResult<T>>{return new Promise(resolve=>{finish=()=>resolve({data:receipt() as T,scope})})}}
    const pending=createLocationClient(delayed,h.lease).requestLocation({subject_id:F},K)
    h.revoke();finish()
    await expect(pending).rejects.toMatchObject({kind:'session-changed',uncertain:true})
  })
  it('cancels a port ignoring the signal and discards its eventual response', async () => {
    const h=harness();let finish=()=>{}
    const delayed:LocationCanonicalTransport={request<T>():Promise<HttpResult<T>>{return new Promise(resolve=>{finish=()=>resolve({data:snapshot() as T,scope})})}}
    const pending=createLocationClient(delayed,h.lease).getSnapshot();h.abort.abort()
    await expect(pending).rejects.toMatchObject({kind:'aborted'});finish()
  })
  it('uses actual caller clock at delivery; never invents browser/device time', async () => {
    const h=harness();h.setNow(Date.parse(X))
    await expect(createLocationClient(h.transport,h.lease).getSnapshot()).rejects.toBeInstanceOf(TransportError)
    const h2=harness();const expired={...snapshot(),friends:[{...friend(),access_until:T}]}
    await expect(createLocationClient(harness(expired).transport,h2.lease).getSnapshot()).rejects.toBeInstanceOf(TransportError)
  })
  it('preserves canonical errors and explicit keys with no retry, refresh or status autosend', async () => {
    const h=harness();let calls=0
    const original=new TransportError('http',{status:409,uncertain:true})
    const transport:LocationCanonicalTransport={async request(){calls++;throw original}}
    await expect(createLocationClient(transport,h.lease).requestLocation({subject_id:F},K)).rejects.toBe(original)
    expect(calls).toBe(1)
  })
})
