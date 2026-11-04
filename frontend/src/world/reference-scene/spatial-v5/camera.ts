import type { LoadedMap } from '../../map/resources'
import type { Point } from '../../camera/port'
import { getSpatialV5OverviewMetadata } from './model'
import source from './source-inputs.json'

/** Visual projection only. Logical zoom, server feet, collisions and input policy stay unchanged. */
export function spatialV5CameraPresentation(map: Pick<LoadedMap, 'map' | 'theme' | 'widthPx' | 'heightPx'>,
  viewport: { width: number; height: number }, focus: Point, logicalZoom: number) {
  if (!getSpatialV5OverviewMetadata({ mapId: map.map.map_id, mapRevision: map.map.map_revision,
    geometryHash: map.map.geometry_hash, themeId: map.theme.theme_id, width: map.widthPx, height: map.heightPx })) return null
  if (![viewport.width, viewport.height, focus.x, focus.y, logicalZoom].every(Number.isFinite)
    || viewport.width <= 0 || viewport.height <= 0 || logicalZoom < 1 || logicalZoom > 8) return null
  const shot = source.world_camera
  const scale = Math.min(1, viewport.width / shot.width, viewport.height / shot.height)
  // Bounded look-ahead retains the authored stage/self composition near spawn and follows further movement.
  const offsetX = Math.max(-32, Math.min(32, focus.x - (shot.x + shot.width / 2)))
  const offsetY = Math.max(-160, Math.min(160, focus.y - (shot.y + shot.height / 2)))
  return { zoom: logicalZoom * scale, center: { x: focus.x - offsetX, y: focus.y - offsetY } }
}
