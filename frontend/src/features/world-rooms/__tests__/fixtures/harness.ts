import { expect, vi } from 'vitest'
import { CAPABILITIES, projectCapabilities } from '../../../../app/capabilities'
import { createSessionController } from '../../../auth/model/session'
import { withAdmission } from '../../../auth/__tests__/fixtures/admission'
import { createWorldReplication } from '../../../../world/network/replication'
import { MockWorldClock, MockWorldTransport } from '../../../../world/network/mock'
import type { Snapshot, WorldRooms, WorldTicketResponse } from '../../../../world/network/WorldTransport'
import rooms from '../../../../../../contracts/fixtures/realtime/rooms.json'
import movement from '../../../../../../contracts/fixtures/realtime/movement-heartbeat.json'
import ticket from '../../../../../../contracts/fixtures/realtime/ticket-response.json'
import { createWorldAccessController, type WorldAccessRelease } from '../../model/controller'
export const initial = movement.messages[1]!.message as Snapshot
export const uid = (n: number) => `00000000-0000-4000-8000-${String(n).padStart(12, '0')}`
export const catalog = (availability: 'AVAILABLE' | 'FULL' | 'UNAVAILABLE' = 'AVAILABLE'): WorldRooms => ({ ...rooms, items: rooms.items.map((r, i) => ({ ...r, availability: i ? availability : 'AVAILABLE' })) })
export const errorResponse = (status: number, reason: string) => Response.json({ code: 'FIXTURE', message: 'PRIVATE RAW ERROR MUST NOT RENDER', details: { reason } }, { status })
export const flush = async () => { for (let i = 0; i < 20; i++) await Promise.resolve(); await new Promise(r => setTimeout(r, 1)) }
export function harness(fetcher?: (r: Request) => Promise<Response> | Response, qualifiedRefresh = false, beforeTicket?: (roomId: string) => Promise<void>) {
  const requests: Request[] = [], worldListeners = new Set<() => void>(), clock = new MockWorldClock(Date.parse(ticket.server_time))
  const auth = createSessionController({ eventId: initial.eventId, qualifiedRefresh, cookiePolicy: { available: true, onInvalidate: () => () => {}, run: (_kind, work) => work(), dispose() {} },
    fetch: withAdmission(async input => { const r = input as Request; requests.push(r); return fetcher ? fetcher(r) : Response.json(catalog()) }) })
  auth.session.replace(0, { userId: initial.selfUserId, eventId: initial.eventId, accessToken: 'fixture-token-memory-only' })
  const resources = { eventId: initial.eventId, roomId: initial.roomId, mapId: initial.mapId, mapRevision: initial.mapRevision, avatarManifestId: initial.avatarManifestId, avatarManifestVersion: initial.avatarManifestVersion }
  let keys = 110, tickets = 0
  const transport = new MockWorldTransport({ session: auth.session, origin: 'https://mcr.example', scheduler: clock, acquireTicket: async ({ body }) => { if (beforeTicket) await beforeTicket(body.room_id); return { ...ticket, ...body, ticket: String.fromCharCode(65 + tickets++).repeat(43) } as WorldTicketResponse } })
  const projected: string[][] = []
  const replication = createWorldReplication({ session: auth.session, transport, resources, scheduler: clock, requestId: () => uid(keys++), sink: { replace(view) { projected.push(view.entities.map(e => e.player.userId)) } } })
  let release: WorldAccessRelease | null = { scope: auth.privacy.capture(), replication, capabilities: projectCapabilities(Object.fromEntries(CAPABILITIES.map(k => [k, { available: true, reason: null }]))) }
  const controller = createWorldAccessController({ auth, world: { read: () => release, subscribe(fn) { worldListeners.add(fn); return () => { worldListeners.delete(fn) } } } })
  const receive = (data: unknown) => transport.sockets.at(-1)!.receive(JSON.stringify(data))
  const lastSwitch = () => transport.sockets.at(-1)!.sent.map(s => JSON.parse(s)).filter(s => s.type === 'switchRoom').at(-1) as { requestId: string; roomId: string; streamId: string }
  const waitForSwitch = (roomId: string) => {
    // Capture the request issued by this intent, not a previous sent frame.
    const requestId = uid(keys - 1), streamId = replication.read().baseline!.streamId
    return vi.waitFor(() => {
      const frames = transport.sockets.at(-1)!.sent.map(s => JSON.parse(s) as { type: string; requestId: string; roomId: string; streamId: string })
        .filter(frame => frame.type === 'switchRoom' && frame.requestId === requestId && frame.roomId === roomId && frame.streamId === streamId)
      expect(frames, `outbound switch ${requestId} to ${roomId}`).toHaveLength(1)
      return frames[0]!
    }, { timeout: 2000, interval: 1 })
  }
  return { auth, clock, requests, transport, replication, controller, projected, receive, lastSwitch, waitForSwitch,
    release: () => release,
    publish(value: WorldAccessRelease | null) { release = value; [...worldListeners].forEach(fn => fn()) },
    async live() {
      controller.start(); const p = replication.start()
      // Ticket validation includes asynchronous digest work. Wait for the
      // socket it authorizes, not an assumed single event-loop turn under load.
      const deadline = performance.now() + 2000
      while (!transport.sockets.length && performance.now() < deadline) await flush()
      if (!transport.sockets.length) throw new Error('Fixture ticket did not authorize a socket')
      transport.sockets.at(-1)!.open(); receive(initial); await p; await controller.loadRooms()
    },
    reject() { receive({ protocolVersion: '1.0.0', type: 'switchRejected', requestId: lastSwitch().requestId, streamId: replication.read().baseline!.streamId, reason: 'ROOM_FULL' }) },
    admit() { receive({ ...initial, cause: 'ROOM_SWITCH', requestId: lastSwitch().requestId, roomId: lastSwitch().roomId, streamId: uid(401), roomEpoch: uid(402), players: [initial.players[0]] }) },
    dispose() { controller.dispose(); replication.dispose(); auth.dispose() },
  }
}
