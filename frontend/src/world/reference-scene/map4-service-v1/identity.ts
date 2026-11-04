import type { LoadedMap } from '../../map/resources'

/** Public operator-bundle/quality-v6 pins. Presentation never grants release admission. */
export const map4Parade = Object.freeze({
  mapId: 'mcr_reference_scene_r3', mapRevision: 4,
  mapHash: 'sha256:736afd273a067f6c40db76b8cc54412416fba88984a50d9a6b6738d869831fb8',
  geometryHash: 'sha256:899aca9bd6ce499c6f46aff5eaf21e30ab34b301e988771b978a6fab7c2d469f',
  themeId: 'PARADE', themeRevision: 1,
  themeHash: 'sha256:334011156771af9cfa199671f4977c517921bb877928245f501ecde3a55f163b',
  width: 768, height: 1296,
})

export const map4ThemeHashes = Object.freeze({
  REVENGE: 'sha256:2c456a8cb4b601a66c07577fa105136e11b1d4e4d9c401eb7e3181fed706e04f',
  PARADE: map4Parade.themeHash,
  DANGER: 'sha256:fdf92937e87445ca82826da062d7d21e673dd9e6d463a36e9d95b7acce55546a',
})

export function matchesMap4Release(map: LoadedMap): boolean {
  const pin = map4Parade
  return map.map.map_id === pin.mapId && map.map.map_revision === pin.mapRevision
    && map.map.content_hash === pin.mapHash && map.map.geometry_hash === pin.geometryHash
    && Object.hasOwn(map4ThemeHashes, map.theme.theme_id) && map.theme.theme_revision === pin.themeRevision
    && map.theme.content_hash === map4ThemeHashes[map.theme.theme_id as keyof typeof map4ThemeHashes] && map.theme.map_content_hash === pin.mapHash
    && map.theme.map_id === pin.mapId && map.theme.map_revision === pin.mapRevision
    && map.theme.geometry_hash === pin.geometryHash && map.widthPx === pin.width && map.heightPx === pin.height
}
