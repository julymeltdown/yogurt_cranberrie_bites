import { execFileSync } from 'node:child_process'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { DOMAIN_OPERATIONS } from '../../domain-operations.generated'
import { createHttpClient, createMemorySession, type DecodeContext } from '../../http'
import { createHttpLocationTransport } from '../location-http'
import { createLocationClient, locationRequests, type LocationLease } from '../location'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../../../../../..')
const USER = '00000000-0000-4000-8000-000000000101'
const FRIEND = '00000000-0000-4000-8000-000000000102'
const EVENT = '00000000-0000-4000-8000-000000000201'
const OTHER = '00000000-0000-4000-8000-000000000202'
const KEY = '00000000-0000-4000-8000-000000000301'
const REQUEST = '00000000-0000-4000-8000-000000000401'
const NOW = '2030-01-01T00:00:00Z'
const UNTIL = '2030-01-01T00:10:00Z'
const selection = { landmark_id: 'zone.~west', catalog_revision: 2, landmark_revision: 3 }
const grant = { event_id: EVENT, selection, provider_id: 'fixture-provider', source_revision: 'fixture-v1',
  venue_key: 'fixture-venue', map_id: 'fixture-map', map_revision: 1, policy_revision: 1,
  label: 'Fixture landmark', review_reference: 'fixture-review', reviewed_at: NOW, server_time: NOW,
  valid_until: UNTIL, physical_binding: null, presentation_id: null, can_check_in: true, can_meet: true, can_note: true }
const receipt = { command_key: KEY, operation: 'setVenueLocationSharing', state: 'SUCCEEDED', resource_kind: 'SHARING',
  resource_id: REQUEST, revision: 1, created_at: NOW, expires_at: UNTIL }
const factories = [
  ['getVenueLocationSnapshot', () => locationRequests.snapshot(EVENT)],
  ['listVenueLandmarks', () => locationRequests.landmarks(EVENT)],
  ['getVenueLandmark', () => locationRequests.landmark(EVENT, selection.landmark_id)],
  ['getVenueLocationCommand', () => locationRequests.command(EVENT, 'setVenueLocationSharing', KEY)],
  ['setVenueLocationSharing', () => locationRequests.setSharing(EVENT, { expected_revision: 0, state: 'ACTIVE', viewer_ids: [FRIEND] }, KEY)],
  ['publishVenueLocationFix', () => locationRequests.publishFix(EVENT, { expected_sharing_revision: 1,
    source: 'DEVICE_GEOLOCATION_REPORTED', latitude: 37, longitude: 127, accuracy_m: 5, captured_at: NOW }, KEY)],
  ['requestVenueLocation', () => locationRequests.requestLocation(EVENT, { subject_id: FRIEND }, KEY)],
  ['decideVenueLocationRequest', () => locationRequests.decideRequest(EVENT, REQUEST, { expected_revision: 1, decision: 'APPROVE' }, KEY)],
  ['setVenueLocationCheckIn', () => locationRequests.checkIn(EVENT, { expected_sharing_revision: 1, landmark: selection }, KEY)],
] as const

function setup(handler: (request: Request) => Response | Promise<Response> = () => Response.json({ ok: true })) {
  const session = createMemorySession()
  session.replace(0, { userId: USER, eventId: EVENT, accessToken: 'fixture-token' })
  const requests: Request[] = []
  const fetch = vi.fn<typeof globalThis.fetch>(async input => {
    const request = input as Request
    requests.push(request.clone())
    return handler(request)
  })
  const http = createHttpClient({ session, fetch, timeoutMs: 100 })
  const transport = createHttpLocationTransport(http, session)
  const controller = new AbortController()
  return { session, fetch, http, transport, controller, requests }
}
function lease(fixture: ReturnType<typeof setup>, isCurrent = () => true): LocationLease {
  return { scope: { generation: fixture.session.read().generation, userId: USER, eventId: EVENT, policyGeneration: 1, gate: 'open' },
    signal: fixture.controller.signal, isCurrent, serverNow: () => Date.parse(NOW) }
}
beforeEach(() => vi.stubGlobal('location', { origin: 'https://mcr.example' }))
afterEach(() => { vi.unstubAllGlobals(); vi.restoreAllMocks() })

describe('actual additive helper projection, expected registration only', () => {
  const script = `
import runpy,json,sys,copy
from pathlib import Path
from unittest.mock import patch
root=Path(sys.argv[1]);m=runpy.run_path(str(root/'scripts/mcr/paradise-city-v1/location_transport.py'))
inputs={'legacy':'retained'};ops={'legacy':{'sentinel':True}};queries={'legacy':{'sentinel':True}};errors={'RATE_LIMITED':{'required':False},'LEGACY':None}
m['extend'](root,inputs,ops,queries,errors)
checks={}
for case in ['duplicate','queryCollision','inputCollision','hashDrift']:
 a,b,c,d=copy.deepcopy(inputs),copy.deepcopy(ops),copy.deepcopy(queries),copy.deepcopy(errors)
 if case!='duplicate':b.pop('getVenueLocationSnapshot')
 if case in ['queryCollision','inputCollision','hashDrift']:
  b={k:v for k,v in b.items() if k=='legacy'}
 if case in ['inputCollision','hashDrift']:
  b={k:v for k,v in b.items() if k=='legacy'};c={k:v for k,v in c.items() if k=='legacy'}
 if case=='inputCollision':a[m['PATH']]='wrong'
 before=copy.deepcopy([a,b,c,d])
 try:
  if case=='hashDrift':
   with patch('pathlib.Path.read_bytes',return_value=b'{}'):m['extend'](root,a,b,c,d)
  else:m['extend'](root,a,b,c,d)
  checks[case]=False
 except ValueError:checks[case]=[a,b,c,d]==before
print(json.dumps({'inputs':inputs,'operations':ops,'queries':queries,'errors':errors,'atomicRejections':checks}))
`
  function project() {
    return JSON.parse(execFileSync(process.env.MCR_LOCATION_PYTHON ?? 'python3', ['-c', script, root],
      { cwd: root, encoding: 'utf8', env: { ...process.env, PYTHONDONTWRITEBYTECODE: '1' } })) as {
      inputs: Record<string, string>; operations: Record<string, { sentinel?: boolean; idempotency?: boolean }>;
      queries: Record<string, { sentinel?: boolean; required?: string[]; properties?: Record<string, unknown> }>;
      errors: Record<string, unknown>; atomicRejections: Record<string, boolean>
    }
  }
  it('projects exactly nine routes/five idempotent writes and preserves existing rules', () => {
    const result = project()
    expect(Object.keys(result.operations)).toHaveLength(10)
    expect(Object.values(result.operations).filter(op => op.idempotency)).toHaveLength(5)
    expect(result.operations.legacy).toEqual({ sentinel: true })
    expect(result.queries.legacy).toEqual({ sentinel: true })
    expect(result.errors.RATE_LIMITED).toEqual({ required: false })
    expect(result.inputs['backend/docs/api/paradise-city-v1/location.openapi.json']).toBe('acbd0feaa10bdc742c8ce906050bf6a463b037f4a52c2c2e151827e78cefa32e')
    expect(result.queries['paradiseLocation.getVenueLocationCommand']?.required).toEqual(['operation'])
  })
  it('rejects operation/query/input collisions and frozen input drift without partial mutation', () => {
    expect(project().atomicRejections).toEqual({ duplicate: true, queryCollision: true, inputCollision: true, hashDrift: true })
  })
})

describe('actual canonical HttpClient with expected generated Location registry overlay', () => {
  it.each(factories)('dispatches %s exactly once with canonical scope/headers/decoder', async (_id, factory) => {
    const f = setup()
    const descriptor = factory()
    const decode = vi.fn((value: unknown, context: DecodeContext) => ({ value, context }))
    const result = await f.transport.request({ ...descriptor, decode }, f.controller.signal)
    expect(f.fetch).toHaveBeenCalledTimes(1)
    const request = f.requests[0]!
    expect(request.method).toBe(descriptor.method)
    expect(new URL(request.url).pathname).toBe(descriptor.path)
    expect(request.headers.get('Authorization')).toBe('Bearer fixture-token')
    expect(request.cache).toBe('no-store')
    expect(decode).toHaveBeenCalledTimes(1)
    expect(result.data.context).toEqual({ scope: { generation: 1, userId: USER, eventId: EVENT }, status: 200, path: descriptor.path })
    if (descriptor.method === 'GET') {
      expect(request.headers.get('Content-Type')).toBeNull()
      expect(request.headers.get('Idempotency-Key')).toBeNull()
      expect(await request.text()).toBe('')
    } else {
      expect(request.headers.get('Idempotency-Key')).toBe(KEY)
      expect(request.headers.get('X-MCR-Intent')).toBe('browser')
      expect(await request.json()).toEqual(descriptor.body?.json)
    }
    expect(new URL(request.url).search).toBe(descriptor.operationId === 'getVenueLocationCommand' ? '?operation=setVenueLocationSharing' : '')
  })

  it('fails unavailable before dispatch when actual registration is absent', async () => {
    const property = Object.getOwnPropertyDescriptor(DOMAIN_OPERATIONS, 'getVenueLandmark')!
    Reflect.deleteProperty(DOMAIN_OPERATIONS, 'getVenueLandmark')
    try {
      const f = setup()
      await expect(f.transport.request(locationRequests.landmark(EVENT, 'zone'), f.controller.signal)).rejects.toMatchObject({ kind: 'UNAVAILABLE' })
      expect(f.fetch).not.toHaveBeenCalled()
    } finally { Object.defineProperty(DOMAIN_OPERATIONS, 'getVenueLandmark', property) }
  })

  it.each(['.', '..', '%2e', '%7E', 'a/b', 'a?x=1', 'a#b', 'a+b', 'zone\n', 'zone\r', 'zone\t'])('rejects malformed landmark path %s before HTTP', async id => {
    const f = setup()
    const descriptor = locationRequests.landmark(EVENT, 'zone')
    await expect(f.transport.request({ ...descriptor, path: `/api/concert/events/${EVENT}/venue/landmarks/${id}` }, f.controller.signal))
      .rejects.toMatchObject({ kind: 'invalid-request' })
    expect(f.fetch).not.toHaveBeenCalled()
  })

  it('rejects a forged method, extra query and GET idempotency key before HTTP', async () => {
    const f = setup(), descriptor = locationRequests.landmark(EVENT, 'zone')
    for (const extra of [{ method: 'POST' as const }, { query: { extra: 'hidden' } }, { idempotencyKey: KEY }]) {
      await expect(f.transport.request({ ...descriptor, ...extra }, f.controller.signal)).rejects.toMatchObject({ kind: 'invalid-request' })
    }
    expect(f.fetch).not.toHaveBeenCalled()
  })

  it('uses the original operation/key for explicit command lookup without retrying an uncertain mutation', async () => {
    const f = setup(request => request.method === 'GET' ? Response.json(receipt) : new Promise(() => undefined))
    const body = { expected_revision: 0, state: 'ACTIVE', viewer_ids: [FRIEND] } as const
    await expect(f.transport.request(locationRequests.setSharing(EVENT, body, KEY), f.controller.signal))
      .rejects.toMatchObject({ kind: 'timeout', uncertain: true })
    expect(f.fetch).toHaveBeenCalledTimes(1)
    const result = await f.transport.request(locationRequests.command(EVENT, 'setVenueLocationSharing', KEY), f.controller.signal)
    expect(result.data.command_key).toBe(KEY)
    expect(f.fetch).toHaveBeenCalledTimes(2)
    expect(f.requests[1]!.url).toContain(`/commands/${KEY}?operation=setVenueLocationSharing`)
  })

  it.each([401, 409, 503])('retains canonical HTTP %s error behavior without refresh/replay', async status => {
    const reason = status === 401 ? 'AUTHENTICATION_REQUIRED' : status === 409 ? 'VERSION_CONFLICT' : 'DEPENDENCY_UNAVAILABLE'
    const f = setup(() => Response.json({ code: 'FIXTURE', message: 'fixture', traceId: KEY, timestamp: NOW, details: { reason } }, { status }))
    await expect(f.transport.request(locationRequests.setSharing(EVENT, { expected_revision: 0, state: 'PAUSED', viewer_ids: [] }, KEY), f.controller.signal))
      .rejects.toMatchObject({ kind: 'http', status, uncertain: status === 503 })
    expect(f.fetch).toHaveBeenCalledTimes(1)
  })

  it('fails closed for a different event scope without disclosing a network request', async () => {
    const f = setup()
    await expect(f.transport.request(locationRequests.landmark(OTHER, 'zone'), f.controller.signal)).rejects.toMatchObject({ kind: 'invalid-request' })
    expect(f.fetch).not.toHaveBeenCalled()
  })

  it('does not dispatch with missing canonical credentials or an already aborted caller', async () => {
    const f = setup(); f.controller.abort()
    await expect(f.transport.request(locationRequests.landmarks(EVENT), f.controller.signal)).rejects.toMatchObject({ kind: 'aborted', uncertain: false })
    f.session.replace(1, null)
    await expect(f.transport.request(locationRequests.landmarks(EVENT), new AbortController().signal)).rejects.toMatchObject({ kind: 'session-changed' })
    expect(f.fetch).not.toHaveBeenCalled()
  })

  it('cancels a pending canonical request on logout even when fetch ignores its signal', async () => {
    const f = setup(() => new Promise(() => undefined))
    const result = f.transport.request(locationRequests.landmarks(EVENT), f.controller.signal)
    const rejected = expect(result).rejects.toMatchObject({ kind: 'session-changed' })
    await vi.waitFor(() => expect(f.fetch).toHaveBeenCalledTimes(1))
    f.session.replace(1, null)
    await rejected
  })

  it('connects actual raw Location decoding and full caller privacy fences to canonical HTTP', async () => {
    const f = setup(() => Response.json(grant))
    const current = vi.fn(() => true)
    const client = createLocationClient(f.transport, lease(f, current))
    expect((await client.getLandmark(selection.landmark_id)).data).toEqual(grant)
    expect(current).toHaveBeenCalled()
    expect(f.requests[0]!.url).toContain('/landmarks/zone.~west')
    current.mockReturnValue(false)
    expect(() => client.getLandmark(selection.landmark_id)).toThrow('Request failed: session-changed.')
    expect(f.fetch).toHaveBeenCalledTimes(1)
  })

  it('rejects decoded foreign-event data at the real HTTP/raw decoder boundary', async () => {
    const f = setup(() => Response.json({ ...grant, event_id: OTHER }))
    await expect(createLocationClient(f.transport, lease(f)).getLandmark(selection.landmark_id)).rejects.toMatchObject({ kind: 'invalid-response' })
  })

  it('discards a response when full privacy is revoked while canonical HTTP is pending', async () => {
    let release!: (response: Response) => void
    const f = setup(() => new Promise(resolve => { release = resolve }))
    let current = true
    const result = createLocationClient(f.transport, lease(f, () => current)).getLandmark(selection.landmark_id)
    const rejected = expect(result).rejects.toMatchObject({ kind: 'session-changed' })
    await vi.waitFor(() => expect(f.fetch).toHaveBeenCalledTimes(1))
    current = false; release(Response.json(grant)); await rejected
  })
})
