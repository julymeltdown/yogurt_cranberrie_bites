// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { projectCapabilities } from '../../../app/capabilities'
import { validateWorldRooms } from '../model/contract'
import { catalog, errorResponse, flush, harness, initial, uid } from './fixtures/harness'
const owned: ReturnType<typeof harness>[] = []
const setup = (fetcher?: Parameters<typeof harness>[0], refresh?: boolean) => { const h = harness(fetcher, refresh); owned.push(h); return h }
afterEach(() => owned.splice(0).forEach(h => h.dispose()))
const scope = { generation: 1, userId: initial.selfUserId, eventId: initial.eventId }, path = `/api/concert/events/${scope.eventId}/rooms`, context = { scope, path, status: 200 }
describe('FE18 frozen C05 room reads', () => {
  it('uses existing oracle, closed WorldRooms, exact context, unique IDs and included default', () => {
    const decode = validateWorldRooms(scope), data = decode(catalog(), context)
    expect(Object.isFrozen(data.items[0])).toBe(true)
    for (const value of [{ ...catalog(), occupancy: 3 }, { ...catalog(), event_id: uid(900) }, { ...catalog(), default_room_id: uid(900) },
      { ...catalog(), items: [catalog().items[0], catalog().items[0]] }, { ...catalog(), items: [] },
      { ...catalog(), items: [{ ...catalog().items[0], capacity: 51 }] }, { ...catalog(), items: [{ ...catalog().items[0], availability: 'INVITATION_ONLY' }] }]) expect(() => decode(value, context)).toThrow()
    for (const c of [{ ...context, status: 202 }, { ...context, path: `${path}?bad` }, { ...context, scope: { ...scope, userId: uid(2) } }, { ...context, scope: { ...scope, generation: 2 } }]) expect(() => decode(catalog(), c)).toThrow()
  })
  it('performs canonical protected core GET/no-store and never persists private rooms', async () => {
    const h = setup(); await h.live(); const r = h.requests[0]!
    expect(r.method).toBe('GET'); expect(new URL(r.url).pathname).toBe(path); expect(r.cache).toBe('no-store'); expect(r.headers.get('Authorization')).toBe('Bearer fixture-token-memory-only')
    expect(r.headers.has('Idempotency-Key')).toBe(false); expect(h.auth.queryClient.getQueryCache().getAll()).toHaveLength(0)
  })
  it.each([403, 404, 429, 503])('presents safe failure %i with no stale catalog', async status => {
    const h = setup(() => errorResponse(status, status === 403 ? 'PERMISSION_DENIED' : status === 404 ? 'RESOURCE_UNAVAILABLE' : status === 429 ? 'RATE_LIMITED' : 'DEPENDENCY_UNAVAILABLE')); await h.live()
    expect(h.controller.read().rooms.phase).toBe('error'); expect(h.controller.read().rooms.data).toBeNull(); expect(JSON.stringify(h.controller.read())).not.toContain('PRIVATE RAW')
    if (status === 429) { const count = h.requests.length; await h.controller.loadRooms(); expect(h.requests).toHaveLength(count) }
  })
  it('uses the canonical qualified refresh owner exactly once; feature never supplies a refresh override', async () => {
    let roomReads = 0
    const h = setup(r => {
      if (new URL(r.url).pathname === '/api/auth/refresh') return Response.json({ userId: scope.userId, accessToken: 'fresh-fixture-token', tokenType: 'Bearer', refreshToken: '', linkedProviders: [], expiresIn: 3600 })
      return ++roomReads === 1 ? errorResponse(401, 'AUTHENTICATION_REQUIRED') : Response.json(catalog())
    }, true)
    await h.live(); expect(h.controller.read().rooms.phase).toBe('ready'); expect(h.requests.filter(r => new URL(r.url).pathname === '/api/auth/refresh')).toHaveLength(1); expect(roomReads).toBe(2)
    expect(h.auth.session.read().userId).toBe(scope.userId)
  })
})
describe('FE18 switch authority and bounded ownership', () => {
  it('treats switchRoom true as dispatch only and suppresses duplicate switches', async () => {
    const h = setup(); await h.live(); const first = h.controller.switchRoom(uid(12)), second = h.controller.switchRoom(uid(12)); await Promise.all([first, second])
    expect(h.transport.sockets[0]!.sent.map(s => JSON.parse(s)).filter(s => s.type === 'switchRoom')).toHaveLength(1)
    expect(h.controller.read()).toMatchObject({ currentRoomId: initial.roomId, switch: { phase: 'switching' } }); h.admit()
    expect(h.controller.read()).toMatchObject({ currentRoomId: uid(12), switch: { phase: 'admitted' } }); expect(h.controller.read().nearby.map(p => p.userId)).toEqual([scope.userId])
  })
  it('synchronously withdraws a nullable shell projection and stays subscribed for a fresh authorized baseline', async () => {
    const h = setup(); await h.live(); const original = h.replication.read.bind(h.replication), observers = new Set<() => void>()
    let value: ReturnType<typeof original> | null = original()
    const adapter = { read: () => value, subscribe(fn: () => void) { observers.add(fn); return () => { observers.delete(fn) } }, switchRoom: h.replication.switchRoom.bind(h.replication) }
    h.publish({ ...h.release()!, replication: adapter }); await h.controller.loadRooms()
    value = null; [...observers].forEach(fn => fn())
    expect(h.controller.read()).toMatchObject({ available: false, nearby: [], currentRoomId: null, rooms: { data: null } }); expect(observers.size).toBe(1)
    const count = h.requests.length; await h.controller.loadRooms(); expect(h.requests).toHaveLength(count)
    await h.controller.switchRoom(uid(12)); expect(h.transport.sockets[0]!.sent).toHaveLength(1)
    value = original(); [...observers].forEach(fn => fn()); await flush()
    expect(h.controller.read().available).toBe(true); expect(h.controller.read().nearby).toHaveLength(3); expect(h.controller.read().rooms.phase).toBe('ready')
  })
  it('does not dispatch if a synchronous observer revokes access while publishing intent', async () => {
    const h = setup(); await h.live()
    const stop = h.controller.subscribe(() => { if (h.controller.read().switch.phase === 'switching') h.auth.privacy.changePolicy(h.auth.privacy.capture(), 'block') })
    await h.controller.switchRoom(uid(12)); stop(); expect(h.transport.sockets[0]!.sent).toHaveLength(1); expect(h.controller.read().nearby).toEqual([])
  })
  it('keeps admission when a replacement arrives before the dispatch promise resolves false', async () => {
    const h = setup(); await h.live(); const send = h.replication.switchRoom.bind(h.replication)
    vi.spyOn(h.replication, 'switchRoom').mockImplementation(async room => { await send(room); h.admit(); return false })
    await h.controller.switchRoom(uid(12))
    expect(h.controller.read()).toMatchObject({ currentRoomId: uid(12), switch: { phase: 'admitted' } })
  })
  it('cannot admit a stale request or keep a pending switch after transport authority is invalidated', async () => {
    const h = setup(); await h.live(); await h.controller.switchRoom(uid(12))
    h.receive({ ...initial, cause: 'ROOM_SWITCH', requestId: uid(999), roomId: uid(12), streamId: uid(405), roomEpoch: uid(406), players: [initial.players[0]] })
    expect(h.controller.read().switch.phase).toBe('interrupted'); expect(h.controller.read().nearby).toEqual([])
    expect(h.controller.read().currentRoomId).not.toBe(uid(12)); expect(h.controller.switchReason(uid(12))).not.toBeNull()
  })
  it('full and unavailable catalog rows cannot dispatch; actual server rejection preserves old room and roster', async () => {
    for (const availability of ['FULL', 'UNAVAILABLE'] as const) { const blocked = setup(() => Response.json(catalog(availability))); await blocked.live(); await blocked.controller.switchRoom(uid(12)); expect(blocked.controller.switchReason(uid(12))).toBe(availability === 'FULL' ? 'This room is full.' : 'This room is unavailable.'); expect(blocked.transport.sockets[0]!.sent).toHaveLength(1) }
    const h = setup(); await h.live(); await h.controller.switchRoom(uid(12)); h.reject(); await flush()
    expect(h.controller.read()).toMatchObject({ currentRoomId: initial.roomId, switch: { phase: 'rejected' } }); expect(h.controller.read().nearby).toHaveLength(3); expect(h.replication.read().baseline!.roomId).toBe(initial.roomId)
  })
  it('epoch replacement removes the old roster without claiming room-switch admission', async () => {
    const h = setup(); await h.live(); h.receive({ ...initial, cause: 'EPOCH_CHANGED', streamId: uid(403), roomEpoch: uid(404), players: [initial.players[0]] })
    expect(h.controller.read().nearby).toHaveLength(1); expect(h.controller.read().switch.phase).toBe('idle')
    h.replication.setVisible(false); expect(h.controller.read().nearby).toEqual([]); expect(h.controller.switchReason(uid(12))).not.toBeNull()
  })
  it('delta removal and resync clear the exact authorized projection; only authorized avatar data reaches the portrait projection; no coordinates or distance leak', async () => {
    const h = setup(); await h.live(); const b = h.replication.read().baseline!
    h.receive({ protocolVersion: '1.0.0', type: 'delta', streamId: b.streamId, roomEpoch: b.roomEpoch, streamSeq: 1, serverTick: 2, ackInputSeq: 0, upserts: [], removes: [uid(2)] })
    expect(h.controller.read().nearby.map(p => p.userId)).not.toContain(uid(2)); expect(h.projected.at(-1)).not.toContain(uid(2)); expect(h.controller.read().nearby[0]).toEqual({ userId: uid(1), displayName: 'Raven', self: true, moving: false, avatar: initial.players[0]!.avatar })
    expect(JSON.stringify(h.controller.read().nearby)).not.toMatch(/position|distance/)
    h.receive({ protocolVersion: '1.0.0', type: 'delta', streamId: b.streamId, roomEpoch: b.roomEpoch, streamSeq: 10, serverTick: 20, ackInputSeq: 0, upserts: [], removes: [] })
    expect(h.controller.read().nearby).toEqual([]); expect(h.controller.read().phase).toBe('resync')
  })
  it.each(['policy', 'identity', 'event', 'access', 'dispose'] as const)('clears private feature state on %s without taking replication ownership', async kind => {
    const h = setup(); await h.live(); const release = h.release()!, old = h.auth.privacy.capture(), closes = h.transport.sockets[0]!.closeCount
    if (kind === 'policy') h.auth.privacy.changePolicy(old, 'block')
    else if (kind === 'identity' || kind === 'event') h.auth.session.replace(old.generation, { userId: kind === 'identity' ? uid(2) : uid(1), eventId: kind === 'event' ? uid(99) : scope.eventId, accessToken: 'replacement-fixture-token' })
    else if (kind === 'access') h.publish({ ...release, capabilities: projectCapabilities({ ...release.capabilities, world: { available: false, reason: 'FEATURE_DISABLED' } }) })
    else h.controller.dispose()
    expect(h.controller.read().rooms.data).toBeNull(); expect(h.controller.read().nearby).toEqual([]); expect(h.controller.read().switch.phase).toBe('idle')
    if (kind === 'policy' || kind === 'dispose' || kind === 'access') expect(h.transport.sockets[0]!.closeCount).toBe(closes)
    if (kind === 'policy') { h.publish(release); expect(h.controller.read().available).toBe(false); expect(h.auth.session.read().generation).toBe(old.generation) }
  })
  it('drops noncooperative late room reads after policy change and performs no work after disposal', async () => {
    let resolve!: (r: Response) => void
    const h = setup(() => new Promise(r => { resolve = r })); h.controller.start(); const pending = h.controller.loadRooms(); await flush()
    h.auth.privacy.changePolicy(h.auth.privacy.capture(), 'consent-revoked'); resolve(Response.json(catalog())); await pending; expect(h.controller.read().rooms.data).toBeNull()
    h.controller.dispose(); const count = h.requests.length; await h.controller.loadRooms(); await h.controller.switchRoom(uid(12)); expect(h.requests).toHaveLength(count)
  })
})
