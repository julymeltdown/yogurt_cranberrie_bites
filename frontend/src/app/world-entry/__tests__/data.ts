// TEST ONLY: shapes simulating reviewed resources, never production artwork or a release registry.
import original from '../../../../../contracts/fixtures/core/bootstrap-open.json'
import originalRooms from '../../../../../contracts/fixtures/realtime/rooms.json'
import movement from '../../../../../contracts/fixtures/realtime/movement-heartbeat.json'
import { createSessionController } from '../../../features/auth/model/session'
import { decodeBootstrap } from '../../../features/auth/model/bootstrap'
import { contentHash, digest, sha256 } from '../../../world/map/json'
import { immutable } from '../../../world/entities/replicated'
import type { Snapshot } from '../../../world/network/WorldTransport'
import type { ReplicationView } from '../../../world/network/replication'
import { characterSourcePack } from '../../../entities/avatar/source-catalog'
import { isolatedCookiePolicy, capabilities, authResponse } from '../../../features/auth/__tests__/fixtures/data'
import type { LocalInputConfiguration } from '../resolver'
import type { WorldEntryRequest } from '../authority'
export const eventId = original.event.event_id, userId = original.participant.user_id
export const inputConfiguration: LocalInputConfiguration = { revision: 'test-only-policy-v1', reviewedBy: 'test-only reviewer',
  movementPolicy: { speedPxPerSecond: 120, footWidthPx: 8, footHeightPx: 6, interactionRadiusPx: 42 }, zoom: 2 }
export const assetBytes = { json: new TextEncoder().encode('{"test_only":"not a product map"}'), png: new TextEncoder().encode('TEST ONLY HASH READER BYTES; no character art') }
export type Bundle = typeof original
export async function seal(b: Bundle) {
  b.avatar_manifest.content_hash = await contentHash(b.avatar_manifest)
  b.map_manifest.geometry_hash = await digest(b.map_manifest.geometry)
  b.map_manifest.content_hash = await contentHash(b.map_manifest)
  for (const theme of b.theme_manifests) {
    Object.assign(theme, { map_id: b.map_manifest.map_id, map_revision: b.map_manifest.map_revision,
      map_content_hash: b.map_manifest.content_hash, geometry_hash: b.map_manifest.geometry_hash,
      avatar_manifest_id: b.avatar_manifest.manifest_id, avatar_manifest_version: b.avatar_manifest.manifest_version,
      avatar_content_hash: b.avatar_manifest.content_hash })
    theme.content_hash = await contentHash(theme)
  }
  return b
}
export async function bundle() {
  const b = structuredClone(original)
  b.capabilities.world = { available: true, reason: null } as unknown as typeof b.capabilities.world
  b.capabilities.meet = { available: true, reason: null } as unknown as typeof b.capabilities.meet
  b.participant.preferences.theme = 'PARADE'
  for (const [i, manifest] of [b.avatar_manifest, b.map_manifest, ...b.theme_manifests].entries()) Object.assign(manifest.provenance, {
    source_kind: 'ORIGINAL_ART', source_id: i === 0 ? characterSourcePack.source_pack_id : 'test_only_release',
    source_revision: 'test-only', review_state: 'REVIEWED', reviewed_by: 'test-only reviewer', reviewed_at: '2026-11-03T00:00:00Z',
  })
  for (const place of b.map_manifest.places) Object.assign(place.provenance, { source_kind: 'ORIGINAL_ART', review_state: 'REVIEWED', reviewed_by: 'test-only reviewer', reviewed_at: '2026-11-03T00:00:00Z' })
  Object.assign(b.map_manifest.tilemap, { resource_state: 'PROVIDED', path: 'world/test-only/map.json', sha256: await sha256(assetBytes.json) })
  Object.assign(b.avatar_manifest.atlas.texture, { image_state: 'PROVIDED', path: 'world/test-only/avatar.png', sha256: await sha256(assetBytes.png) })
  for (const theme of b.theme_manifests) Object.assign(theme.texture, { image_state: 'PROVIDED', path: `world/test-only/${theme.theme_id.toLowerCase()}.png`, sha256: await sha256(assetBytes.png) })
  return seal(b)
}
export const rooms = () => immutable({ ...structuredClone(originalRooms), event_id: eventId, items: originalRooms.items.map(r => ({ ...r, availability: 'AVAILABLE' as const })) })
export function authHarness(handler?: (request: Request) => Promise<Response> | Response) {
  const requests: Request[] = []
  const auth = createSessionController({ eventId, cookiePolicy: isolatedCookiePolicy(), fetch: async input => {
    const request = input as Request; requests.push(request)
    if (handler) return handler(request)
    const path = new URL(request.url).pathname
    if (path === '/api/auth/capabilities') return Response.json(capabilities())
    if (path === '/api/auth/login') return Response.json(authResponse())
    if (path === '/api/auth/logout') return new Response(null, { status: 204 })
    if (path.endsWith('/rooms')) return Response.json(rooms())
    throw new Error('Unexpected test request')
  } })
  auth.session.replace(0, { userId, eventId, accessToken: 'memory-only-test-token' })
  return { auth, requests }
}
export async function requestFor(auth: ReturnType<typeof authHarness>['auth'], b = bundle()): Promise<WorldEntryRequest> {
  const scope = { generation: auth.session.read().generation, userId, eventId }
  return { scope, privacyScope: auth.privacy.capture(), bootstrap: await decodeBootstrap(await b, scope), signal: new AbortController().signal }
}
export function replicationPort(request: WorldEntryRequest) {
  const frame = movement.messages[1]!.message as Snapshot, { players: _players, ...baseline } = frame
  const self = { ...frame.players[0]!, userId, displayName: 'Current fan' }, other = { ...frame.players[1]!, displayName: 'Authorized visitor' }
  let value: ReplicationView | null = immutable({ scope: request.scope, visible: true, phase: 'live',
    baseline: { ...baseline, eventId, selfUserId: userId }, entities: [self, other].map(player => ({ player, self: player.userId === userId, position: player.position })),
    pendingInputs: 0, ackInputSeq: 0, spatialPrediction: 'unavailable', problem: null })
  const listeners = new Set<() => void>(), sent: string[] = []
  return { read: () => value, subscribe(fn: () => void) { listeners.add(fn); return () => { listeners.delete(fn) } },
    async switchRoom(id: string) { sent.push(id); return true },
    publish(next: ReplicationView | null) { value = next; [...listeners].forEach(fn => fn()) }, sent, listeners }
}
