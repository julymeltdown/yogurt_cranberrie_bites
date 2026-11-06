import { afterEach, describe, expect, it } from 'vitest'
import { createMeetingClient, MeetingClientError, type MeetingRequest, type MeetingContext, type MeetingMessagePage,
  type MeetingInvitation, type MeetingSelection } from '../../src/shared/api/paradise-city-v1/meeting'
import { createNormalizedMeetingAdapter } from '../../src/app/venue-community-entry/adapter'
import type { LeafCommand, MeetingContextView } from '../../src/features/venue-invitations/model'
import type { MeetingOpenIntent } from '../../src/features/paradise-city-map/ports'
import { deferred, fixtureIds as ids, meetingFixture } from './fixture'
const stops: (() => void)[] = []
afterEach(() => { for (const stop of stops.splice(0)) stop() })
const uuid = (n: number) => `00000000-0000-4000-8000-${String(n).padStart(12, '0')}`
function setup(place = true) {
  const f = meetingFixture(), request = { scope: f.scope, signal: f.lifetime.signal }, now = f.time()
  const iso = (offset: number) => new Date(now + offset).toISOString()
  const selected: MeetingSelection = { selectedFriendId: ids.friend, ...(place ? { landmark: { landmarkId: 'synthetic-real-main-gate', catalogRevision: 7, landmarkRevision: 11 } } : {}) }
  const actualIntent: MeetingOpenIntent = place ? f.intent : { action: 'MESSAGE', scope: f.scope, venue: f.intent.venue, targetUserId: ids.friend, origin: 'map-friend' }
  let context: MeetingContext = { contextId: `vm1_${'a'.repeat(64)}`, eventId: ids.event, actorId: ids.actor, selection: selected,
    conversationId: uuid(99), canRead: true, canSend: true, canInvite: place, relationshipRevision: 1, policyRevision: 2,
    serverTime: iso(0), validUntil: iso(60_000), eventTimezone: 'Asia/Seoul', invitationExpiresAt: place ? iso(600_000) : null,
    contentExpiresAt: iso(3_600_000), purgeAt: iso(7_200_000) }
  const invitation: MeetingInvitation = { id: ids.invitation, eventId: ids.event, senderId: ids.friend, recipientId: ids.actor,
    landmarkId: 'synthetic-real-main-gate', revision: 3, state: 'PENDING', createdAt: iso(-1000), expiresAt: iso(600_000), allowedInvitationActions: ['ACCEPT', 'DECLINE'] }
  const message = (sequence: number, body = `Message ${sequence}`) => ({ id: uuid(100 + sequence), sequence, senderId: ids.friend,
    body, invitationId: null, createdAt: iso(-1000), expiresAt: iso(3_600_000) })
  let page: MeetingMessagePage = { contextId: context.contextId, conversationId: context.conversationId, items: [], cursor: '0', hasMore: false, gap: false, serverTime: iso(0) }
  const calls: MeetingRequest<unknown>[] = []
  let override: ((descriptor: MeetingRequest<unknown>) => unknown | Promise<unknown>) | null = null
  const client = createMeetingClient({ async request<T>(descriptor: MeetingRequest<T>, signal: AbortSignal) {
    expect(signal).toBe(request.signal); calls.push(descriptor)
    const data = override ? await override(descriptor) : descriptor.operationId === 'resolveVenueMeetingContext' ? context
      : descriptor.operationId === 'listVenueMeetingMessages' ? page
        : descriptor.operationId === 'getVenueMeetingInvitation' ? invitation
          : descriptor.operationId === 'listVenueMeetingInvitations' ? { items: [{ selection: selected, invitation }], nextCursor: null, serverTime: iso(0) } : null
    return { data: data as T, scope: f.scope }
  } }, { scope: f.scope, signal: request.signal, isCurrent: () => f.auth.privacy.isCurrent(f.scope), now: f.time })
  const factory = createNormalizedMeetingAdapter({ client, scope: f.scope, signal: request.signal,
    isCurrent: scope => f.auth.privacy.isCurrent(scope), now: f.time, registerCleanup: cb => f.auth.privacy.registerCleanup(cb),
    selectionFor: () => selected, intentFor: () => f.inviteIntent,
    presentationFor: () => ({ self: f.context().self, friend: f.context().friend,
      landmarkLabel: place ? 'Main Gate' : null, landmarkDescription: place ? 'Reviewed fixture meeting place.' : null }) })
  stops.push(() => { factory.dispose(); f.dispose() })
  const command = (view: MeetingContextView, kind: LeafCommand['kind'] = 'send'): LeafCommand => ({ key: uuid(800), kind,
    contextId: view.contextId, contextVersion: view.contextVersion, invitationId: view.invitation?.id ?? null,
    invitationVersion: view.invitation?.version ?? null, body: kind === 'send' ? 'Meet after the show.' : null, selection: view.selection })
  return { f, request, selected, intent: actualIntent, invitation, calls, factory, message, command,
    context: () => context, setContext(value: Partial<MeetingContext>) { context = { ...context, ...value } },
    page: () => page, setPage(value: Partial<MeetingMessagePage>) { page = { ...page, ...value } },
    override(value: typeof override) { override = value },
    async read() { return await factory.adapter.readSelection(actualIntent, request) } }
}
describe('actual shared-client normalized adapter', () => {
  it('reads a no-place authorized friend context without writing or requiring an invitation', async () => {
    const h = setup(false), view = await h.read(); expect(view?.canSend).toBe(true); expect(view?.landmarkLabel).toBeNull()
    expect(h.calls.map(x => x.method)).toEqual(['GET', 'GET']); expect(h.calls[0]?.query).toEqual({ selectedFriendId: ids.friend })
  })
  it('never coerces mismatched actual provider revisions into a numeric request', async () => {
    const h = setup(); expect(await h.factory.adapter.readSelection({ ...h.f.intent, landmark: { landmarkId: 'synthetic-real-main-gate', catalogRevision: 'art-v7', landmarkRevision: '11' } }, h.request)).toBeNull(); expect(h.calls).toHaveLength(0)
  })
  it('checks both 2000 scalar and UTF8 wire limits through the actual schema', async () => {
    const h = setup(false), view = (await h.read())!; expect(view.messagePolicy.accepts('😀'.repeat(2000))).toBe(true)
    expect(view.messagePolicy.accepts('😀'.repeat(2001))).toBe(false); expect(view.messagePolicy.accepts('a\u0000')).toBe(false)
  })
  it('supports explicit ascending pages, stable deduplication and cursor advancement', async () => {
    const h = setup(); h.setPage({ items: [h.message(1)], cursor: '1', hasMore: true })
    const first = (await h.read())!; h.setPage({ items: [h.message(1), h.message(2)], cursor: '2', hasMore: false })
    const next = await h.factory.adapter.loadMore!(first, h.request)
    expect(next?.messages.map(x => x.body)).toEqual(['Message 1', 'Message 2']); expect(next?.conversation?.nextCursor).toBeNull()
    expect(h.calls.at(-1)?.query?.cursor).toBe('1')
  })
  it('drops old private messages at an authoritative history gap instead of resurrecting them', async () => {
    const h = setup(); h.setPage({ items: [h.message(1)], cursor: '1', hasMore: true }); const first = (await h.read())!
    h.setPage({ items: [h.message(5)], cursor: '5', gap: true, hasMore: false }); const next = await h.factory.adapter.loadMore!(first, h.request)
    expect(next?.messages.map(x => x.body)).toEqual(['Message 5']); expect(next?.conversation?.earlierCursor).toBe('4')
  })
  it('rejects a same-ID changed payload across page boundaries', async () => {
    const h = setup(); h.setPage({ items: [h.message(1)], cursor: '1', hasMore: true }); const first = (await h.read())!
    h.setPage({ items: [h.message(1, 'Rewritten historical content')], cursor: '2', hasMore: false })
    expect(await h.factory.adapter.loadMore!(first, h.request)).toBeNull()
  })
  it('rejects a third participant even when the message has a valid wire schema', async () => {
    const h = setup(); h.setPage({ items: [{ ...h.message(1), senderId: ids.other }], cursor: '1' }); expect(await h.read()).toBeNull()
  })
  it('resets a changed conversation before reusing the old cursor', async () => {
    const h = setup(); h.setPage({ items: [h.message(1)], cursor: '1' }); await h.read()
    h.setContext({ conversationId: uuid(98) }); h.setPage({ conversationId: uuid(98), items: [h.message(2)], cursor: '2' })
    expect((await h.read())?.messages.map(x => x.body)).toEqual(['Message 2']); expect(h.calls.at(-1)?.query?.cursor).toBe('0')
  })
  it('leaves an uncertain command under its original key and never executes it twice', async () => {
    const h = setup(false), view = (await h.read())!, command = h.command(view)
    h.override(descriptor => { throw new MeetingClientError('UNKNOWN_OUTCOME', true, descriptor.idempotencyKey ?? null) })
    expect(await h.factory.adapter.execute(command, h.request)).toEqual({ state: 'unknown' })
    expect(await h.factory.adapter.execute(command, h.request)).toEqual({ state: 'unknown' })
    expect(h.calls.filter(x => x.method === 'POST')).toHaveLength(1)
    expect(h.calls.at(-1)?.idempotencyKey).toBe(command.key)
  })
  it('resolves only the original key/selection/operation and never resends', async () => {
    const h = setup(false), view = (await h.read())!, command = h.command(view)
    h.override(() => { throw new MeetingClientError('UNKNOWN_OUTCOME', true, command.key) }); await h.factory.adapter.execute(command, h.request)
    h.override(() => ({ commandKey: command.key, state: 'APPLIED', operation: 'sendVenueMeetingMessage', serverTime: h.context().serverTime,
      result: { kind: 'MESSAGE', message: { id: uuid(321), conversationId: uuid(99), sequence: 1, createdAt: h.context().serverTime, expiresAt: h.context().contentExpiresAt } } }))
    expect(await h.factory.adapter.status(command, h.request)).toEqual({ state: 'applied' })
    expect(h.calls.filter(x => x.method === 'POST')).toHaveLength(1); expect(h.calls.at(-1)?.query).toEqual({ selectedFriendId: ids.friend })
  })
  it('uses current integer invitation revision and server decision capabilities', async () => {
    const h = setup(); await h.factory.listInvitations('INCOMING', undefined, h.request)
    const view = (await h.factory.adapter.readInvitation(ids.invitation, h.request))!, command = h.command(view, 'accept')
    h.override(descriptor => descriptor.operationId === 'getVenueMeetingInvitation' ? h.invitation : { commandKey: command.key, state: 'APPLIED', replayed: false, result: { kind: 'INVITATION', invitation: { ...h.invitation, revision: 4, state: 'ACCEPTED', allowedInvitationActions: [] } } })
    expect(await h.factory.adapter.execute(command, h.request)).toEqual({ state: 'applied' })
    expect(h.calls.at(-1)?.body?.json).toMatchObject({ expectedRevision: 3, decision: 'ACCEPT' })
  })
  it.each([true, false])('embeds a place only for an actual matching invitation message: %s', async associated => {
    const h = setup(); await h.factory.listInvitations('INCOMING', undefined, h.request)
    h.setPage({ items: [{ ...h.message(1), invitationId: associated ? ids.invitation : null }], cursor: '1' })
    const view = await h.factory.adapter.readInvitation(ids.invitation, h.request)
    expect(view?.sharedPlaceMessageId).toBe(associated ? h.message(1).id : null)
  })
  it('does not infer detail authority from a bare invitation UUID', async () => {
    const h = setup(); expect(await h.factory.adapter.readInvitation(ids.invitation, h.request)).toBeNull(); expect(h.calls).toHaveLength(0)
  })
  it('maps a producer permission denial to a truthful rejected reason', async () => {
    const h = setup(); await h.factory.listInvitations('INCOMING', undefined, h.request)
    const view = (await h.factory.adapter.readInvitation(ids.invitation, h.request))!, command = h.command(view, 'cancel')
    h.override(() => { throw new MeetingClientError('PERMISSION_DENIED') })
    expect(await h.factory.adapter.execute(command, h.request)).toEqual({ state: 'rejected', reason: 'denied' })
  })
  it('keeps terminal producer status past the original deadline and never invents producer capacity', async () => {
    const h = setup(), expiresAt = new Date(h.f.time() - 1).toISOString()
    h.override(() => ({ items: [{ selection: h.selected, invitation: { ...h.invitation, state: 'ACCEPTED', expiresAt,
      allowedInvitationActions: [] } }], nextCursor: null, serverTime: h.context().serverTime }))
    const page = await h.factory.listInvitations('INCOMING', undefined, h.request)
    expect(page?.items[0]?.invitation.status).toBe('accepted')
    expect(page?.items[0]?.invitation.allowedActions).toEqual([])
    expect(page?.capacity).toBeNull()
  })
  it('keeps terminal producer status and granted actions past the original deadline', async () => {
    const h = setup(), expiresAt = new Date(h.f.time() - 1).toISOString()
    h.override(() => ({ items: [{ selection: h.selected, invitation: { ...h.invitation, state: 'ACCEPTED', expiresAt,
      allowedInvitationActions: ['CANCEL'] } }], nextCursor: null, serverTime: h.context().serverTime }))
    const page = await h.factory.listInvitations('INCOMING', undefined, h.request)
    expect(page?.items[0]?.invitation).toMatchObject({ status: 'accepted', allowedActions: ['cancel'] })
  })
  it('supports both inbox directions and their actual continuation cursors', async () => {
    const h = setup(); expect((await h.factory.listInvitations('INCOMING', '5', h.request))?.items[0]?.target?.targetUserId).toBe(ids.friend)
    expect(h.calls.at(-1)?.query).toEqual({ direction: 'INCOMING', cursor: '5' })
    h.override(() => ({ items: [{ selection: h.selected, invitation: { ...h.invitation, senderId: ids.actor, recipientId: ids.friend } }], nextCursor: '9', serverTime: h.context().serverTime }))
    expect((await h.factory.listInvitations('OUTGOING', undefined, h.request))?.nextCursor).toBe('9')
  })
  it('scrubs pending keys and invitation targets on canonical privacy withdrawal', async () => {
    const h = setup(); await h.factory.listInvitations('INCOMING', undefined, h.request); const view = (await h.read())!, command = h.command(view)
    h.f.auth.privacy.changePolicy(h.f.scope, 'block')
    expect(await h.factory.adapter.status(command, h.request)).toEqual({ state: 'unknown' }); expect(await h.factory.adapter.readInvitation(ids.invitation, h.request)).toBeNull()
  })
  it('rejects a replacement signal even under the same visible scope', async () => {
    const h = setup(); expect(await h.factory.adapter.readSelection(h.intent, { scope: h.request.scope, signal: new AbortController().signal })).toBeNull(); expect(h.calls).toHaveLength(0)
  })
  it('does not resurrect an earlier asynchronous target after a new context read', async () => {
    const h = setup(), delayed = deferred<unknown>(); let count = 0
    h.override(descriptor => descriptor.operationId === 'resolveVenueMeetingContext' ? ++count === 1 ? delayed.promise : h.context() : h.page())
    const old = h.read(), current = h.read(); expect((await current)?.contextId).toBe(h.context().contextId)
    delayed.resolve(h.context()); expect(await old).toBeNull()
    expect((await h.read())?.contextId).toBe(h.context().contextId)
  })
})
