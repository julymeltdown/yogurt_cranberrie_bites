import { afterEach, describe, expect, it, vi } from 'vitest'
import { createMemorySession } from '../../../../shared/api/http'
import { MockWorldClock, MockWorldTransport } from '../../mock'
import { decode, validate } from '../../transport/contract'
import type { Delta, Snapshot, WorldTicketResponse, WorldTransportOptions } from '../../WorldTransport'
import negotiated from '../../../../../../contracts/fixtures/realtime/motion-v1/negotiated.json'
import movement from '../../../../../../contracts/fixtures/realtime/movement-heartbeat.json'
import ticket from '../../../../../../contracts/fixtures/realtime/ticket-response.json'

const initial = negotiated.messages[1]!.message as Snapshot
const legacy = movement.messages[1]!.message as Snapshot
const resources = { eventId: initial.eventId, roomId: initial.roomId, mapId: initial.mapId, mapRevision: initial.mapRevision,
  avatarManifestId: initial.avatarManifestId, avatarManifestVersion: initial.avatarManifestVersion }
const owned: MockWorldTransport[] = []
afterEach(() => owned.splice(0).forEach(transport => transport.dispose()))
async function setup(optin = true, admitted: unknown = initial, mutate?: (options: WorldTransportOptions) => void) {
  const session = createMemorySession(); session.replace(0, { userId: initial.selfUserId, eventId: initial.eventId, accessToken: 'synthetic-only' })
  const clock = new MockWorldClock(Date.parse(ticket.server_time))
  const options: WorldTransportOptions = { session, scheduler: clock, origin: 'https://mcr.example',
    ...(optin ? { motionProfileVersion: '1.0.0' as const } : {}), acquireTicket: async ({ body }) => ({ ...ticket, ...body }) as WorldTicketResponse }
  const transport = new MockWorldTransport(options); owned.push(transport); mutate?.(options)
  const connected = transport.connect(resources); void connected.catch(() => {})
  await vi.waitFor(() => { expect(transport.sockets).toHaveLength(1); expect(transport.state).toBe('opening') })
  const socket = transport.sockets[0]!
  socket.open(); socket.receive(JSON.stringify(admitted))
  return { transport, socket, options, connected, clock,
    receive(frame: unknown) { socket.receive(JSON.stringify(frame)) },
    input(seq: number) { transport.sendInput({ seq, axisX: .5, axisY: 0, buttons: [] }) },
  }
}
const delta = (patch: Partial<Delta> = {}): Delta => ({ protocolVersion: '1.0.0', type: 'delta', streamId: initial.streamId,
  roomEpoch: initial.roomEpoch, streamSeq: 1, serverTick: 2, ackInputSeq: 1, upserts: [], removes: [],
  selfMotion: { serverTick: 2, position: initial.selfMotion!.position, consumedInputSeq: 1, inputAppliedTicks: 1, droppedTicks: 0,
    heldInput: { seq: 1, axisX: .5, axisY: 0, remainingHoldMs: 200 } }, ...patch })

describe('C05 explicit motion negotiation and strict schema', () => {
  it('keeps exact legacy Authenticate shape by default and captures explicit optin immutably', async () => {
    const old = await setup(false, legacy, options => Object.assign(options, { motionProfileVersion: '1.0.0' })); await old.connected
    expect(JSON.parse(old.socket.sent[0]!)).toEqual(movement.messages[0]!.message)
    const next = await setup(true, initial, options => Object.assign(options, { motionProfileVersion: undefined })); await next.connected
    expect(JSON.parse(next.socket.sent[0]!)).toEqual(negotiated.messages[0]!.message)
    expect(next.transport.state).toBe('live')
  })
  it('permits authoritative fallback when an explicitly requested profile is absent', async () => {
    const s = await setup(true, legacy); await s.connected
    s.input(1)
    const { selfMotion: _motion, ...frame } = delta(); s.receive(frame)
    expect(s.transport.state).toBe('live')
  })
  it('rejects unsolicited negotiated motion and releases admission resources', async () => {
    const s = await setup(false); await expect(s.connected).rejects.toThrow()
    expect(s.transport.state).toBe('closed'); expect(s.clock.pending).toBe(0)
  })
  for (const version of [null, true, '2.0.0']) it(`rejects unsupported wire version ${version}`, () => {
    expect(() => decode(JSON.stringify({ ...negotiated.messages[0]!.message, motionProfileVersion: version }), 'client')).toThrow()
  })
  it('implements dependentRequired in both directions and exclusiveMinimum rather than ignoring schema keywords', () => {
    const { motionProfile, selfMotion, ...base } = initial
    expect(() => validate('Snapshot', { ...base, motionProfile })).toThrow()
    expect(() => validate('Snapshot', { ...base, selfMotion })).toThrow()
    for (const speedPxPerSecond of [0, -1, Infinity]) expect(() => validate('MotionProfile', { ...motionProfile, speedPxPerSecond })).toThrow()
    expect(() => validate('MotionProfile', { ...motionProfile, speedPxPerSecond: .001 })).not.toThrow()
    expect(() => validate('Snapshot', { ...initial, motionProfile: { ...motionProfile, hiddenSpeed: 160 } })).toThrow()
  })
  it('consumes expired input independently of ACK and accepts a complete anchor without a self upsert', async () => {
    const s = await setup(); await s.connected; s.input(1)
    s.receive(delta({ serverTick: 5, ackInputSeq: 0, selfMotion: { serverTick: 5, position: { x: 48, y: 48 }, consumedInputSeq: 1,
      inputAppliedTicks: 0, droppedTicks: 1, heldInput: null } }))
    expect(s.transport.state).toBe('live')
  })
  const bad: [string, (frame: Delta) => Delta][] = [
    ['missing anchor', frame => { const { selfMotion: _motion, ...rest } = frame; return rest }],
    ['future consumption', frame => ({ ...frame, selfMotion: { ...frame.selfMotion!, consumedInputSeq: 2 } })],
    ['mismatched tick', frame => ({ ...frame, selfMotion: { ...frame.selfMotion!, serverTick: 1 } })],
    ['hold beyond profile', frame => ({ ...frame, selfMotion: { ...frame.selfMotion!, heldInput: { ...frame.selfMotion!.heldInput!, remainingHoldMs: 251 } } })],
    ['conflicting self upsert', frame => ({ ...frame, upserts: [{ ...initial.players[0]!, position: { x: 100, y: 100 } }] })],
    ['unbounded duration', frame => ({ ...frame, selfMotion: { ...frame.selfMotion!, inputAppliedTicks: 3 } })],
    ['wrong held ACK', frame => ({ ...frame, selfMotion: { ...frame.selfMotion!, heldInput: { ...frame.selfMotion!.heldInput!, seq: 2 } } })],
  ]
  for (const [name, corrupt] of bad) it(`rejects ${name} without retaining authority`, async () => {
    const s = await setup(); await s.connected; s.input(1); s.receive(corrupt(delta()))
    expect(s.transport.state).toBe('closed'); expect(s.clock.pending).toBe(0)
  })
  it('rejects counter regression and advancing applied time without actual model ticks', async () => {
    for (const patch of [{ consumedInputSeq: 0 }, { inputAppliedTicks: 0 }, { inputAppliedTicks: 2 }]) {
      const s = await setup(); await s.connected; s.input(1); s.receive(delta()); expect(s.transport.state).toBe('live')
      s.receive(delta({ streamSeq: 2, selfMotion: { ...delta().selfMotion!, ...patch } }))
      expect(s.transport.state).toBe('closed')
    }
  })
  it('discards valid queued old anchors during resync and resets stream bookkeeping without replay', async () => {
    const s = await setup(); await s.connected; s.input(1); s.receive(delta())
    const requestId = '00000000-0000-4000-8000-000000000201'
    s.transport.resync(requestId, 'GAP')
    s.receive(delta({ streamSeq: 999, selfMotion: { ...delta().selfMotion!, consumedInputSeq: 100 } }))
    expect(s.transport.state).toBe('resync')
    s.receive({ ...initial, streamId: '00000000-0000-4000-8000-000000000202', serverTick: 2,
      cause: 'RESYNC', requestId, selfMotion: { ...initial.selfMotion!, serverTick: 2 } })
    expect(s.transport.state).toBe('live')
    s.input(1); expect(JSON.parse(s.socket.sent.at(-1)!)).toMatchObject({ type: 'input', seq: 1, buttons: [] })
  })
  it('rejects a dropped-tick regression both within a stream and across same-room resync', async () => {
    for (const replacement of [false, true]) {
      const s = await setup(); await s.connected; s.input(1)
      s.receive(delta({ selfMotion: { ...delta().selfMotion!, droppedTicks: 1 } })); expect(s.transport.state).toBe('live')
      if (replacement) {
        const requestId = '00000000-0000-4000-8000-000000000203'
        s.transport.resync(requestId, 'GAP')
        s.receive({ ...initial, streamId: '00000000-0000-4000-8000-000000000204', serverTick: 4, cause: 'RESYNC', requestId,
          selfMotion: { ...initial.selfMotion!, serverTick: 4 } })
      } else s.receive(delta({ streamSeq: 2, serverTick: 4, selfMotion: { ...delta().selfMotion!, serverTick: 4, inputAppliedTicks: 2 } }))
      expect(s.transport.state).toBe('closed'); expect(s.clock.pending).toBe(0)
    }
  })
})
