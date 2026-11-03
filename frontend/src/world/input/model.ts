import timing from '../../../../contracts/realtime/v1/policy.json'
import type { Input } from '../network/transport/wire-types'
import type { LoadedMap } from '../map/resources'
import type { Point } from '../camera/port'
import { nearbyTargets, type NearbyTarget } from '../interaction/selection'
import { createCollision, normalized, type SimulationPolicy } from './simulation'
export type MovementIntent = Pick<Input, 'axisX' | 'axisY' | 'buttons'>
export const STEP_MS = 1000 / timing.simulation_hz_target
export interface MovementSnapshot {
  readonly foot: Point
  readonly axis: Point
  readonly path: readonly Point[]
  readonly nearby: readonly NearbyTarget[]
  readonly selectedId: string | null
}
/** Predicts only; has no stream, epoch, command sequence, membership, or authoritative position. */
export function createMovementModel(map: LoadedMap, policy: SimulationPolicy) {
  if (timing.protocol_version !== '1.0.0' || timing.state !== 'FROZEN_V1_0' || !Number.isInteger(timing.simulation_hz_target)
    || timing.simulation_hz_target < 10 || timing.simulation_hz_target > timing.input_messages_per_second || timing.input_messages_per_second > 30) throw new Error('Unsupported input policy')
  policy = Object.freeze({ ...policy })
  const g = map.map.geometry, collision = createCollision(g, policy)
  let foot: Point = collision.center(map.spawn.cell_x, map.spawn.cell_y), axis: Point = { x: 0, y: 0 }, path: Point[] = [], pending: string | null = null
  let nearby = nearbyTargets(map, foot, collision, policy.interactionRadiusPx), selectedId: string | null = nearby[0]?.interactionId ?? null
  if (!collision.walkable(foot)) throw new Error('Local foot does not fit the approved spawn')
  const refresh = () => {
    nearby = nearbyTargets(map, foot, collision, policy.interactionRadiusPx)
    if (!nearby.some(target => target.interactionId === selectedId)) { selectedId = nearby[0]?.interactionId ?? null; pending = null }
  }
  return {
    collision,
    read(): MovementSnapshot { return Object.freeze({ foot: Object.freeze({ ...foot }), axis: Object.freeze({ ...axis }), path: Object.freeze(path.map(p => Object.freeze({ ...p }))),
      nearby: Object.freeze(nearby.map(target => Object.freeze({ ...target }))), selectedId }) },
    axis(x: number, y: number) { axis = normalized(x, y); path = [] },
    tap(point: Point) { axis = { x: 0, y: 0 }; pending = null; path = collision.route(foot, point) ?? []; return path.length > 0 },
    select(id: string) { if (nearby.some(target => target.interactionId === id)) { selectedId = id; pending = null; return true } return false },
    interact() { if (selectedId && nearby.some(target => target.interactionId === selectedId)) { pending = selectedId; return true } return false },
    reset() { axis = { x: 0, y: 0 }; path = []; pending = null },
    active: () => axis.x !== 0 || axis.y !== 0 || path.length > 0 || pending !== null,
    step(): MovementIntent {
      let input = axis
      while (path.length && Math.hypot(path[0]!.x - foot.x, path[0]!.y - foot.y) < 1e-6) path.shift()
      if (path.length) {
        const dx = path[0]!.x - foot.x, dy = path[0]!.y - foot.y, distance = Math.hypot(dx, dy)
        const scale = Math.max(distance, policy.speedPxPerSecond * STEP_MS / 1000)
        input = { x: dx / scale, y: dy / scale }
      }
      const next = collision.move(foot, input, STEP_MS / 1000)
      if (path.length && Math.hypot(next.x - foot.x, next.y - foot.y) < 1e-8) { path = []; input = { x: 0, y: 0 } }
      foot = next; refresh()
      const buttons: MovementIntent['buttons'] = pending && pending === selectedId ? ['interact'] : []
      pending = null
      return Object.freeze({ axisX: input.x, axisY: input.y, buttons: Object.freeze(buttons) })
    },
  }
}
export type MovementModel = ReturnType<typeof createMovementModel>
