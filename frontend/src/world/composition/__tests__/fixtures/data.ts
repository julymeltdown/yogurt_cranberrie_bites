// TEST ONLY: synthetic C06 terrain and metadata, never an approved product asset release.
import original from '../../../../../../contracts/fixtures/core/bootstrap-open.json'
import roomFixture from '../../../../../../contracts/fixtures/realtime/rooms.json'
import movement from '../../../../../../contracts/fixtures/realtime/movement-heartbeat.json'
import { movementData } from '../../../__tests__/fixtures/movement-data'
import { contentHash } from '../../../map/json'
import { createAssetResources } from '../../../map/resources'
import { decodeBootstrap } from '../../../../features/auth/model/bootstrap'
import type { SessionController } from '../../../../features/auth/model/session'
import type { ActiveScope } from '../../../../app/ports'
import type { WorldRooms, Snapshot, WorldTicketRequest } from '../../../network/WorldTransport'
import { validate } from '../../../network/transport/contract'
import type { ResolvedWorldRelease } from '../../release'

export const eventId = original.event.event_id, userId = original.participant.user_id
export async function testData() {
  const f = await movementData()
  const themes = await Promise.all(original.theme_manifests.map(async theme => {
    const value = { ...structuredClone(f.theme), theme_id: theme.theme_id }
    value.content_hash = await contentHash(value)
    return value
  }))
  const bootstrap = { ...structuredClone(original), capabilities: { ...structuredClone(original.capabilities), world: { available: true, reason: null } },
    map_manifest: f.map, theme_manifests: themes }
  const rooms = validate<WorldRooms>('WorldRooms', { ...structuredClone(roomFixture), event_id: eventId })
  return { ...f, bootstrap, rooms }
}
export type TestData = Awaited<ReturnType<typeof testData>>
export function resolverFor(auth: SessionController, data: TestData, fetchAssets?: typeof fetch) {
  return async (): Promise<ResolvedWorldRelease> => ({ privacyScope: auth.privacy.capture(), bootstrap: data.bootstrap,
    themeId: 'REVENGE', roomId: data.rooms.default_room_id, rooms: data.rooms,
    resources: createAssetResources({ origin: location.origin, allow: data.allow, fetch: fetchAssets ?? (async (input, init) => {
      const url = new URL(String(input)); if (init?.signal?.aborted) throw new Error('Aborted test asset')
      const bytes = url.pathname.endsWith('.png') ? data.png : data.tiledBytes
      const response = new Response(bytes, { headers: { 'Content-Type': url.pathname.endsWith('.png') ? 'image/png' : 'application/json' } })
      Object.defineProperty(response, 'url', { value: url.href }); return response
    }) }), movementPolicy: { speedPxPerSecond: 96, footWidthPx: 12, footHeightPx: 8, interactionRadiusPx: 96 }, zoom: 1 })
}
export function currentScope(auth: SessionController): ActiveScope {
  const s = auth.session.read()
  if (!s.userId || !s.eventId) throw new Error('Fixture must be authenticated')
  return { userId: s.userId, eventId: s.eventId, generation: s.generation }
}
export const project = (auth: SessionController, data: TestData) => decodeBootstrap(data.bootstrap, currentScope(auth))
export function ticket(body: WorldTicketRequest, n: number) {
  const now = Date.now()
  return { ...body, protocol_version: '1.0.0', event_id: eventId, ticket: String.fromCharCode(65 + n).repeat(43),
    server_time: new Date(now).toISOString(), expires_at: new Date(now + 30000).toISOString(), ws_path: '/world-ws', auth_timeout_ms: 3000 }
}
export function snapshot(roomId: string, streamId = movement.messages[1]!.message.streamId!, patch: Partial<Snapshot> = {}): Snapshot {
  const template = movement.messages[1]!.message as unknown as Snapshot
  return { ...template, eventId, roomId, streamId, selfUserId: userId, accessExpiresAt: new Date(Date.now() + 60000).toISOString(),
    players: template.players.map((p, i) => ({ ...p, userId: i === 0 ? userId : p.userId,
      position: { x: 80 + i * 32, y: 112 } })), ...patch }
}
