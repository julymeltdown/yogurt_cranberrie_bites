import type { WorldPlayer, WorldPosition } from '../network/WorldTransport'

/** Public C05 data only. position is local presentation; player.position remains authoritative. */
export interface ReplicatedEntity {
  readonly player: WorldPlayer
  readonly position: WorldPosition
  readonly self: boolean
}
export interface EntitySink<T> {
  /** Synchronous, atomic full replacement, including empty eviction. Never retain an old roster. */
  replace(view: T): void
}
export function immutable<T>(value: T): T {
  if (value && typeof value === 'object' && !Object.isFrozen(value)) {
    for (const child of Object.values(value)) immutable(child)
    Object.freeze(value)
  }
  return value
}
/** One current player and two bounded presentation positions per ID; no extrapolation/history queue. */
export function createEntityProjection(interpolationMs: number) {
  type Row = { player: WorldPlayer; from: WorldPosition; at: number }
  const rows = new Map<string, Row>()
  let selfId = ''
  function position(row: Row, now: number): WorldPosition {
    const target = row.player.position
    if (row.player.userId === selfId) return target
    const t = Math.min(1, Math.max(0, (now - row.at) / interpolationMs))
    return { x: row.from.x + (target.x - row.from.x) * t, y: row.from.y + (target.y - row.from.y) * t }
  }
  return {
    clear() { rows.clear(); selfId = '' },
    reset(players: readonly WorldPlayer[], self: string, now: number) {
      rows.clear(); selfId = self
      for (const player of players) {
        const owned = immutable(structuredClone(player))
        rows.set(player.userId, { player: owned, from: owned.position, at: now })
      }
    },
    delta(upserts: readonly WorldPlayer[], removes: readonly string[], now: number) {
      // FE16 has already validated the complete frame and roster invariants.
      for (const id of removes) rows.delete(id)
      for (const player of upserts) {
        const previous = rows.get(player.userId), owned = immutable(structuredClone(player))
        rows.set(player.userId, { player: owned, from: previous ? position(previous, now) : owned.position, at: now })
      }
    },
    render(now: number): readonly ReplicatedEntity[] {
      return immutable([...rows.values()].sort((a, b) => a.player.userId.localeCompare(b.player.userId))
        .map(row => ({ player: row.player, position: position(row, now), self: row.player.userId === selfId })))
    },
  }
}
