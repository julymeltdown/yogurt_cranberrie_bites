// TEST ONLY authority around exact public map4/selected-static source descriptors.
// No server credentials, producer acceptance or production admission is asserted.
import operator from '../../src/app/world-entry/__tests__/map-presentation-v1/source-inputs/operator-bootstrap.json'
import input from '../../src/app/world-entry/__tests__/map-presentation-v1/source-inputs/local-input.json'
import movement from '../../../contracts/fixtures/realtime/movement-heartbeat.json'
import originalRooms from '../../../contracts/fixtures/realtime/rooms.json'
import { validate } from '../../src/world/network/transport/contract'
import { validateWholeCharacterConfig } from '../../src/entities/avatar/whole-character'
import type { Snapshot } from '../../src/world/network/WorldTransport'

export const eventId = operator.event.event_id, userId = operator.participant.user_id
export const localInput = structuredClone(input)
export const bootstrap = structuredClone(operator)
// Only the synthetic server's explicit world decision changes; all immutable art/map DTOs stay exact.
bootstrap.capabilities.world = { available: true, reason: null } as unknown as typeof bootstrap.capabilities.world
bootstrap.participant.preferences.theme = 'PARADE'
export const avatar = bootstrap.avatar_manifest
export const presets = avatar.presets.map(p => validateWholeCharacterConfig(p.config))
export function rooms() {
  return { ...structuredClone(originalRooms), event_id: eventId,
    items: originalRooms.items.map(r => ({ ...r, availability: 'AVAILABLE' as const })) }
}
export function snapshot(roomId: string): Snapshot {
  const map = bootstrap.map_manifest, g = map.geometry
  const spawn = g.spawns.find(s => s.spawn_id === g.default_spawn_id)!
  const cells: { x: number; y: number }[] = []
  for (const [dx, dy] of [[0, 1], [-5, -3], [5, -3], [0, -3], [-5, 1], [5, 1]]) {
    const x = spawn.cell_x + dx!, y = spawn.cell_y + dy!
    if (g.collision_grid[y]![x]) throw Error('Fixture selected-character placement must use clear source cells')
    cells.push({ x: (x + .5) * 16, y: (y + .5) * 16 })
  }
  return validate<Snapshot>('Snapshot', { ...movement.messages[1]!.message, eventId, roomId, streamId: crypto.randomUUID(),
    mapId: map.map_id, mapRevision: map.map_revision, avatarManifestId: avatar.manifest_id, avatarManifestVersion: avatar.manifest_version,
    selfUserId: userId, accessExpiresAt: new Date(Date.now() + 60_000).toISOString(),
    players: presets.map((config, i) => ({ userId: i === 0 ? userId : `00000000-0000-4000-8000-00000000090${i}`,
      displayName: i === 0 ? 'QA08 synthetic participant' : `QA08 synthetic peer ${i}`, position: cells[i]!, facing: 'down', moving: false,
      avatar: { revision: i === 0 ? bootstrap.avatar.revision : 1, config } })) })
}
