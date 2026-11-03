import type { Geometry } from '../map/manifest'
import type { Point } from '../camera/port'
export interface SimulationPolicy { readonly speedPxPerSecond: number; readonly footWidthPx: number; readonly footHeightPx: number; readonly interactionRadiusPx: number }
export const finite = (n: number) => typeof n === 'number' && Number.isFinite(n)
export function validatePolicy(g: Geometry, p: SimulationPolicy) {
  if (![p.speedPxPerSecond, p.footWidthPx, p.footHeightPx, p.interactionRadiusPx].every(finite)
    || p.speedPxPerSecond <= 0 || p.speedPxPerSecond > 1024 || p.footWidthPx < 1 || p.footWidthPx > g.tile_width_px
    || p.footHeightPx < 1 || p.footHeightPx > g.tile_height_px || p.interactionRadiusPx <= 0 || p.interactionRadiusPx > 512) throw new Error('Invalid local movement policy')
}
export function normalized(x: number, y: number): Point {
  if (!finite(x) || !finite(y)) return { x: 0, y: 0 }
  const length = Math.hypot(x, y), divisor = Math.max(1, length)
  return { x: x / divisor, y: y / divisor }
}
/** Foot is bottom-center of a deliberately injected local collision rectangle; not an avatar sprite. */
export function createCollision(g: Geometry, policy: SimulationPolicy) {
  if (![g.width_cells, g.height_cells, g.tile_width_px, g.tile_height_px].every(n => Number.isSafeInteger(n) && n >= 1 && n <= 128)
    || g.collision_grid.length !== g.height_cells || !g.collision_grid.every(row => row.length === g.width_cells && row.every(cell => cell === 0 || cell === 1))) throw new Error('Invalid collision geometry')
  validatePolicy(g, policy)
  policy = Object.freeze({ ...policy })
  const width = g.width_cells * g.tile_width_px, height = g.height_cells * g.tile_height_px, epsilon = 1e-7
  function walkable(point: Point) {
    if (!finite(point.x) || !finite(point.y)) return false
    const left = point.x - policy.footWidthPx / 2, right = point.x + policy.footWidthPx / 2, top = point.y - policy.footHeightPx, bottom = point.y
    if (left < 0 || right > width || top < 0 || bottom > height) return false
    for (let y = Math.floor(top / g.tile_height_px); y <= Math.floor((bottom - epsilon) / g.tile_height_px); y++) {
      for (let x = Math.floor(left / g.tile_width_px); x <= Math.floor((right - epsilon) / g.tile_width_px); x++) if (g.collision_grid[y]?.[x] !== 0) return false
    }
    return true
  }
  const stride = Math.min(g.tile_width_px, g.tile_height_px, policy.footWidthPx, policy.footHeightPx) / 2
  function move(foot: Point, input: Point, seconds: number): Point {
    if (!walkable(foot) || !finite(seconds) || seconds <= 0) return foot
    const axis = normalized(input.x, input.y), distance = policy.speedPxPerSecond * Math.min(seconds, .1)
    const dx = axis.x * distance, dy = axis.y * distance, count = Math.max(1, Math.ceil(Math.max(Math.abs(dx), Math.abs(dy)) / stride))
    let { x, y } = foot
    for (let i = 0; i < count; i++) {
      // Axis separation allows wall sliding, while every small sweep keeps the complete foot clear.
      if (walkable({ x: x + dx / count, y })) x += dx / count
      if (walkable({ x, y: y + dy / count })) y += dy / count
    }
    return { x, y }
  }
  function line(a: Point, b: Point) {
    if (!walkable(a) || !walkable(b)) return false
    const count = Math.max(1, Math.ceil(Math.hypot(b.x - a.x, b.y - a.y) / stride))
    for (let i = 1; i <= count; i++) if (!walkable({ x: a.x + (b.x - a.x) * i / count, y: a.y + (b.y - a.y) * i / count })) return false
    return true
  }
  const center = (x: number, y: number): Point => ({ x: (x + .5) * g.tile_width_px, y: (y + .5) * g.tile_height_px })
  /** Bounded four-neighbor BFS. No diagonal corner cutting; every edge checks the whole foot sweep. */
  function route(from: Point, to: Point): Point[] | null {
    if (!walkable(from) || !walkable(to)) return null
    const sx = Math.floor(from.x / g.tile_width_px), sy = Math.floor(from.y / g.tile_height_px), tx = Math.floor(to.x / g.tile_width_px), ty = Math.floor(to.y / g.tile_height_px)
    const start = sy * g.width_cells + sx, target = ty * g.width_cells + tx
    const parents = new Int32Array(g.width_cells * g.height_cells).fill(-1), queue = [start]; parents[start] = start
    if (!line(from, center(sx, sy)) || !line(center(tx, ty), to)) return null
    for (let head = 0; head < queue.length && parents[target] === -1; head++) {
      const cell = queue[head]!, x = cell % g.width_cells, y = Math.floor(cell / g.width_cells)
      for (const [nx, ny] of [[x + 1, y], [x, y + 1], [x - 1, y], [x, y - 1]] as const) {
        const next = ny * g.width_cells + nx
        if (nx >= 0 && ny >= 0 && nx < g.width_cells && ny < g.height_cells && parents[next] === -1 && line(center(x, y), center(nx, ny))) { parents[next] = cell; queue.push(next) }
      }
    }
    if (parents[target] === -1) return null
    const cells = [target]; while (cells.at(-1) !== start) cells.push(parents[cells.at(-1)!]!)
    const points = cells.reverse().map(cell => center(cell % g.width_cells, Math.floor(cell / g.width_cells)))
    points.push(to)
    return points.filter((point, index) => Math.hypot(point.x - (index === 0 ? from.x : points[index - 1]!.x), point.y - (index === 0 ? from.y : points[index - 1]!.y)) > epsilon)
  }
  return { walkable, move, line, route, center }
}
export type Collision = ReturnType<typeof createCollision>
