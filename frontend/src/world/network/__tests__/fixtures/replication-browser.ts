import { createMemorySession } from '../../../../shared/api/http'
import { WebSocketWorldTransport } from '../../websocket'
import { createWorldReplication } from '../../replication'
import type { Scheduler, WorldTicketResponse } from '../../WorldTransport'
import ticket from '../../../../../../contracts/fixtures/realtime/ticket-response.json'
import movement from '../../../../../../contracts/fixtures/realtime/movement-heartbeat.json'
import map from '../../../../../../contracts/fixtures/assets/map-manifest.json'
const auth = movement.messages[0]!.message
const resources = { eventId: auth.eventId!, roomId: auth.roomId!, mapId: auth.mapId!, mapRevision: auth.mapRevision!, avatarManifestId: auth.avatarManifestId!, avatarManifestVersion: auth.avatarManifestVersion! }
const session = createMemorySession(); session.replace(0, { userId: movement.actor_id, eventId: resources.eventId, accessToken: 'synthetic-memory-only-token' })
const timers = new Set<ReturnType<typeof setTimeout>>()
const scheduler: Scheduler = { now: () => Date.now(), set(ms, fn) { const h = setTimeout(() => { timers.delete(h); fn() }, ms); timers.add(h); return h }, clear(h) { clearTimeout(h as ReturnType<typeof setTimeout>); timers.delete(h as ReturnType<typeof setTimeout>) } }
let tickets = 0, commits = 0, disposed = false
const transport = new WebSocketWorldTransport({ session, origin: location.origin, scheduler, motionProfileVersion: '1.0.0', acquireTicket: async ({ body }) => {
  const now = Date.now(); tickets++
  return { ...ticket, ...body, ticket: String.fromCharCode(64 + tickets).repeat(43), server_time: new Date(now).toISOString(), expires_at: new Date(now + 30000).toISOString() } as WorldTicketResponse
} })
const replication = createWorldReplication({ session, transport, resources, scheduler, motion: { version: '1.0.0', map }, sink: { replace(view) {
  commits++; document.getElementById('projection')!.textContent = JSON.stringify({ phase: view.phase, names: view.entities.map(e => e.player.displayName), prediction: view.spatialPrediction })
} } })
const visibility = () => replication.setVisible(!document.hidden)
document.addEventListener('visibilitychange', visibility)
const fixture = {
  view: replication.read, sample: replication.sample, intent: replication.setIntent,
  visible: replication.setVisible,
  counts: () => ({ tickets, timers: timers.size, commits, disposed }),
  replaceOwner() { session.replace(session.read().generation, null) },
  dispose() { if (disposed) return; disposed = true; document.removeEventListener('visibilitychange', visibility); replication.dispose() },
}
Object.assign(window, { __FE17_TEST_ONLY__: fixture })
void replication.start()
