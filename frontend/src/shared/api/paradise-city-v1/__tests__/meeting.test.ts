import { describe, expect, it } from 'vitest'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { fileURLToPath } from 'node:url'
import { resolve } from 'node:path'
import { TransportError } from '../../errors'
import type { HttpResult, SessionScope } from '../../http'
import { MEETING_CONTRACT, MEETING_OPERATION_IDS, MeetingClientError, createMeetingClient, decodeMeeting, decodeMeetingError,
  meetingRequests, parseMeetingQuery, type MeetingCanonicalTransport, type MeetingLease, type MeetingRequest } from '../meeting'

const actor = '10000000-0000-4000-8000-000000000001', friend = '10000000-0000-4000-8000-000000000002'
const event = '20000000-0000-4000-8000-000000000001', key = '30000000-0000-4000-8000-000000000001'
const id = '40000000-0000-4000-8000-000000000001', conversation = '50000000-0000-4000-8000-000000000001'
const scope: SessionScope = Object.freeze({ userId: actor, eventId: event, generation: 7 })
const selected = { selectedFriendId: friend }, landmark = { landmarkId: 'reviewed-L.4', catalogRevision: 3, landmarkRevision: 9 }
const inviteSelected = { ...selected, landmark }, contextId = 'vm1_' + 'a'.repeat(64)
const now = Date.parse('2030-01-01T00:00:00Z'), time = '2030-01-01T00:00:00Z', until = '2030-01-01T00:10:00Z'
const context = { contextId, eventId: event, actorId: actor, selection: selected, conversationId: null, canRead: true,
  canSend: true, canInvite: false, relationshipRevision: 2, policyRevision: 8, serverTime: time, validUntil: until,
  eventTimezone: 'Asia/Seoul', invitationExpiresAt: null, contentExpiresAt: until, purgeAt: '2030-01-02T00:00:00Z' }
const invitation = { id, eventId: event, senderId: actor, recipientId: friend, landmarkId: landmark.landmarkId, revision: 4,
  state: 'PENDING', createdAt: time, expiresAt: until, allowedInvitationActions: ['CANCEL'] }
const message = { id, sequence: 1, senderId: actor, body: 'Hello', invitationId: null, createdAt: time, expiresAt: until }
const page = { contextId, conversationId: conversation, items: [message], cursor: '1', hasMore: false, gap: false, serverTime: time }
const applied = { commandKey: key, state: 'APPLIED', replayed: false,
  result: { kind: 'MESSAGE', message: { id, conversationId: conversation, sequence: 1, createdAt: time, expiresAt: until } } }
const unknown = { commandKey: key, state: 'UNKNOWN', serverTime: time }
function lease(controller = new AbortController()): MeetingLease { return { scope, signal: controller.signal, isCurrent: () => true, now: () => now } }
class Transport implements MeetingCanonicalTransport {
  calls: { operation: string; method: string; key: string | undefined; signal: AbortSignal }[] = []
  response: unknown = context
  failure: unknown = null
  gate: Promise<void> = Promise.resolve()
  async request<T>(request: MeetingRequest<T>, signal: AbortSignal): Promise<HttpResult<T>> {
    this.calls.push({ operation: request.operationId, method: request.method, key: request.idempotencyKey, signal })
    await this.gate
    if (this.failure) throw this.failure
    return { data: request.decode(this.response, { scope, path: request.path, status: 200 }), scope }
  }
}
function decode<T>(request: MeetingRequest<T>, value: unknown, active = scope): T {
  return request.decode(value, { scope: active, status: 200, path: request.path })
}
function invalid(name: Parameters<typeof decodeMeeting>[0], value: unknown) {
  expect(() => decodeMeeting(name, value)).toThrow(MeetingClientError)
}

describe('frozen public M01 compatibility', () => {
  it('matches the immutable contract hash, all eight routes/methods and explicit status reasons', () => {
    const root = resolve(fileURLToPath(new URL('.', import.meta.url)), '../../../../../..')
    const raw = execFileSync('git', ['show', MEETING_CONTRACT.revision + ':backend/docs/api/paradise-city-v1/meeting.openapi.json'], { cwd: root })
    expect(createHash('sha256').update(raw).digest('hex')).toBe(MEETING_CONTRACT.openapiSha256)
    const doc = JSON.parse(raw.toString())
    const requests = [meetingRequests.resolveContext(event, selected), meetingRequests.listInvitations(event, 'INCOMING'),
      meetingRequests.getInvitation(event, id, inviteSelected), meetingRequests.createInvitation(event, key, { selection: inviteSelected }),
      meetingRequests.decideInvitation(event, id, key, { selection: inviteSelected, expectedRevision: 4, decision: 'ACCEPT' }),
      meetingRequests.listMessages(event, selected), meetingRequests.sendMessage(event, key, { selection: selected, body: 'Hi' }),
      meetingRequests.getCommand(event, key, selected)]
    const registered = Object.entries(doc.paths).flatMap(([path, methods]) => Object.entries(methods as Record<string, { operationId: string }>).map(([method, op]) => ({ path, method: method.toUpperCase(), id: op.operationId })))
    expect(new Set(registered.map(row => row.id))).toEqual(new Set(MEETING_OPERATION_IDS))
    for (const req of requests) {
      const op = registered.find(row => row.id === req.operationId)!
      expect(req.method).toBe(op.method)
      expect(req.path).toBe(op.path.replace('{eventId}', event).replace('{invitationId}', id).replace('{commandKey}', key))
      expect(req.access).toBe('protected'); expect(req.errorFamily).toBe('domain')
      expect(req.cache).toBe('no-store'); expect(req.retry).toBe('never')
      expect('domainOperation' in req).toBe(false)
      if (req.method === 'GET') { expect(req.body).toBeUndefined(); expect(req.idempotencyKey).toBeUndefined(); expect('headers' in req).toBe(false) }
      else expect(req.idempotencyKey).toBe(key)
    }
    const required = doc.paths['/api/concert/events/{eventId}/venue-meeting/invitations/{invitationId}'].get.parameters
    expect(required.filter((p: { required?: boolean; name?: string }) => p.required && ['landmarkId', 'catalogRevision', 'landmarkRevision'].includes(p.name ?? '')).length).toBe(3)
    for (const [status, reasons] of Object.entries(doc['x-mcr-error-reasons'])) for (const reason of reasons as string[]) {
      expect(decodeMeetingError(Number(status), { code: `CONCERT_${status}`, message: 'Concert request unavailable', details: { reason }, traceId: null, timestamp: time })).toEqual({ reason })
    }
    expect(doc.components.schemas.Revision.maximum).toBe(Number.MAX_SAFE_INTEGER)
    expect(doc['x-mcr-limits']).toMatchObject({ requestBytes: 12000, messageUnicodeScalars: 2000, messageUtf8Bytes: 8000, pageSize: 50 })
  })
  it('accepts the actual closed response and mutation shapes', () => {
    for (const [name, value] of Object.entries({ MeetingContext: context, MeetingInvitation: invitation, MeetingMessage: message,
      MeetingMessagePage: page, MeetingInboxPage: { items: [{ selection: inviteSelected, invitation }], nextCursor: null, serverTime: time },
      AppliedReceipt: applied, UnknownReceipt: unknown,
      StatusAppliedReceipt: { commandKey: key, state: 'APPLIED', operation: 'sendVenueMeetingMessage', result: applied.result, serverTime: time },
      CreateInvitation: { selection: inviteSelected }, DecideInvitation: { selection: inviteSelected, expectedRevision: 4, decision: 'CANCEL' },
      SendMessage: { selection: selected, body: 'Hi', invitationId: null },
    })) expect(decodeMeeting(name as Parameters<typeof decodeMeeting>[0], value)).toEqual(value)
  })
})

describe('closed wire inputs', () => {
  it.each(['00000000-0000-0000-0000-000000000000', actor.toUpperCase().replace('000000000001', 'ABCDEFABCDEF'), actor + '\n', 'user', null])('rejects noncanonical UUID %s', value => invalid('Uuid', value))
  it.each([0, -1, 1.5, Number.MAX_SAFE_INTEGER + 1, '3', NaN, Infinity])('never coerces revisions %s', value => invalid('Revision', value))
  it.each(['01', '+1', '1.0', '-1', '9007199254740992', '1\n', '1e2'])('rejects noncanonical cursors %s', value => invalid('Cursor', value))
  it.each(['.', '..', '%2e', 'place/child', 'ok\n', 'a'.repeat(129)])('rejects opaque identity alias %s', value => invalid('OpaqueIdentity', value))
  it('rejects unknown fields, undefined, prototypes, getters, symbols, non-JSON and malformed dates', () => {
    invalid('Selection', { ...selected, actorId: actor }); invalid('Selection', { ...selected, landmark: undefined })
    invalid('Selection', Object.assign(Object.create({ grant: true }), selected))
    invalid('Selection', { ...selected, [Symbol('secret')]: true })
    invalid('Selection', { get selectedFriendId() { throw Error('private') } })
    invalid('MeetingContext', { ...context, serverTime: '2030-02-30T00:00:00Z' })
    invalid('MeetingContext', { ...context, validUntil: time })
    invalid('MeetingContext', { ...context, purgeAt: '2029-01-01T00:00:00Z' })
    invalid('MeetingContext', { ...context, eventTimezone: '+09:00' })
    invalid('MeetingContext', { ...context, canInvite: true })
    expect(decodeMeeting('Selection', { ...selected, landmark: null })).toEqual({ ...selected, landmark: null })
  })
  it('accepts only all-or-none canonical query triples and exact inbox keys', () => {
    expect(parseMeetingQuery(`selectedFriendId=${friend}`, 'selection')).toEqual(selected)
    expect(parseMeetingQuery(`landmarkRevision=9&selectedFriendId=${friend}&catalogRevision=3&landmarkId=reviewed-L.4`, 'invitation')).toEqual({ ...selected, ...landmark })
    expect(parseMeetingQuery('direction=INCOMING&cursor=0', 'inbox')).toEqual({ direction: 'INCOMING', cursor: '0' })
    for (const raw of [`selectedFriendId=${friend}&landmarkId=x`, `selectedFriendId=${friend}&catalogRevision=1`, `selectedFriendId=${friend}&selectedFriendId=${friend}`,
      `selectedFriendId=${friend}&admin=1`, `selectedFriendId=%31${friend.slice(1)}`, `selectedFriendId=${friend}+`, `selectedFriendId=${friend}&cursor=01`]) {
      expect(() => parseMeetingQuery(raw, 'messages')).toThrow(MeetingClientError)
    }
    expect(() => parseMeetingQuery(`selectedFriendId=${friend}`, 'invitation')).toThrow(MeetingClientError)
    expect(() => parseMeetingQuery('direction=INCOMING&landmarkId=x', 'inbox')).toThrow(MeetingClientError)
  })
  it('bounds Unicode scalars and UTF8 independently and rejects controls/lone surrogates/blank text', () => {
    const body = '😀'.repeat(2000)
    expect(new TextEncoder().encode(body).length).toBe(8000)
    expect(meetingRequests.sendMessage(event, key, { selection: selected, body }).body?.json).toEqual({ selection: selected, body })
    for (const body of ['', ' \n\t', 'a'.repeat(2001), '😀'.repeat(2001), 'x\r', 'x\0', 'x\u007f', 'x\u0085', 'x\ud800']) {
      expect(() => meetingRequests.sendMessage(event, key, { selection: selected, body })).toThrow(MeetingClientError)
    }
    expect(meetingRequests.sendMessage(event, key, { selection: selected, body: 'Hi\n\tthere' }).body).toBeDefined()
  })
  it('requires the reviewed triple for invitation detail/create/CAS; does not accept revision strings', () => {
    expect(() => meetingRequests.createInvitation(event, key, { selection: selected } as never)).toThrow(MeetingClientError)
    expect(() => meetingRequests.getInvitation(event, id, selected as never)).toThrow(MeetingClientError)
    expect(() => meetingRequests.decideInvitation(event, id, key, { selection: { ...selected, landmark: { ...landmark, catalogRevision: '3' } }, expectedRevision: 4, decision: 'CANCEL' } as never)).toThrow(MeetingClientError)
    expect(() => meetingRequests.decideInvitation(event, id, key, { selection: inviteSelected, expectedRevision: 0, decision: 'CANCEL' })).toThrow(MeetingClientError)
  })
})

describe('scope-bound output and ordering', () => {
  it('binds actor/event/selection, invitation participants and exact command key', () => {
    const req = meetingRequests.resolveContext(event, selected)
    for (const value of [{ ...context, actorId: friend }, { ...context, eventId: id }, { ...context, selection: { selectedFriendId: actor } }]) expect(() => decode(req, value)).toThrow(MeetingClientError)
    expect(() => decode(meetingRequests.getInvitation(event, id, inviteSelected), { ...invitation, recipientId: id })).toThrow(MeetingClientError)
    expect(() => decode(meetingRequests.sendMessage(event, key, { selection: selected, body: 'Hi' }), { ...applied, commandKey: id })).toThrow(MeetingClientError)
    expect(() => decode(meetingRequests.getCommand(event, key, selected), { ...unknown, commandKey: id })).toThrow(MeetingClientError)
  })
  it('rejects duplicate/out-of-order messages, backwards cursor and oversized page; retains authoritative gap', () => {
    for (const value of [{ ...page, items: [message, message] }, { ...page, cursor: '0' }, { ...page, conversationId: null },
      { ...page, items: Array.from({ length: 51 }, () => message) },
      { ...page, cursor: '2', items: [{ ...message, sequence: 2 }, { ...message, id: key, sequence: 1 }] }]) invalid('MeetingMessagePage', value)
    expect(decodeMeeting('MeetingMessagePage', { ...page, gap: true })).toMatchObject({ gap: true })
    expect(decode(meetingRequests.listMessages(event, selected, '1', contextId), { ...page, conversationId: key })).toMatchObject({ conversationId: key })
    expect(() => decode(meetingRequests.listMessages(event, selected, '1', 'vm1_' + 'b'.repeat(64)), page)).toThrow(MeetingClientError)
  })
  it('rejects a foreign sender even when the page context identifier looks valid', () => {
    expect(() => decode(meetingRequests.listMessages(event, selected), { ...page, items: [{ ...message, senderId: key }] })).toThrow(MeetingClientError)
  })
  it('retains historical receipt snapshots without treating them as current invitation rights', () => {
    const historical = { commandKey: key, state: 'APPLIED', operation: 'createVenueMeetingInvitation', serverTime: time,
      result: { kind: 'INVITATION', invitation: { ...invitation, createdAt: '2020-01-01T00:00:00Z', expiresAt: '2020-01-02T00:00:00Z' } } }
    expect(decode(meetingRequests.getCommand(event, key, inviteSelected), historical)).toEqual(historical)
    expect(() => decode(meetingRequests.getCommand(event, key, selected), historical)).toThrow(MeetingClientError)
  })
})

describe('one canonical dispatch with real caller fences', () => {
  it('opening friend-only MESSAGE creates no invitation, conversation or send', async () => {
    const transport = new Transport(), client = createMeetingClient(transport, lease())
    const req = meetingRequests.resolveContext(event, { ...selected, landmark: null })
    expect(transport.calls).toEqual([])
    expect((await client.execute(req)).data.conversationId).toBeNull()
    expect(transport.calls.map(row => row.method)).toEqual(['GET'])
    transport.response = applied
    await client.execute(meetingRequests.sendMessage(event, key, { selection: selected, body: 'Hi', invitationId: null }))
    expect(transport.calls.map(row => row.operation)).toEqual(['resolveVenueMeetingContext', 'sendVenueMeetingMessage'])
  })
  it('no registered transport means unavailable with zero dispatch; only sealed requests execute', async () => {
    const request = meetingRequests.resolveContext(event, selected)
    await expect(createMeetingClient(null, lease()).execute(request)).rejects.toMatchObject({ code: 'DEPENDENCY_UNAVAILABLE' })
    const transport = new Transport()
    await expect(createMeetingClient(transport, lease()).execute({ ...request })).rejects.toMatchObject({ code: 'INVALID_INPUT' })
    expect(transport.calls).toHaveLength(0)
  })
  it('keeps exact immutable body/key bytes and never silently supplies another key', () => {
    const source = { selection: { ...selected }, body: 'original' }, req = meetingRequests.sendMessage(event, key, source)
    const before = JSON.stringify(req.body?.json)
    source.body = 'changed'; source.selection.selectedFriendId = actor
    expect(JSON.stringify(req.body?.json)).toBe(before)
    expect(Object.isFrozen(req)).toBe(true); expect(Object.isFrozen(req.body?.json)).toBe(true)
    expect(req.idempotencyKey).toBe(key)
    expect(() => meetingRequests.sendMessage(event, 'new-key', source)).toThrow(MeetingClientError)
  })
  it('uncertain send reconciles only by explicit original-key GET; UNKNOWN triggers no retry', async () => {
    const transport = new Transport(), client = createMeetingClient(transport, lease())
    transport.failure = new TransportError('timeout', { uncertain: true })
    await expect(client.execute(meetingRequests.sendMessage(event, key, { selection: selected, body: 'Hi' }))).rejects.toMatchObject({ code: 'UNKNOWN_OUTCOME', uncertain: true, commandKey: key })
    expect(transport.calls).toHaveLength(1)
    transport.failure = null; transport.response = unknown
    expect((await client.execute(meetingRequests.getCommand(event, key, selected))).data.state).toBe('UNKNOWN')
    await Promise.resolve(); expect(transport.calls.map(row => row.method)).toEqual(['POST', 'GET'])
  })
  it('keeps 503/504 mutation uncertainty instead of dropping the original key', async () => {
    for (const status of [503, 504]) {
      const transport = new Transport(); transport.failure = new TransportError('http', { status, uncertain: true })
      await expect(createMeetingClient(transport, lease()).execute(meetingRequests.sendMessage(event, key, { selection: selected, body: 'Hi' }))).rejects.toMatchObject({ code: 'UNKNOWN_OUTCOME', uncertain: true, commandKey: key })
      expect(transport.calls).toHaveLength(1)
    }
  })
  it('rejects pre-abort without dispatch and a late privacy-revoked callback after dispatch', async () => {
    const controller = new AbortController(); controller.abort()
    const transport = new Transport(), req = meetingRequests.resolveContext(event, selected)
    await expect(createMeetingClient(transport, lease(controller)).execute(req)).rejects.toMatchObject({ code: 'STALE_SCOPE' })
    expect(transport.calls).toHaveLength(0)
    let release!: () => void, active = true
    transport.gate = new Promise<void>(resolve => { release = resolve })
    const client = createMeetingClient(transport, { ...lease(), isCurrent: () => active })
    const pending = client.execute(req); active = false; release()
    await expect(pending).rejects.toMatchObject({ code: 'STALE_SCOPE' }); expect(transport.calls).toHaveLength(1)
  })
  it('rechecks privacy after a synchronous caller clock callback before disclosing data', async () => {
    let active = true
    const transport = new Transport()
    const client = createMeetingClient(transport, { ...lease(), isCurrent: () => active, now: () => { active = false; return now } })
    await expect(client.execute(meetingRequests.resolveContext(event, selected))).rejects.toMatchObject({ code: 'STALE_SCOPE' })
  })
  it('hides expired context/message data at the caller real clock', async () => {
    const transport = new Transport(); transport.response = context
    const client = createMeetingClient(transport, { ...lease(), now: () => Date.parse(until) })
    await expect(client.execute(meetingRequests.resolveContext(event, selected))).rejects.toMatchObject({ code: 'RESOURCE_UNAVAILABLE' })
    transport.response = page
    await expect(client.execute(meetingRequests.listMessages(event, selected))).rejects.toMatchObject({ code: 'RESOURCE_UNAVAILABLE' })
  })
  it('caps current pending invitation and inbox disclosure without changing historical snapshots', async () => {
    const transport = new Transport(), client = createMeetingClient(transport, { ...lease(), now: () => Date.parse(until) })
    transport.response = invitation
    await expect(client.execute(meetingRequests.getInvitation(event, id, inviteSelected))).rejects.toMatchObject({ code: 'RESOURCE_UNAVAILABLE' })
    transport.response = { items: [{ selection: inviteSelected, invitation }], nextCursor: null, serverTime: time }
    await expect(client.execute(meetingRequests.listInvitations(event, 'OUTGOING'))).rejects.toMatchObject({ code: 'RESOURCE_UNAVAILABLE' })
  })
  it('never passes server messages, credential bodies or underlying exceptions to callers', async () => {
    const transport = new Transport(); transport.failure = new Error('Bearer secret-password raw response')
    try { await createMeetingClient(transport, lease()).execute(meetingRequests.sendMessage(event, key, { selection: selected, body: 'private' })) } catch (error) {
      expect(error).toBeInstanceOf(MeetingClientError)
      expect(String(error)).not.toMatch(/Bearer|secret-password|raw response|private/)
      expect('cause' in (error as object)).toBe(false)
    }
    expect(() => decodeMeetingError(404, { reason: 'RESOURCE_UNAVAILABLE' })).toThrow(MeetingClientError)
    expect(() => decodeMeetingError(404, { code: 'X', message: 'secret', details: { reason: 'RESOURCE_UNAVAILABLE', body: 'secret' } })).toThrow(MeetingClientError)
    expect(() => decodeMeetingError(404, { code: 'X', message: 'secret', details: { reason: 'INVALID_INPUT' } })).toThrow(MeetingClientError)
    expect(decodeMeetingError(404, { code: 'X', message: 'secret', details: { reason: 'RESOURCE_UNAVAILABLE' } })).toEqual({ reason: 'RESOURCE_UNAVAILABLE' })
  })
})
