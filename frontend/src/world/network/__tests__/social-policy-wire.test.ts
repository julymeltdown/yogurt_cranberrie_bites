import { afterEach, expect, it, vi } from 'vitest'
import { createMemorySession } from '../../../shared/api/http'
import { MockWorldClock, MockWorldTransport } from '../mock'
import { decode, validate } from '../transport/contract'
import type { WorldEvent, WorldResources, WorldTicketResponse } from '../WorldTransport'
import movement from '../../../../../contracts/fixtures/realtime/movement-heartbeat.json'
import ticket from '../../../../../contracts/fixtures/realtime/ticket-response.json'

const authenticate = movement.messages[0]!.message
const snapshot = movement.messages[1]!.message
const resources: WorldResources = {
  eventId: authenticate.eventId!, roomId: authenticate.roomId!, mapId: authenticate.mapId!,
  mapRevision: authenticate.mapRevision!, avatarManifestId: authenticate.avatarManifestId!,
  avatarManifestVersion: authenticate.avatarManifestVersion!,
}
const baseline = { protocolVersion: '1.0.0', type: 'policy.baseline', streamId: snapshot.streamId!,
  eventId: resources.eventId, scope: 'SOCIAL', policyRevision: 1 }
const invalidation = { ...baseline, type: 'policy.invalidate', policyRevision: 2, reason: 'CONSENT_CHANGED' }
const stops: (() => void)[] = []
afterEach(() => { for (const stop of stops.splice(0)) stop() })
async function setup(admit = true) {
  const session = createMemorySession()
  session.replace(0, { userId: movement.actor_id, eventId: resources.eventId, accessToken: 'memory-only-test' })
  const clock = new MockWorldClock(Date.parse(ticket.server_time))
  const transport = new MockWorldTransport({ session, origin: 'https://mcr.example', scheduler: clock,
    acquireTicket: async () => ticket as WorldTicketResponse })
  stops.push(() => transport.dispose())
  const events: WorldEvent[] = []
  transport.subscribe(event => events.push(event))
  const connected = transport.connect(resources)
  void connected.catch(() => {})
  await vi.waitFor(() => expect(transport.state).toBe('opening'), { timeout: 2000, interval: 1 })
  const socket = transport.sockets[0]!
  socket.open()
  if (admit) { socket.receive(JSON.stringify(snapshot)); await connected }
  return { transport, socket, events, clock, connected, session }
}
it('keeps the existing strict Snapshot valid without policy metadata', () => {
  expect(validate('Snapshot', snapshot)).toBe(snapshot)
  expect(Object.hasOwn(snapshot, 'policyRevision')).toBe(false)
})
it.each([baseline, invalidation])('decodes the explicit metadata-only variant $type', frame => {
  expect(decode(JSON.stringify(frame), 'server')).toEqual(frame)
  expect(() => decode(JSON.stringify(frame), 'client')).toThrow()
  for (const patch of [{ policyRevision: 0 }, { policyRevision: Number.MAX_SAFE_INTEGER + 1 },
    { policyRevision: 1.5 }, { scope: 'WORLD' }, { userId: movement.actor_id }, { handle: '@private' },
    { eventId: 'wrong' }, { protocolVersion: '2.0.0' }]) {
    expect(() => decode(JSON.stringify({ ...frame, ...patch }), 'server')).toThrow()
  }
})
it('rejects unknown invalidation reasons and baseline-only extra reasons', () => {
  expect(() => decode(JSON.stringify({ ...invalidation, reason: 'ALLOW' }), 'server')).toThrow()
  expect(() => decode(JSON.stringify({ ...baseline, reason: 'CONSENT_CHANGED' }), 'server')).toThrow()
})
it('forwards both variants on the incumbent admitted World stream without opening another socket', async () => {
  const s = await setup()
  for (const frame of [baseline, invalidation]) s.socket.receive(JSON.stringify(frame))
  expect(s.transport.state).toBe('live')
  expect(s.transport.sockets).toHaveLength(1)
  expect(s.events.filter(event => event.type === 'frame').slice(-2)).toEqual([
    { type: 'frame', frame: baseline }, { type: 'frame', frame: invalidation },
  ])
})
it.each(['eventId', 'streamId'] as const)('rejects a mismatched policy %s before forwarding', async key => {
  const s = await setup()
  s.socket.receive(JSON.stringify({ ...baseline, [key]: movement.actor_id }))
  expect(s.transport.state).toBe('closed')
  expect(s.events.some(event => event.type === 'frame' && event.frame.type === 'policy.baseline')).toBe(false)
  expect(s.clock.pending).toBe(0)
})
it('requires the actual World snapshot before a policy baseline', async () => {
  const s = await setup(false)
  s.socket.receive(JSON.stringify(baseline))
  await expect(s.connected).rejects.toThrow()
  expect(s.transport.state).toBe('closed')
})
