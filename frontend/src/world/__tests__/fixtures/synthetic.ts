import originalMap from '../../../../../contracts/fixtures/assets/map-manifest.json'
import originalTheme from '../../../../../contracts/fixtures/assets/theme-revenge.json'
import { contentHash, digest, sha256 } from '../../map/json'
import type { MapManifest, ThemeManifest } from '../../map/manifest'
import { pngBase64 } from './png'
export const imageBytes = () => Uint8Array.from(atob(pngBase64), c => c.charCodeAt(0))
export async function rehash(map: MapManifest, theme: ThemeManifest) {
  map.geometry_hash = await digest(map.geometry); map.content_hash = await contentHash(map)
  Object.assign(theme, { map_id: map.map_id, map_revision: map.map_revision, map_content_hash: map.content_hash,
    geometry_hash: map.geometry_hash, collision_id: map.geometry.collision_id })
  theme.content_hash = await contentHash(theme)
  return { mapHash: map.content_hash, themeHash: theme.content_hash,
    avatar: { manifest_id: theme.avatar_manifest_id, manifest_version: theme.avatar_manifest_version, content_hash: theme.avatar_content_hash } }
}
export async function synthetic() {
  const map: MapManifest = structuredClone(originalMap), theme: ThemeManifest = structuredClone(originalTheme)
  const g = map.geometry
  g.terrain_layer = g.terrain_layer.map((row, y) => row.map((_v, x) => g.collision_grid[y]![x] ? 2 : 1))
  g.overdraw_layer[1]![1] = 3
  theme.texture = { image_state: 'PROVIDED', path: 'world/fe14/synthetic-tiles.png', sha256: await sha256(imageBytes()), width_px: 96, height_px: 32 }
  const tiled = { type: 'map', orientation: 'orthogonal', renderorder: 'right-down', infinite: false, width: g.width_cells, height: g.height_cells,
    tilewidth: g.tile_width_px, tileheight: g.tile_height_px, version: '1.10', tiledversion: '1.11.0',
    layers: Object.entries({ terrain: g.terrain_layer, collision: g.collision_grid, overdraw: g.overdraw_layer }).map(([name, grid], i) => ({ id: i + 1, name, type: 'tilelayer', width: g.width_cells, height: g.height_cells, x: 0, y: 0, opacity: 1, visible: true, data: grid.flat() })),
    tilesets: [{ firstgid: 1, name: 'theme', image: theme.texture.path, imagewidth: 96, imageheight: 32, tilewidth: g.tile_width_px, tileheight: g.tile_height_px, tilecount: 3, columns: 3, margin: 0, spacing: 0 }] }
  const tiledText = JSON.stringify(tiled), tiledBytes = new TextEncoder().encode(tiledText)
  map.tilemap = { format: 'TILED_JSON', resource_state: 'PROVIDED', path: 'world/fe14/synthetic-map.json', sha256: await sha256(tiledBytes) }
  const pin = await rehash(map, theme)
  const allow = [{ path: map.tilemap.path!, sha256: map.tilemap.sha256!, kind: 'json' as const }, { path: theme.texture.path!, sha256: theme.texture.sha256!, kind: 'png' as const }]
  return { map, theme, pin, tiled, tiledText, tiledBytes, png: imageBytes(), allow }
}
