import { synthetic, rehash } from './synthetic'
import { sha256 } from '../../map/json'
/** Feature-test-only terrain; no character or production map/venue assets. */
export async function movementData() {
  const f = await synthetic(), g = f.map.geometry
  g.width_cells = 24; g.height_cells = 14
  g.collision_grid = Array.from({ length: g.height_cells }, (_, y) => Array.from({ length: g.width_cells }, (_, x) => x === 0 || y === 0 || x === 23 || y === 13 || (x === 8 && y < 12 && y !== 7) ? 1 : 0))
  g.terrain_layer = g.collision_grid.map(row => row.map(cell => cell ? 2 : 1))
  g.overdraw_layer = g.collision_grid.map(row => row.map(() => 0)); g.overdraw_layer[3]![2] = 3
  g.spawns[0]!.cell_x = 2; g.spawns[0]!.cell_y = 3
  g.interactions[0]!.cell_x = 4; g.interactions[0]!.cell_y = 3
  g.interactions.push({ ...g.interactions[0]!, interaction_id: 'inspect_far', cell_x: 6, cell_y: 10 })
  f.theme.interaction_ids = g.interactions.map(target => target.interaction_id)
  f.theme.texture.path = 'world/fe15/movement-tiles.png'
  const tiled = { ...f.tiled, width: g.width_cells, height: g.height_cells,
    layers: Object.entries({ terrain: g.terrain_layer, collision: g.collision_grid, overdraw: g.overdraw_layer }).map(([name, grid], i) => ({ id: i + 1, name, type: 'tilelayer', width: g.width_cells, height: g.height_cells, x: 0, y: 0, opacity: 1, visible: true, data: grid.flat() })),
    tilesets: f.tiled.tilesets.map(set => ({ ...set, image: f.theme.texture.path })) }
  const tiledText = JSON.stringify(tiled), tiledBytes = new TextEncoder().encode(tiledText)
  f.map.tilemap.path = 'world/fe15/movement-map.json'; f.map.tilemap.sha256 = await sha256(tiledBytes)
  const pin = await rehash(f.map, f.theme)
  return { ...f, tiled, tiledText, tiledBytes, pin, allow: [{ path: f.map.tilemap.path, sha256: f.map.tilemap.sha256, kind: 'json' as const }, { path: f.theme.texture.path, sha256: f.theme.texture.sha256!, kind: 'png' as const }] }
}
