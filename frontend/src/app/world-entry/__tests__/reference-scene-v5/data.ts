// TEST ONLY: published v10 source on synthetic map7005; no backend revision or art-review relabeling.
import original from '../../../../../../contracts/fixtures/core/bootstrap-open.json'
import movement from '../../../../../../contracts/fixtures/realtime/movement-heartbeat.json'
import { spatialV5Fixture } from '../../../../world/reference-scene/__tests__/spatial-v5/fixture'
import { spatialV5ThemeIds, type SpatialV5ThemeId } from '../../../../world/reference-scene'
import { validate } from '../../../../world/network/transport/contract'
import { validateWholeCharacterConfig } from '../../../../entities/avatar/whole-character'
import type { Snapshot } from '../../../../world/network/WorldTransport'

export const eventId = original.event.event_id, userId = original.participant.user_id
export const localInput = { revision: 'TEST_ONLY_reference_source', reviewedBy: 'TEST ONLY local intent configuration, not C05 authority',
  movementPolicy: { speedPxPerSecond: 96, footWidthPx: 12, footHeightPx: 8, interactionRadiusPx: 96 }, zoom: 1 }
export async function sceneData(initial: SpatialV5ThemeId = 'PARADE') {
  const f = await spatialV5Fixture(initial), presets = f.avatar.presets.map(p => validateWholeCharacterConfig(p.config))
  const themes = await Promise.all(spatialV5ThemeIds.map(async id => (await spatialV5Fixture(id)).theme))
  const originalThemeProvenance = themes.map(t => structuredClone(t.provenance))
  const suppliedThemes = themes
  const avatar = { revision: 7, config: presets[0]! }
  const bootstrap = { ...structuredClone(original), server_time: '2026-11-07T07:00:00Z', avatar,
    avatar_manifest: f.avatar, map_manifest: f.map, theme_manifests: suppliedThemes,
    participant: { ...structuredClone(original.participant), profile: { ...structuredClone(original.participant.profile), display_name: 'Scene fixture fan', avatar },
      preferences: { ...structuredClone(original.participant.preferences), theme: f.theme.theme_id } } }
  bootstrap.capabilities.world = { available: true, reason: null } as unknown as typeof bootstrap.capabilities.world
  return { ...f, bootstrap, presets, version: 'spatial-v5' as const, derived: false, originalThemeProvenance }
}
export type SceneData = Awaited<ReturnType<typeof sceneData>>
export function snapshot(data: SceneData, roomId: string): Snapshot {
  const { geometry: g } = data.map
  const cells: { x: number; y: number }[] = []
  const spawn = g.spawns.find(s => s.spawn_id === g.default_spawn_id)!
  // TEST_ONLY: keep all selected source characters separately legible; no C05/server placement claim.
  for (const [dx, dy] of [[0, 1], [-5, -3], [5, -3], [0, -3], [-5, 1], [5, 1]]) {
    const x = spawn.cell_x + dx!, y = spawn.cell_y + dy!
    if (g.collision_grid[y]![x]) throw Error('Synthetic roster feet must remain canonical clear cells')
    cells.push({ x: (x + .5) * 16, y: (y + .5) * 16 })
  }
  return validate<Snapshot>('Snapshot', { ...movement.messages[1]!.message, eventId, roomId,
    mapId: data.map.map_id, mapRevision: data.map.map_revision, avatarManifestId: data.avatar.manifest_id,
    avatarManifestVersion: data.avatar.manifest_version, accessExpiresAt: new Date(Date.now() + 60_000).toISOString(), selfUserId: userId,
    players: data.presets.map((config, i) => ({ userId: i === 0 ? userId : `00000000-0000-4000-8000-00000000090${i}`,
      displayName: i === 0 ? 'Scene fixture fan' : `Scene visitor ${i + 1}`, position: cells[i]!, facing: 'down', moving: false,
      avatar: { revision: i === 0 ? 7 : 1, config } })) })
}
