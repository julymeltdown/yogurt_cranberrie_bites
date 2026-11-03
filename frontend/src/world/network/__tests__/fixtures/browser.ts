import { createMemorySession } from '../../../../shared/api/http'
import { WebSocketWorldTransport } from '../../websocket'
import ticket from '../../../../../../contracts/fixtures/realtime/ticket-response.json'
import movement from '../../../../../../contracts/fixtures/realtime/movement-heartbeat.json'
import type { WorldEvent, WorldTicketResponse } from '../../WorldTransport'
const auth = movement.messages[0]!.message
const resources = { eventId: auth.eventId!, roomId: auth.roomId!, mapId: auth.mapId!, mapRevision: auth.mapRevision!, avatarManifestId: auth.avatarManifestId!, avatarManifestVersion: auth.avatarManifestVersion! }
const session = createMemorySession(); session.replace(0,{userId:movement.actor_id,eventId:resources.eventId,accessToken:'FE16-memory-only-token'})
let ticketCount = 0
const events: WorldEvent[] = [], urls: string[] = []
const transport = new WebSocketWorldTransport({ session, origin: location.origin, acquireTicket: async ({body}) => {
  ticketCount++
  const now = Date.now(), server_time = new Date(now).toISOString(), expires_at = new Date(now+30000).toISOString()
  return {...ticket,...body,server_time,expires_at,ticket: String.fromCharCode(64+ticketCount).repeat(43)} as WorldTicketResponse
} }, url => { urls.push(url); return new WebSocket(url) })
transport.subscribe(event => { events.push(event); if(event.type==='frame' && event.frame.type==='ping') transport.clientPong(event.frame.heartbeatId) })
const fixture = { transport, events, urls, session, connect: () => transport.connect(resources), count: () => ticketCount }
Object.assign(window,{__FE16_TEST_ONLY__: fixture})
