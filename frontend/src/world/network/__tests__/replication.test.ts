import { afterEach, describe, expect, it, vi } from 'vitest'
import { createMemorySession } from '../../../shared/api/http'
import { TransportError, parseServerError } from '../../../shared/api/errors'
import { MockWorldClock, MockWorldTransport } from '../mock'
import { createWorldReplication, type ReplicationView, type WorldReplication } from '../replication'
import type { Snapshot, WorldPlayer, WorldTicketResponse, WorldTransportOptions } from '../WorldTransport'
import movement from '../../../../../contracts/fixtures/realtime/movement-heartbeat.json'
import ticket from '../../../../../contracts/fixtures/realtime/ticket-response.json'
import filtered from '../../../../../contracts/fixtures/realtime/filtered-viewers.json'

const initial = movement.messages[1]!.message as Snapshot
const resources = { eventId: initial.eventId, roomId: initial.roomId, mapId: initial.mapId, mapRevision: initial.mapRevision,
  avatarManifestId: initial.avatarManifestId, avatarManifestVersion: initial.avatarManifestVersion }
const id = (n: number) => `00000000-0000-4000-8000-${String(n).padStart(12, '0')}`
const flush = async () => { for (let i = 0; i < 20; i++) await Promise.resolve(); await new Promise(r => setTimeout(r, 5)) }
const owned: WorldReplication[] = []
afterEach(() => owned.splice(0).forEach(c => c.dispose()))
function setup(patch: Partial<WorldTransportOptions> = {}, sink?: (view: ReplicationView) => void, admitted = initial) {
  const session = createMemorySession(); session.replace(0, { userId: admitted.selfUserId, eventId: resources.eventId, accessToken: 'synthetic-only-token' })
  const clock = new MockWorldClock(Date.parse(ticket.server_time)), commits: ReplicationView[] = []
  let issued = 0, requests = 100
  const acquire = vi.fn(async ({ body }: Parameters<WorldTransportOptions['acquireTicket']>[0]) => ({ ...ticket, ...body, ticket: String.fromCharCode(65 + issued++).repeat(43) }) as WorldTicketResponse)
  const transport = new MockWorldTransport({ session, origin: 'https://mcr.example', scheduler: clock, acquireTicket: acquire, ...patch })
  const c = createWorldReplication({ session, resources, transport, scheduler: clock, requestId: () => id(requests++), sink: { replace: value => { commits.push(value); sink?.(value) } } })
  owned.push(c)
  const socket = () => transport.sockets.at(-1)!
  const receive = (data: unknown) => socket().receive(JSON.stringify(data))
  return { c, session, clock, transport, commits, acquire, socket, receive,
    async live() { const p = c.start(); await flush(); socket().open(); receive(admitted); expect(await p).toBe(true) },
    delta(seq: number, upserts: readonly WorldPlayer[] = [], removes: readonly string[] = [], extra = {}) {
      receive({ protocolVersion: '1.0.0', type: 'delta', streamId: c.read().baseline!.streamId, roomEpoch: c.read().baseline!.roomEpoch,
        streamSeq: seq, serverTick: seq * 2, ackInputSeq: 0, upserts, removes, ...extra })
    },
  }
}
describe('FE17 authoritative entity projection', () => {
  for (const viewer of filtered.viewers) it(`consumes canonical filtered-viewer stream for ${viewer.actor_id}`, async () => {
    const admitted = viewer.initial[1]!.message as Snapshot
    const s = setup({}, undefined, admitted); await s.live()
    const frames = filtered.expected[viewer.actor_id as keyof typeof filtered.expected]
    for (const frame of frames) s.receive(frame)
    expect(s.c.read().phase).toBe('live'); expect(s.c.read().baseline!.streamSeq).toBe(frames.length)
    const ids = s.c.read().entities.map(e => e.player.userId)
    if (viewer.actor_id === id(2)) expect(ids).not.toContain(id(3))
    if (viewer.actor_id === id(3)) expect(ids).not.toContain(id(2))
    expect(s.socket().sent.map(x => JSON.parse(x).type)).toEqual(['authenticate'])
  })
  it('replaces snapshots, applies atomic upserts/removals and never retains snapshot.players', async () => {
    const s = setup(); await s.live()
    expect(s.c.read().entities).toHaveLength(3); expect(s.c.read().baseline).not.toHaveProperty('players')
    const changed = { ...initial.players[1]!, displayName: 'New name', avatar: { ...initial.players[1]!.avatar, revision: 2 } }
    s.delta(1, [changed], [initial.players[2]!.userId])
    expect(s.c.read().entities.map(e => e.player.displayName)).toEqual(['Raven', 'New name'])
    expect(s.c.read().entities[1]!.player.avatar.revision).toBe(2)
    expect(Object.isFrozen(s.c.read().entities[1]!.player.avatar.config.item_ids)).toBe(true)
    const ref = s.c.read(); s.receive({ ...initial, cause: 'EPOCH_CHANGED', roomEpoch: id(300), streamId: id(301), players: [initial.players[0]], serverTick: 0 })
    expect(s.c.read().entities).toHaveLength(1); expect(s.c.read().baseline!.streamSeq).toBe(0); expect(ref).not.toBe(s.c.read())
  })
  it('interpolates only remote positions within one 100ms window and never extrapolates/local-predicts', async () => {
    const s = setup(); await s.live()
    s.delta(1, initial.players.slice(0, 2).map(p => ({ ...p, position: { x: 132, y: 160 } })))
    expect(s.c.read().entities[0]!.position).toEqual({ x: 132, y: 160 })
    expect(s.c.read().entities[1]!.position).toEqual({ x: 32, y: 60 })
    s.clock.advance(50); s.c.sample(); expect(s.c.read().entities[1]!.position).toEqual({ x: 82, y: 110 })
    s.clock.advance(200); s.c.sample(); expect(s.c.read().entities[1]!.position).toEqual({ x: 132, y: 160 })
    expect(s.c.read().entities[1]!.player.position).toEqual({ x: 132, y: 160 }); expect(s.c.read().spatialPrediction).toBe('unavailable')
    s.delta(2, [], [initial.players[1]!.userId]); expect(s.c.sample().entities.some(e => e.player.userId === initial.players[1]!.userId)).toBe(false)
  })
  it('accepts tick skips without treating viewer-filtered changes as missing deltas', async () => {
    const s = setup(); await s.live(); s.delta(1, [], [], { serverTick: 900 }); s.delta(2, [], [], { serverTick: 1200 })
    expect(s.c.read().phase).toBe('live'); expect(s.c.read().baseline!.streamSeq).toBe(2)
    expect(s.socket().sent.map(x => JSON.parse(x).type)).toEqual(['authenticate'])
  })
  it('epoch mismatch requests epoch recovery and clears old positions', async () => {
    const s = setup(); await s.live(); s.delta(1, [], [], { roomEpoch: id(800) })
    expect(s.c.read().entities).toEqual([]); expect(JSON.parse(s.socket().sent.at(-1)!)).toMatchObject({ type: 'resync', reason: 'EPOCH_CHANGED' })
  })
  it('retains at most the current capacity roster under repeated coalesced position changes', async () => {
    const s = setup(); await s.live()
    const added = Array.from({ length: 47 }, (_, i) => ({ ...initial.players[1]!, userId: id(500 + i) }))
    s.delta(1, added); expect(s.c.read().entities).toHaveLength(50)
    for (let seq = 2; seq <= 202; seq++) s.delta(seq, [{ ...added[0]!, position: { x: seq, y: 60 } }])
    expect(s.c.read().entities).toHaveLength(50); expect(s.c.read().baseline).not.toHaveProperty('players')
    expect(s.c.read().entities.find(e => e.player.userId === added[0]!.userId)!.player.position.x).toBe(202)
  })
  it('a gap clears ghosts, requests one resync, discards queued old changes and replaces the complete baseline', async () => {
    const s = setup(); await s.live(); s.c.setIntent({ axisX: 1, axisY: 0, buttons: ['interact'] }); s.clock.advance(50)
    s.delta(3); expect(s.c.read().phase).toBe('resync'); expect(s.c.read().entities).toEqual([]); expect(s.c.read().pendingInputs).toBe(0)
    const req = JSON.parse(s.socket().sent.at(-1)!); expect(req.type).toBe('resync'); expect(req.lastStreamSeq).toBe(0)
    s.delta(4); expect(s.socket().sent.filter(x => JSON.parse(x).type === 'resync')).toHaveLength(1)
    s.receive({ ...initial, streamId: id(120), cause: 'RESYNC', requestId: req.requestId, players: [initial.players[0]], serverTick: 8 })
    s.clock.advance(50); expect(JSON.parse(s.socket().sent.at(-1)!)).toMatchObject({ type: 'input', seq: 1, axisX: 1, buttons: [] })
    expect(s.c.read().entities).toHaveLength(1)
  })
  it('spaces repeated resyncs without adding another recovery or reconnect loop', async () => {
    const s = setup(); await s.live(); s.delta(2)
    const req = JSON.parse(s.socket().sent.at(-1)!)
    s.receive({ ...initial, streamId: id(120), cause: 'RESYNC', requestId: req.requestId })
    s.delta(2); expect(s.c.read().phase).toBe('resync')
    expect(s.socket().sent.filter(x => JSON.parse(x).type === 'resync')).toHaveLength(1)
    s.clock.advance(999); expect(s.socket().sent.filter(x => JSON.parse(x).type === 'resync')).toHaveLength(1)
    s.clock.advance(1); expect(s.socket().sent.filter(x => JSON.parse(x).type === 'resync')).toHaveLength(2)
    s.c.dispose(); expect(s.clock.pending).toBe(0)
  })
})
describe('FE17 bounded input and transport lifecycle', () => {
  it('coalesces normalized axes to 20Hz, sends interactions once and reconciles acknowledgements without replay', async () => {
    const s = setup(); await s.live()
    expect(s.c.setIntent({ axisX: 2, axisY: 0, buttons: [] })).toBe(false)
    s.c.setIntent({ axisX: 1, axisY: 0, buttons: ['interact'] }); s.c.setIntent({ axisX: 0, axisY: 1, buttons: [] })
    s.clock.advance(49); expect(s.socket().sent).toHaveLength(1)
    s.clock.advance(1); expect(JSON.parse(s.socket().sent.at(-1)!)).toMatchObject({ seq: 1, axisX: 0, axisY: 1, buttons: ['interact'] })
    s.clock.advance(50); expect(JSON.parse(s.socket().sent.at(-1)!)).toMatchObject({ seq: 2, buttons: [] }); expect(s.c.read().pendingInputs).toBe(2)
    s.delta(1, [{ ...initial.players[0]!, position: { x: 34, y: 61 } }], [], { ackInputSeq: 1 })
    expect(s.c.read().pendingInputs).toBe(1); expect(s.c.read().entities[0]!.position).toEqual({ x: 34, y: 61 })
    expect(s.c.setIntent({ axisX: 0, axisY: 0, buttons: [], pickup: 'never' } as never)).toBe(false)
  })
  it('bounds unacknowledged input at 64 and recovers instead of growing a replay queue', async () => {
    const s = setup(); await s.live(); s.c.setIntent({ axisX: 1, axisY: 0, buttons: [] }); s.clock.advance(3200); expect(s.c.read().pendingInputs).toBe(64)
    s.clock.advance(50); expect(s.c.read().phase).toBe('resync'); expect(s.c.read().pendingInputs).toBe(0)
    expect(s.socket().sent.filter(x => JSON.parse(x).type === 'input')).toHaveLength(64)
    expect(s.socket().sent.at(-1)).toContain('PROTOCOL_RECOVERY')
  })
  it('does not accumulate an idle replay ledger or run an idle cadence after sending neutral once', async () => {
    const s = setup(); await s.live(); s.clock.advance(4000)
    expect(s.c.read().phase).toBe('live'); expect(s.c.read().pendingInputs).toBe(1)
    expect(s.socket().sent.filter(x => JSON.parse(x).type === 'input')).toHaveLength(1)
    s.c.setIntent({ axisX: 1, axisY: 0, buttons: [] }); s.clock.advance(50)
    expect(JSON.parse(s.socket().sent.at(-1)!)).toMatchObject({ seq: 2, axisX: 1 })
  })
  it.each(['MAP_VERSION_MISMATCH', 'BACKPRESSURE'] as const)('terminal %s clears roster without an in-place resync', async reason => {
    const s = setup(); await s.live(); s.receive({ protocolVersion: '1.0.0', type: 'resyncRequired', streamId: initial.streamId, reason })
    expect(s.c.read().entities).toEqual([]); expect(s.c.read().baseline).toBeNull(); expect(s.clock.pending).toBe(0)
    expect(s.socket().sent.some(x => JSON.parse(x).type === 'resync')).toBe(false)
    expect(s.c.read().problem).toBe(reason === 'MAP_VERSION_MISMATCH' ? 'map-version' : 'connection')
    if (reason === 'MAP_VERSION_MISMATCH') expect(await s.c.reconnect()).toBe(false)
  })
  it('responds to the existing heartbeat event without owning a heartbeat timer', async () => {
    const s = setup(); await s.live(); s.receive(movement.messages[4]!.message)
    expect(JSON.parse(s.socket().sent.at(-1)!)).toMatchObject({ type: 'pong', heartbeatId: movement.messages[4]!.message.heartbeatId })
    expect(s.clock.pending).toBe(3) // transport access + heartbeat, one replication input cadence.
  })
  it('hidden teardown removes ghosts/listeners, foreground gets a fresh ticket once and replays no old intent', async () => {
    const s = setup(); await s.live(); const old = s.socket()
    s.c.setIntent({ axisX: 1, axisY: 0, buttons: ['interact'] }); s.c.setVisible(false)
    expect(s.c.read().phase).toBe('suspended'); expect(s.c.read().entities).toEqual([]); expect(s.clock.pending).toBe(0)
    expect([...old.listeners.values()].every(set => set.size === 0)).toBe(true)
    s.c.setVisible(true); s.c.setVisible(true); await flush(); expect(s.acquire).toHaveBeenCalledTimes(2)
    s.socket().open(); s.receive(initial); s.clock.advance(50)
    expect(JSON.parse(s.socket().sent.at(-1)!)).toMatchObject({ type: 'input', seq: 1, axisX: 0, axisY: 0, buttons: [] })
    s.c.dispose(); expect(s.clock.pending).toBe(0)
  })
  it('can start hidden without acquiring a ticket until first foreground', async () => {
    const s = setup(); s.c.setVisible(false); expect(await s.c.start()).toBe(false); expect(s.acquire).not.toHaveBeenCalled()
    s.c.setVisible(true); await flush(); expect(s.acquire).toHaveBeenCalledTimes(1)
    s.socket().open(); s.receive(initial); expect(s.c.read().entities).toHaveLength(3)
  })
  it('does not reconnect on unexpected close until explicit retry', async () => {
    const s = setup(); await s.live(); s.socket().serverClose(4500); s.clock.advance(1000)
    expect(s.c.read().entities).toEqual([]); expect(s.acquire).toHaveBeenCalledTimes(1)
    const retry = s.c.reconnect(); await flush(); s.socket().open(); s.receive(initial); expect(await retry).toBe(true)
  })
  it('canonical owner replacement while a ticket is pending disposes and ignores its eventual result', async () => {
    let resolve!: (value: WorldTicketResponse) => void
    const pending = new Promise<WorldTicketResponse>(r => { resolve = r }), s = setup({ acquireTicket: () => pending })
    const started = s.c.start(); s.session.replace(s.session.read().generation, { userId: id(2), eventId: resources.eventId, accessToken: 'new-owner' })
    resolve(ticket as WorldTicketResponse); expect(await started).toBe(false); await flush()
    expect(s.c.read().entities).toEqual([]); expect(s.c.read().baseline).toBeNull(); expect(s.transport.state).toBe('disposed')
    expect(await s.c.reconnect()).toBe(false); expect(s.transport.sockets).toHaveLength(0); expect(s.clock.pending).toBe(0)
  })
  it('revoke clears all data and disables visibility takeover and retries', async () => {
    const s = setup(); await s.live(); s.receive({ protocolVersion: '1.0.0', type: 'revoke', streamId: initial.streamId, reason: 'CONNECTION_REPLACED' })
    expect(s.c.read().entities).toEqual([]); expect(s.c.read().baseline).toBeNull(); s.c.setVisible(false); s.c.setVisible(true)
    expect(await s.c.reconnect()).toBe(false); expect(s.acquire).toHaveBeenCalledTimes(1)
  })
  it('sink failure and reentrant disposal cannot leave an active transport, timers or later commits', async () => {
    const broken = setup({}, view => { if (view.entities.length) throw Error('private sink error') }); await broken.live().catch(() => {})
    expect(broken.c.read().problem).toBe('sink'); expect(broken.transport.state).toBe('disposed'); expect(broken.clock.pending).toBe(0)
    const s = setup(); s.c.subscribe(() => { if (s.c.read().entities.length) s.c.dispose() })
    const p = s.c.start(); await flush(); s.socket().open(); s.receive(initial); expect(await p).toBe(false)
    const n = s.commits.length; s.clock.advance(10000); s.c.sample(); expect(s.commits).toHaveLength(n)
    expect(s.clock.pending).toBe(0); expect(s.c.read().phase).toBe('disposed')
  })
  it('room ticket denial preserves old baseline and movement; target snapshot atomically resets roster', async () => {
    let denied = true
    const s = setup({ acquireTicket: async ({ body }) => {
      if (body.room_id !== resources.roomId && denied) throw new TransportError('http', { status: 409,
        server: parseServerError({ code: 'room', message: 'private', details: { reason: 'RESOURCE_UNAVAILABLE' } }, 'core', 409) })
      return { ...ticket, ...body, ticket: body.room_id === resources.roomId ? 'A'.repeat(43) : 'B'.repeat(43) } as WorldTicketResponse
    } }); await s.live(); expect(await s.c.switchRoom(id(90))).toBe(false)
    expect(s.c.read().baseline!.roomId).toBe(resources.roomId); expect(s.c.read().entities).toHaveLength(3)
    s.clock.advance(50); expect(JSON.parse(s.socket().sent.at(-1)!)).toMatchObject({ type: 'input', seq: 1 })
    denied = false; expect(await s.c.switchRoom(id(90))).toBe(true)
    const req = JSON.parse(s.socket().sent.at(-1)!)
    const beforeAdmission = s.commits.length
    s.receive({ ...initial, streamId: id(200), roomId: id(90), roomEpoch: id(201), cause: 'ROOM_SWITCH', requestId: req.requestId, players: [initial.players[0]] })
    expect(s.commits.slice(beforeAdmission).filter(view => view.phase === 'live').every(view => view.baseline?.roomId === id(90))).toBe(true)
    expect(s.c.read().baseline!.roomId).toBe(id(90)); expect(s.c.read().entities).toHaveLength(1); expect(s.c.read().pendingInputs).toBe(0)
  })
})
