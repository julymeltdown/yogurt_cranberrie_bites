import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createHttpClient, createMemorySession, type HttpRequest } from '../../http'
import { createHttpNotesTransport, buildNotesRequest } from '../notes'

const eventId = '00000000-0000-4000-8000-000000000201'
const key = '00000000-0000-4000-8000-000000000401'
const root = `/api/concert/events/${eventId}/venue/landmarks/`
function fixture() {
  const session = createMemorySession()
  session.replace(0, {userId:'00000000-0000-4000-8000-000000000101',eventId,accessToken:'fixture.token'})
  const fetch = vi.fn<typeof globalThis.fetch>().mockResolvedValue(Response.json({ok:true}))
  return {session, fetch, client:createHttpClient({session,fetch})}
}
const read = (landmark:string): HttpRequest<unknown> => ({path:root+landmark+'/notes',method:'GET',access:'protected',
  errorFamily:'domain',domainOperation:'listVenuePlaceNotes',decode:value=>value})
beforeEach(() => vi.stubGlobal('location', {origin:'https://mcr.example'}))
afterEach(() => {vi.unstubAllGlobals();vi.restoreAllMocks()})

describe('exact registered provider landmark path compatibility', () => {
  it.each(['reviewed.main-gate~v1', 'gate.with.dots', '~gate'])('dispatches qualified GET for %s with canonical privacy headers', async landmark => {
    const f = fixture()
    await f.client.request(read(landmark))
    expect(f.fetch).toHaveBeenCalledTimes(1)
    const request = f.fetch.mock.calls[0]![0] as Request
    expect(new URL(request.url).pathname).toBe(root+landmark+'/notes')
    expect(request.headers.get('Authorization')).toBe('Bearer fixture.token')
    expect(request.cache).toBe('no-store')
  })
  it('dispatches a registered write with its explicit canonical key and intent', async () => {
    const f = fixture()
    f.fetch.mockResolvedValue(Response.json({ok:true}, {status:201}))
    await f.client.request({...read('gate.~'),method:'POST',domainOperation:'createVenuePlaceNote',idempotencyKey:key,
      body:{json:{text:'fixture'}}})
    const request = f.fetch.mock.calls[0]![0] as Request
    expect(request.headers.get('Idempotency-Key')).toBe(key)
    expect(request.headers.get('X-MCR-Intent')).toBe('browser')
    expect(request.method).toBe('POST')
  })
  it.each(['.', '..', '%2e', '%2e%2e', 'gate%2fother', 'gate/other', 'gate\\other', 'gate?x', 'gate#x', '', 'x'.repeat(129)])(
    'rejects unsafe landmark %j before fetch', async landmark => {
      const f = fixture()
      await expect(f.client.request(read(landmark))).rejects.toMatchObject({kind:'invalid-request'})
      expect(f.fetch).not.toHaveBeenCalled()
    })
  it('rejects mismatched registration, method, query and missing command key before fetch', async () => {
    const f = fixture()
    for (const request of [
      {...read('gate.~'),domainOperation:'getVenuePlaceNote' as const},
      {...read('gate.~'),method:'POST' as const},
      {...read('gate.~'),query:{limit:51}},
      {...read('gate.~'),query:{unknown:1}},
      {...read('gate.~'),method:'POST' as const,domainOperation:'createVenuePlaceNote' as const},
    ]) await expect(f.client.request(request)).rejects.toMatchObject({kind:'invalid-request'})
    expect(f.fetch).not.toHaveBeenCalled()
  })
  it.each(['/api/auth/log.in', '/api/legacy/dotted.route', `/api/concert/events/${eventId}/me.~`])(
    'preserves the historical grammar outside registered landmark routes: %s', async path => {
      const f = fixture()
      await expect(f.client.request({path,method:'GET',access:'protected',errorFamily:'core',decode:value=>value}))
        .rejects.toMatchObject({kind:'invalid-request'})
      expect(f.fetch).not.toHaveBeenCalled()
    })
  it('does not let the Notes adapter dispatch a stale shell session', async () => {
    const f = fixture(), scope = {...f.session.read(),policyGeneration:0,gate:'open' as const}
    f.session.replace(scope.generation,null)
    const transport = createHttpNotesTransport(f.client,f.session)
    await expect(transport.send(buildNotesRequest('listVenuePlaceNotes',{eventId,landmarkId:'gate.~'}),
      {scope,signal:new AbortController().signal})).rejects.toMatchObject({kind:'scope-expired'})
    expect(f.fetch).not.toHaveBeenCalled()
  })
})
