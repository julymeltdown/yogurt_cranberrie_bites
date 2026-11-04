import type { Geometry, MapManifest } from '../map/manifest'
import { digest } from '../map/json'
import type { MotionProfile, SelfMotion, WorldPosition } from './WorldTransport'

/** Internal point geometry, not a negotiated wire profile or a source of server tuning. */
export type MotionGeometry = Pick<Geometry, 'width_cells' | 'height_cells' | 'tile_width_px' | 'tile_height_px' | 'collision_grid'>
const finite = (value: number) => typeof value === 'number' && Number.isFinite(value)

/**
 * Point-supercover collision derived from the repository's BE12 WorldMap kernel.
 * No foot rectangle, wall sliding, speed default, server clock or command authority.
 * Public profile qualification must precede any application use of this primitive.
 */
export function createPointMotion(input: MotionGeometry) {
  const geometry = structuredClone(input)
  const { width_cells: width, height_cells: height, tile_width_px: tileWidth, tile_height_px: tileHeight, collision_grid: cells } = geometry
  if (![width, height, tileWidth, tileHeight].every(value => Number.isSafeInteger(value) && value >= 1 && value <= 128)
    || width * height > 16_384 || cells.length !== height
    || !cells.every(row => row.length === width && row.every(value => value === 0 || value === 1))) throw Error('Invalid prediction geometry')
  const cell = (x: number, y: number) => x >= 0 && y >= 0 && x < width && y < height && cells[y]![x] === 0
  function walkable(point: WorldPosition) {
    if (!finite(point.x) || !finite(point.y) || point.x < 0 || point.y < 0) return false
    const x = Math.floor(point.x / tileWidth), y = Math.floor(point.y / tileHeight)
    const edgeX = point.x % tileWidth === 0, edgeY = point.y % tileHeight === 0
    return cell(x, y) && (!edgeX || cell(x - 1, y)) && (!edgeY || cell(x, y - 1))
      && (!(edgeX && edgeY) || cell(x - 1, y - 1))
  }
  function clearSegment(from: WorldPosition, to: WorldPosition) {
    if (!walkable(from) || !walkable(to)) return false
    let x = Math.floor(from.x / tileWidth), y = Math.floor(from.y / tileHeight)
    const endX = Math.floor(to.x / tileWidth), endY = Math.floor(to.y / tileHeight)
    const dx = (to.x - from.x) / tileWidth, dy = (to.y - from.y) / tileHeight
    const sx = Math.sign(dx), sy = Math.sign(dy)
    const edgeX = dx === 0 && from.x % tileWidth === 0, edgeY = dy === 0 && from.y % tileHeight === 0
    const txDelta = dx === 0 ? Infinity : 1 / Math.abs(dx), tyDelta = dy === 0 ? Infinity : 1 / Math.abs(dy)
    let tx = sx === 0 ? Infinity : (((x + (sx > 0 ? 1 : 0)) * tileWidth - from.x) / tileWidth) / dx
    let ty = sy === 0 ? Infinity : (((y + (sy > 0 ? 1 : 0)) * tileHeight - from.y) / tileHeight) / dy
    for (let remaining = 512; remaining > 0; remaining--) {
      if (!cell(x, y) || (edgeX && !cell(x - 1, y)) || (edgeY && !cell(x, y - 1))) return false
      if (x === endX && y === endY) return true
      if (Math.abs(tx - ty) <= 1e-12) {
        if (!cell(x + sx, y) || !cell(x, y + sy)) return false
        x += sx; y += sy; tx += txDelta; ty += tyDelta
      } else if (tx < ty) { x += sx; tx += txDelta }
      else { y += sy; ty += tyDelta }
    }
    return false
  }
  /** Distance is explicit from the eventual negotiated profile, never an input sequence or ACK. */
  function step(from: WorldPosition, axes: { readonly axisX: number; readonly axisY: number }, distance: number): WorldPosition {
    const { axisX, axisY } = axes, length = Math.hypot(axisX, axisY)
    if (![axisX, axisY, distance].every(finite) || Math.abs(axisX) > 1 || Math.abs(axisY) > 1
      || length > 1 + 1e-9 || distance < 0 || distance > 1_048_576) throw Error('Invalid prediction step')
    const divisor = Math.max(1, length)
    const target = { x: from.x + axisX / divisor * distance, y: from.y + axisY / divisor * distance }
    return clearSegment(from, target) ? target : { x: from.x, y: from.y }
  }
  return { walkable, clearSegment, step }
}

/** Source is the already qualified C06 release, never a rendering rectangle. */
export interface MotionContext {
  readonly version: '1.0.0'
  readonly map: Pick<MapManifest, 'map_id' | 'map_revision' | 'geometry_hash' | 'geometry'>
}
export interface MotionInput {
  readonly seq: number
  readonly axisX: number
  readonly axisY: number
  readonly sentAt: number
}
export async function qualifyMotionContext(context: MotionContext) {
  const map = structuredClone(context.map)
  if (context.version !== '1.0.0' || await digest(map.geometry) !== map.geometry_hash) throw Error('Unqualified motion geometry')
  return { mapId: map.map_id, mapRevision: map.map_revision, geometryHash: map.geometry_hash, point: createPointMotion(map.geometry) }
}
export type QualifiedMotionContext = Awaited<ReturnType<typeof qualifyMotionContext>>

/**
 * Tentative presentation from the latest authoritative anchor, at most four
 * public fixed steps. Receipt time is not server arrival time. In particular,
 * neither ACK differences nor inputAppliedTicks imply a duration to replay.
 */
export function createMotionPrediction(context: QualifiedMotionContext, profile: MotionProfile) {
  if (profile.version !== '1.0.0' || profile.geometryHash !== context.geometryHash || profile.stepMs !== 50
    || profile.maxCatchUpSteps !== 4 || profile.axes !== 'UNIT_DISK_ANALOG' || profile.collision !== 'POINT_SUPERCOVER_STOP'
    || profile.catchUp !== 'DISCARD_OLDEST_DUE_STEPS' || profile.holdBoundary !== 'AGE_LE_HOLD_MS'
    || !finite(profile.speedPxPerSecond) || profile.speedPxPerSecond <= 0 || profile.speedPxPerSecond > 1000
    || !Number.isSafeInteger(profile.holdMs) || profile.holdMs < 50 || profile.holdMs > 1000) return null
  const model = structuredClone(profile)
  let anchor: { motion: SelfMotion; ack: number; at: number; position: WorldPosition; due: number; steps: number } | null = null
  const integer = (value: number) => Number.isSafeInteger(value) && value >= 0
  return {
    correct(motion: SelfMotion, tick: number, ack: number, sentSequence: number, at: number) {
      const previous = anchor, held = motion.heldInput
      if (![tick, ack, sentSequence, motion.serverTick, motion.consumedInputSeq, motion.inputAppliedTicks, motion.droppedTicks].every(integer)
        || !finite(at) || motion.serverTick !== tick || motion.droppedTicks > tick || ack > motion.consumedInputSeq
        || motion.consumedInputSeq > sentSequence || motion.inputAppliedTicks > tick - motion.droppedTicks || !context.point.walkable(motion.position)
        || (ack === 0 && (held !== null || motion.inputAppliedTicks !== 0))
        || (held !== null && (held.seq !== ack || !finite(held.axisX) || !finite(held.axisY)
          || Math.abs(held.axisX) > 1 || Math.abs(held.axisY) > 1 || Math.hypot(held.axisX, held.axisY) > 1 + 1e-9
          || !integer(held.remainingHoldMs) || held.remainingHoldMs > model.holdMs))
        || (previous && (tick < previous.motion.serverTick || at < previous.at || ack < previous.ack
          || motion.droppedTicks < previous.motion.droppedTicks || motion.consumedInputSeq < previous.motion.consumedInputSeq
          || motion.droppedTicks - previous.motion.droppedTicks > tick - previous.motion.serverTick
          || (ack === previous.ack && (motion.inputAppliedTicks < previous.motion.inputAppliedTicks
            || motion.inputAppliedTicks - previous.motion.inputAppliedTicks > tick - previous.motion.serverTick - (motion.droppedTicks - previous.motion.droppedTicks)))))) {
        anchor = null
        return false
      }
      anchor = { motion: structuredClone(motion), ack, at, position: { ...motion.position }, due: 0, steps: 0 }
      return true
    },
    sample(now: number, pending: readonly MotionInput[]): WorldPosition | null {
      if (!anchor || !finite(now) || now < anchor.at || pending.length > 64) return null
      const due = Math.floor((now - anchor.at) / model.stepMs)
      if (!Number.isSafeInteger(due) || due < anchor.due) return { ...anchor.position }
      // One total four-step budget per authoritative anchor. Discard old due
      // boundaries after a stall; repeated sampling cannot renew that budget or
      // slide an old window backwards. Hold the last presentation until correction.
      const available = model.maxCatchUpSteps - anchor.steps
      const first = Math.max(anchor.due + 1, due - available + 1)
      for (let step = first; step <= due; step++) {
        const at = anchor.at + step * model.stepMs
        let selected: MotionInput | undefined
        for (const input of pending) if (input.seq > anchor.motion.consumedInputSeq && input.sentAt <= at
          && (!selected || input.seq > selected.seq)) selected = input
        const held = anchor.motion.heldInput
        const axes = selected ? (at - selected.sentAt <= model.holdMs ? selected : null)
          : held && at - anchor.at <= held.remainingHoldMs ? held : null
        if (axes) anchor.position = context.point.step(anchor.position, axes, model.speedPxPerSecond * model.stepMs / 1000)
        anchor.steps++
      }
      anchor.due = due
      return { ...anchor.position }
    },
  }
}
