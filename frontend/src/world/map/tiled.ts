import type { Geometry, ThemeManifest } from './manifest'
import { canonicalJson, record, requireMap, strictJson } from './json'
const only = (value: Record<string, unknown>, keys: string[]) => requireMap(Object.keys(value).every(k => keys.includes(k)))
/** Deliberately finite orthogonal subset. No Tiled loader/plugin may discover other resources. */
export function validateTiled(text: string, geometry: Geometry, texture: ThemeManifest['texture']) {
  const map = record(strictJson(text))
  only(map, ['type', 'orientation', 'renderorder', 'infinite', 'width', 'height', 'tilewidth', 'tileheight', 'layers', 'tilesets', 'version', 'tiledversion'])
  requireMap(map.type === 'map' && map.orientation === 'orthogonal' && map.renderorder === 'right-down' && map.infinite === false
    && map.version === '1.10' && typeof map.tiledversion === 'string' && map.tiledversion.length <= 32)
  requireMap(map.width === geometry.width_cells && map.height === geometry.height_cells && map.tilewidth === geometry.tile_width_px && map.tileheight === geometry.tile_height_px)
  requireMap(Array.isArray(map.layers) && map.layers.length === 3)
  const ids = new Set<number>(), names = new Set<string>()
  const grids = { terrain: geometry.terrain_layer, collision: geometry.collision_grid, overdraw: geometry.overdraw_layer }
  for (const value of map.layers) {
    const layer = record(value)
    only(layer, ['id', 'name', 'type', 'width', 'height', 'x', 'y', 'opacity', 'visible', 'data'])
    requireMap(typeof layer.id === 'number' && layer.id > 0 && !ids.has(layer.id)); ids.add(layer.id)
    requireMap(typeof layer.name === 'string' && Object.hasOwn(grids, layer.name) && !names.has(layer.name)); names.add(layer.name)
    requireMap(layer.type === 'tilelayer' && layer.width === map.width && layer.height === map.height && layer.x === 0 && layer.y === 0 && layer.opacity === 1 && layer.visible === true)
    requireMap(canonicalJson(layer.data) === canonicalJson(grids[layer.name as keyof typeof grids].flat()))
  }
  requireMap(Array.isArray(map.tilesets) && map.tilesets.length === 1)
  const set = record(map.tilesets[0])
  only(set, ['firstgid', 'name', 'image', 'imagewidth', 'imageheight', 'tilewidth', 'tileheight', 'tilecount', 'columns', 'margin', 'spacing'])
  requireMap(set.firstgid === 1 && set.name === 'theme' && set.image === texture.path && set.imagewidth === texture.width_px && set.imageheight === texture.height_px
    && set.tilewidth === map.tilewidth && set.tileheight === map.tileheight && set.columns === texture.width_px / geometry.tile_width_px
    && set.tilecount === texture.width_px / geometry.tile_width_px * (texture.height_px / geometry.tile_height_px) && set.margin === 0 && set.spacing === 0)
}
