// TEST ONLY: published spatial-v3/all-era or retained v4 source, no art-review relabeling.
import original from '../../../../../../contracts/fixtures/core/bootstrap-open.json'
import movement from '../../../../../../contracts/fixtures/realtime/movement-heartbeat.json'
import { referenceFixture } from '../../../../world/reference-scene/__tests__/fixture'
import { spatialFixture } from '../../../../world/reference-scene/__tests__/spatial-v3/fixture'
import { spatialThemeIds, type SpatialThemeId } from '../../../../world/reference-scene'
import { contentHash } from '../../../../world/map/json'
import { validate } from '../../../../world/network/transport/contract'
import { validateWholeCharacterConfig } from '../../../../entities/avatar/whole-character'
import type { Snapshot } from '../../../../world/network/WorldTransport'

export const eventId = original.event.event_id, userId = original.participant.user_id
export const localInput = { revision: 'TEST_ONLY_reference_source', reviewedBy: 'TEST ONLY local intent configuration, not C05 authority',
  movementPolicy: { speedPxPerSecond: 96, footWidthPx: 12, footHeightPx: 8, interactionRadiusPx: 96 }, zoom: 1 }
export async function sceneData(version: 'legacy' | 'spatial' = 'spatial', initial: SpatialThemeId = 'PARADE') {
  const f = await (version === 'spatial' ? spatialFixture(initial) : referenceFixture()), presets = f.avatar.presets.map(p => validateWholeCharacterConfig(p.config))
  const themes = version === 'spatial' ? await Promise.all(spatialThemeIds.map(async id => (await spatialFixture(id)).theme))
    : [f.theme, ...await Promise.all(original.theme_manifests.filter(t => t.theme_id !== 'PARADE').map(async base => {
    const theme = { ...structuredClone(base), map_id: f.map.map_id, map_revision: f.map.map_revision, map_content_hash: f.map.content_hash,
      geometry_hash: f.map.geometry_hash, collision_id: f.map.geometry.collision_id, place_ids: f.map.places.map(p => p.place_id),
      interaction_ids: f.map.geometry.interactions.map(i => i.interaction_id), avatar_manifest_id: f.avatar.manifest_id,
      avatar_manifest_version: f.avatar.manifest_version, avatar_content_hash: f.avatar.content_hash,
      provenance: structuredClone(f.theme.provenance),
      texture: { image_state: 'NOT_PROVIDED', path: null, sha256: null, width_px: 768, height_px: 1296 } }
    theme.content_hash = await contentHash(theme); return theme
  }))]
  const avatar = { revision: 7, config: presets[0]! }
  const bootstrap = { ...structuredClone(original), server_time: '2026-11-07T07:00:00Z', avatar,
    avatar_manifest: f.avatar, map_manifest: f.map, theme_manifests: themes,
    participant: { ...structuredClone(original.participant), profile: { ...structuredClone(original.participant.profile), display_name: 'Scene fixture fan', avatar },
      preferences: { ...structuredClone(original.participant.preferences), theme: f.theme.theme_id } } }
  bootstrap.capabilities.world = { available: true, reason: null } as unknown as typeof bootstrap.capabilities.world
  return { ...f, bootstrap, presets, version }
}
export type SceneData = Awaited<ReturnType<typeof sceneData>>
export function snapshot(data: SceneData, roomId: string): Snapshot {
  const { geometry: g } = data.map
  const cells: { x: number; y: number }[] = []
  const spawn = g.spawns.find(s => s.spawn_id === g.default_spawn_id)!
  for (let y = spawn.cell_y; y < spawn.cell_y + 6 && cells.length < 6; y++) for (let x = spawn.cell_x - 1; x < spawn.cell_x + 6 && cells.length < 6; x++) {
    if (!g.collision_grid[y]![x]) cells.push({ x: (x + .5) * 16, y: (y + .5) * 16 })
  }
  return validate<Snapshot>('Snapshot', { ...movement.messages[1]!.message, eventId, roomId,
    mapId: data.map.map_id, mapRevision: data.map.map_revision, avatarManifestId: data.avatar.manifest_id,
    avatarManifestVersion: data.avatar.manifest_version, accessExpiresAt: new Date(Date.now() + 60_000).toISOString(), selfUserId: userId,
    players: data.presets.map((config, i) => ({ userId: i === 0 ? userId : `00000000-0000-4000-8000-00000000090${i}`,
      displayName: i === 0 ? 'Scene fixture fan' : `Scene visitor ${i + 1}`, position: cells[i]!, facing: 'down', moving: false,
      avatar: { revision: i === 0 ? 7 : 1, config } })) })
}
