import type { AvatarState } from '../../world/network/WorldTransport'
import type { ReplicationView } from '../../world/network/replication'
import type { SessionScope } from '../../shared/api/http'
import { immutable } from '../../world/entities/replicated'
export interface NearbyPerson { readonly userId: string; readonly displayName: string; readonly self: boolean; readonly moving: boolean; readonly avatar?: AvatarState }
/** The authorized replication roster is the only source. Do not derive physical distance or history. */
export function projectNearby(view: ReplicationView, scope: SessionScope): readonly NearbyPerson[] {
  const b = view.baseline
  if (!view.visible || !['live', 'switching'].includes(view.phase) || !b || b.eventId !== scope.eventId || b.selfUserId !== scope.userId
    || view.scope.generation !== scope.generation || view.scope.userId !== scope.userId || view.scope.eventId !== scope.eventId) return []
  const ids = new Set<string>()
  const people: NearbyPerson[] = []
  for (const entity of view.entities) {
    if (ids.has(entity.player.userId) || entity.self !== (entity.player.userId === scope.userId)) return []
    ids.add(entity.player.userId)
    people.push({ userId: entity.player.userId, displayName: entity.player.displayName, self: entity.self, moving: entity.player.moving,
      ...(entity.player.avatar ? { avatar: structuredClone(entity.player.avatar) } : {}) })
  }
  if (!people.some(p => p.self)) return []
  return immutable(people.sort((a, b) => Number(b.self) - Number(a.self) || a.displayName.localeCompare(b.displayName) || a.userId.localeCompare(b.userId)))
}
