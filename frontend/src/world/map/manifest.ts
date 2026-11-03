import type fixtureMap from '../../../../contracts/fixtures/assets/map-manifest.json'
import type fixtureTheme from '../../../../contracts/fixtures/assets/theme-revenge.json'
import { contentHash, digest, MapError, record, requireMap, strictJson } from './json'
import { schemaValid } from './schema'
export type Geometry = typeof fixtureMap.geometry
type Provenance = Omit<typeof fixtureMap.provenance, 'source_uri' | 'reviewed_by' | 'reviewed_at'> & { source_uri: string | null; reviewed_by: string | null; reviewed_at: string | null }
export type MapManifest = Omit<typeof fixtureMap, 'tilemap' | 'provenance' | 'places'> & { provenance: Provenance; places: (Omit<typeof fixtureMap.places[number], 'provenance'> & { provenance: Provenance })[]; tilemap: { format: string; resource_state: string; path: string | null; sha256: string | null } }
export type ThemeManifest = Omit<typeof fixtureTheme, 'texture' | 'provenance'> & { provenance: Provenance; texture: { image_state: string; path: string | null; sha256: string | null; width_px: number; height_px: number } }
/** Trusted release identity supplied by future composition; no avatar image/layout consumption. */
export interface ReleasePin {
  mapHash: string; themeHash: string
  avatar: { manifest_id: string; manifest_version: number; content_hash: string }
}
export const limits = Object.freeze({ manifestBytes: 1_048_576, tiledBytes: 1_048_576, pngBytes: 4_194_304,
  gridSide: 128, cells: 16_384, tileSide: 128, textureSide: 2048, texturePixels: 4_194_304, entries: 256,
  viewportSide: 2048, deadlineMs: 12_000 })
function unique<T>(entries: readonly T[], key: (value: T) => string) {
  requireMap(entries.length <= limits.entries)
  const result = new Map<string, T>()
  for (const entry of entries) { const id = key(entry); requireMap(!result.has(id)); result.set(id, entry) }
  return result
}
function freeze<T>(value: T): T {
  if (value && typeof value === 'object') { for (const child of Object.values(value)) freeze(child); Object.freeze(value) }
  return value
}
export async function validateManifests(mapText: string, themeText: string, pin: ReleasePin) {
  const m = strictJson(mapText), t = strictJson(themeText)
  requireMap(schemaValid(m, 'map') && schemaValid(t, 'theme'))
  const map = m as MapManifest, theme = t as ThemeManifest, g = map.geometry
  requireMap(await contentHash(record(map)) === map.content_hash && map.content_hash === pin.mapHash)
  requireMap(await contentHash(record(theme)) === theme.content_hash && theme.content_hash === pin.themeHash)
  requireMap(await digest(g) === map.geometry_hash)
  requireMap(g.width_cells <= limits.gridSide && g.height_cells <= limits.gridSide && g.width_cells * g.height_cells <= limits.cells
    && g.tile_width_px <= limits.tileSide && g.tile_height_px <= limits.tileSide)
  for (const grid of [g.collision_grid, g.terrain_layer, g.overdraw_layer]) requireMap(grid.length === g.height_cells && grid.every(row => row.length === g.width_cells))
  const places = unique(map.places, e => e.place_id), spawns = unique(g.spawns, e => e.spawn_id), interactions = unique(g.interactions, e => e.interaction_id)
  requireMap(spawns.has(g.default_spawn_id))
  const walkable = (x: number, y: number) => x >= 0 && y >= 0 && x < g.width_cells && y < g.height_cells && g.collision_grid[y]![x] === 0
  for (const place of places.values()) if (place.provenance.source_kind === 'ORIGINAL_FIXTURE') requireMap(place.place_kind === 'VIRTUAL_CONCEPT' && place.verification_state === 'CONCEPT_ONLY')
  for (const point of [...spawns.values(), ...interactions.values()]) requireMap(walkable(point.cell_x, point.cell_y))
  for (const point of interactions.values()) requireMap(places.has(point.place_id))
  // Label each component once: equivalent to every-spawn BFS without O(spawns*cells) work.
  const components = new Int32Array(g.width_cells * g.height_cells).fill(-1)
  let component = 0
  for (const spawn of spawns.values()) {
    const index = spawn.cell_y * g.width_cells + spawn.cell_x
    if (components[index] === -1) {
      const queue = [index]; components[index] = component
      for (let head = 0; head < queue.length; head++) {
        const cell = queue[head]!, x = cell % g.width_cells, y = Math.floor(cell / g.width_cells)
        for (const [nx, ny] of [[x - 1, y], [x + 1, y], [x, y - 1], [x, y + 1]] as [number, number][]) {
          const next = ny * g.width_cells + nx
          if (walkable(nx, ny) && components[next] === -1) { components[next] = component; queue.push(next) }
        }
      }
      component++
    }
    requireMap([...interactions.values()].every(p => components[p.cell_y * g.width_cells + p.cell_x] === components[index]))
  }
  for (const [key, expected] of Object.entries({ map_id: map.map_id, map_revision: map.map_revision, map_content_hash: map.content_hash,
    geometry_hash: map.geometry_hash, collision_id: g.collision_id, avatar_manifest_id: pin.avatar.manifest_id,
    avatar_manifest_version: pin.avatar.manifest_version, avatar_content_hash: pin.avatar.content_hash })) requireMap(record(theme)[key] === expected)
  const equalIds = (ids: string[], expected: Map<string, unknown>) => ids.length === expected.size && ids.every(id => expected.has(id))
  requireMap(equalIds(theme.place_ids, places) && equalIds(theme.interaction_ids, interactions))
  const texture = theme.texture
  requireMap(texture.width_px <= limits.textureSide && texture.height_px <= limits.textureSide && texture.width_px * texture.height_px <= limits.texturePixels
    && texture.width_px % g.tile_width_px === 0 && texture.height_px % g.tile_height_px === 0)
  const count = texture.width_px / g.tile_width_px * (texture.height_px / g.tile_height_px)
  requireMap([g.terrain_layer, g.overdraw_layer].every(grid => grid.every(row => row.every(id => id <= count))))
  return freeze({ map, theme, spawn: spawns.get(g.default_spawn_id)!, widthPx: g.width_cells * g.tile_width_px, heightPx: g.height_cells * g.tile_height_px })
}
export function requireProvided(map: MapManifest, theme: ThemeManifest) {
  if (map.tilemap.resource_state === 'NOT_PROVIDED' || theme.texture.image_state === 'NOT_PROVIDED') throw new MapError('unavailable')
  requireMap(map.tilemap.path && map.tilemap.sha256 && theme.texture.path && theme.texture.sha256)
}
