import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createHttpClient, createMemorySession, type HttpRequest } from '../../http'
import { domainOperation } from '../../domain'
import { MEETING_OPERATION_IDS, meetingRequests } from '../meeting'
import { createHttpMeetingTransport } from '../meeting-http'

const event = '00000000-0000-4000-8000-000000000201'
const friend = '00000000-0000-4000-8000-000000000102'
const key = '00000000-0000-4000-8000-000000000401'
const invitation = '00000000-0000-4000-8000-000000000501'
const base = `/api/concert/events/${event}/venue-meeting/`
function fixture() {
  const session = createMemorySession()
  session.replace(0, { userId: '00000000-0000-4000-8000-000000000101', eventId: event, accessToken: 'fixture.token' })
  const fetch = vi.fn<typeof globalThis.fetch>().mockResolvedValue(Response.json({ ok: true }))
  const client = createHttpClient({ session, fetch })
  return { session, fetch, client, transport: createHttpMeetingTransport(client, session) }
}
const selection = { selectedFriendId: friend, landmark: { landmarkId: 'stage.~west', catalogRevision: 2, landmarkRevision: 3 } }
const query = { selectedFriendId: friend, landmarkId: 'stage.~west', catalogRevision: 2, landmarkRevision: 3 }
const read = (extra: HttpRequest<unknown>['query'] = query): HttpRequest<unknown> => ({
  path: base + 'context', method: 'GET', access: 'protected', errorFamily: 'domain',
  domainOperation: 'resolveVenueMeetingContext', query: extra, decode: value => value,
})
beforeEach(() => vi.stubGlobal('location', { origin: 'https://mcr.example' }))
afterEach(() => { vi.unstubAllGlobals(); vi.restoreAllMocks() })

describe('registered canonical Meeting HTTP binding', () => {
  it('registers exactly the eight public operations with the frozen write and error rules', () => {
    expect(MEETING_OPERATION_IDS).toHaveLength(8)
    for (const id of MEETING_OPERATION_IDS) {
      const operation = domainOperation(id)
      expect(operation).not.toBeNull()
      expect(operation?.path).toContain('/venue-meeting/')
      expect(operation?.idempotency).toBe(['createVenueMeetingInvitation', 'decideVenueMeetingInvitation', 'sendVenueMeetingMessage'].includes(id))
      expect(operation?.errors['409']).toEqual(['REVISION_OR_KEY_CONFLICT'])
      expect(operation?.success).toEqual([200])
    }
  })
  it.each(['stage.~west', '~', 'gate.with.dots'])('preserves the actual unreserved query spelling %s', async landmarkId => {
    const f = fixture()
    await f.client.request(read({ ...query, landmarkId }))
    const request = f.fetch.mock.calls[0]![0] as Request
    expect(new URL(request.url).search).toContain(`landmarkId=${landmarkId}`)
    expect(new URL(request.url).search).not.toMatch(/[%+]/)
    expect(request.headers.get('Authorization')).toBe('Bearer fixture.token')
    expect(request.cache).toBe('no-store')
    expect(request.headers.has('Idempotency-Key')).toBe(false)
  })
  it.each(['.', '..', 'gate%7E', 'gate%2Fwest', 'gate+west', 'gate/west', 'gate?west', 'x'.repeat(129)])(
    'rejects invalid or encoded landmark query %j before fetching', async landmarkId => {
      const f = fixture()
      await expect(f.client.request(read({ ...query, landmarkId }))).rejects.toMatchObject({ kind: 'invalid-request' })
      expect(f.fetch).not.toHaveBeenCalled()
    })
  it('rejects extra query, unsafe revision, missing friend, wrong method and wrong event before fetching', async () => {
    const f = fixture()
    for (const request of [read({ ...query, extra: 'x' }), read({ ...query, catalogRevision: 0 }),
      read({ ...query, landmarkRevision: 1.5 }), read({ landmarkId: 'stage' }), { ...read(), method: 'POST' as const },
      { ...read(), path: base.replace(event, friend) + 'context' },
    ]) await expect(f.client.request(request)).rejects.toMatchObject({ kind: 'invalid-request' })
    expect(f.fetch).not.toHaveBeenCalled()
  })
  it.each(['\n', '\r', '\t'])('rejects trailing control byte %j before URL serialization or fetching', async suffix => {
    const f = fixture()
    await expect(f.client.request(read({ ...query, landmarkId: 'stage.~west' + suffix })))
      .rejects.toMatchObject({ kind: 'invalid-request' })
    expect(f.fetch).not.toHaveBeenCalled()
  })
  it('dispatches all eight exact methods and paths with command keys only on the three writes', async () => {
    const f = fixture()
    f.fetch.mockImplementation(async () => Response.json({ ok: true }))
    const descriptors = [meetingRequests.resolveContext(event, selection), meetingRequests.listInvitations(event, 'INCOMING'),
      meetingRequests.getInvitation(event, invitation, selection), meetingRequests.createInvitation(event, key, { selection }),
      meetingRequests.decideInvitation(event, invitation, key, { selection, expectedRevision: 1, decision: 'ACCEPT' }),
      meetingRequests.listMessages(event, selection), meetingRequests.sendMessage(event, key, { selection, body: 'Hello' }),
      meetingRequests.getCommand(event, key, selection)]
    for (const descriptor of descriptors) {
      await f.transport.request({ ...descriptor, decode: value => value }, new AbortController().signal)
      const request = f.fetch.mock.calls.at(-1)![0] as Request
      expect(request.method).toBe(descriptor.method)
      expect(new URL(request.url).pathname).toBe(descriptor.path)
      expect(request.headers.get('Idempotency-Key')).toBe(descriptor.idempotencyKey ?? null)
      expect(request.headers.get('X-MCR-Intent')).toBe(descriptor.method === 'POST' ? 'browser' : null)
      expect(request.headers.has('Content-Type')).toBe(descriptor.method === 'POST')
    }
    expect(f.fetch).toHaveBeenCalledTimes(8)
  })
  it('passes the actual decoder and original caller signal through the canonical client', async () => {
    const f = fixture(), descriptor = meetingRequests.resolveContext(event, { selectedFriendId: friend })
    f.fetch.mockResolvedValue(Response.json({ invalid: true }))
    await expect(f.transport.request(descriptor, new AbortController().signal)).rejects.toMatchObject({ kind: 'invalid-response' })
    const controller = new AbortController(); controller.abort()
    await expect(f.transport.request(descriptor, controller.signal)).rejects.toMatchObject({ kind: 'session-changed' })
    expect(f.fetch).toHaveBeenCalledTimes(1)
  })
  it('retains canonical mutation uncertainty without resend or a replacement command key', async () => {
    const f = fixture(), descriptor = meetingRequests.sendMessage(event, key, { selection, body: 'Hello' })
    f.fetch.mockResolvedValue(Response.json({ code: 'CONCERT_503', message: 'Unavailable', details: { reason: 'DEPENDENCY_UNAVAILABLE' } }, { status: 503 }))
    await expect(f.transport.request(descriptor, new AbortController().signal)).rejects.toMatchObject({ kind: 'http', status: 503, uncertain: true })
    expect(f.fetch).toHaveBeenCalledTimes(1)
  })
  it('preserves legacy query serialization outside the exact Meeting whitelist', async () => {
    const f = fixture()
    await f.client.request({ path: `/api/concert/events/${event}/notes`, method: 'GET', access: 'protected', errorFamily: 'core',
      query: { label: '~' }, decode: value => value })
    expect(new URL((f.fetch.mock.calls[0]![0] as Request).url).search).toBe('?label=%7E')
  })
})
