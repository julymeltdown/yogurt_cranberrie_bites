import type { Point } from '../camera/port'
import type { LoadedMap } from '../map/resources'
import type { Collision } from '../input/simulation'
export interface NearbyTarget { readonly interactionId: string; readonly placeId: string; readonly label: string; readonly distancePx: number }
/** Local eligibility only: explicit injected range and unobstructed foot sweep, never domain authorization. */
export function nearbyTargets(map: LoadedMap, foot: Point, collision: Collision, radius: number): readonly NearbyTarget[] {
  const places = new Map(map.map.places.map(place => [place.place_id, place]))
  return map.map.geometry.interactions.flatMap(target => {
    const place = places.get(target.place_id), point = collision.center(target.cell_x, target.cell_y), distancePx = Math.hypot(point.x - foot.x, point.y - foot.y)
    return place?.closure_state === 'OPEN' && distancePx <= radius && collision.line(foot, point)
      ? [{ interactionId: target.interaction_id, placeId: target.place_id, label: place.label, distancePx }] : []
  }).sort((a, b) => a.distancePx - b.distancePx || (a.interactionId < b.interactionId ? -1 : a.interactionId > b.interactionId ? 1 : 0))
}
