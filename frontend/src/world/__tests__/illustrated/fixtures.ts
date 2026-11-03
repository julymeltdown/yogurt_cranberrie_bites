// TEST ONLY assembly of original immutable scenery/map sources, not a C03/avatar release.
import mapBase from '../../../../../contracts/fixtures/assets/map-manifest.json'
import themeBase from '../../../../../contracts/fixtures/assets/theme-revenge.json'
import type geometrySource from '../../../../public/world/maps/geometry.json'
import type placesSource from '../../../../public/world/maps/virtual-places.json'
import type mapBindingSource from '../../../../public/world/maps/binding-inputs.json'
import type sourceThemesSource from '../../../../public/world/themes/binding-inputs.json'
import type bindingsSource from '../../../../public/world/illustrated-runtime/bindings.json'
import type { MapManifest, ThemeManifest } from '../../map/manifest'
import type { Scenery } from '../../illustrated/model'
import { contentHash, digest } from '../../map/json'
export async function illustratedReleases(read: (path: string) => Promise<unknown> = path => fetch('/' + path).then(r => { if (!r.ok) throw Error(path); return r.json() })) {
  const [geometry, places, mapBinding, sourceThemes, bindings] = await Promise.all([
    'world/maps/geometry.json', 'world/maps/virtual-places.json', 'world/maps/binding-inputs.json',
    'world/themes/binding-inputs.json', 'world/illustrated-runtime/bindings.json',
  ].map(read)) as [typeof geometrySource, typeof placesSource, typeof mapBindingSource, typeof sourceThemesSource, typeof bindingsSource]
  const map: MapManifest = { ...structuredClone(mapBase), map_id: mapBinding.map_id, map_revision: mapBinding.map_revision,
    provenance: structuredClone(places[0]!.provenance), geometry: structuredClone(geometry), places: structuredClone(places),
    geometry_hash: mapBinding.geometry_hash, tilemap: { format: 'TILED_JSON', resource_state: 'PROVIDED', path: mapBinding.tilemap_path, sha256: mapBinding.tilemap_sha256 } }
  map.content_hash = await contentHash(map)
  const avatar = { manifest_id: 'illustrated_test_only_unavailable', manifest_version: 1,
    content_hash: await digest({ fixture: 'TEST ONLY unavailable avatar; no approved identity or character art' }) }
  const themes = await Promise.all(bindings.themes.map(async entry => {
    const src = sourceThemes.themes.find(t => t.theme_id === entry.theme_id)!
    const theme: ThemeManifest = { ...structuredClone(themeBase), theme_id: entry.theme_id, texture: structuredClone(src.texture), palette: structuredClone(src.palette),
      provenance: structuredClone(src.provenance), map_id: map.map_id, map_revision: map.map_revision, map_content_hash: map.content_hash,
      geometry_hash: map.geometry_hash, collision_id: map.geometry.collision_id, place_ids: map.places.map(p => p.place_id),
      interaction_ids: map.geometry.interactions.map(p => p.interaction_id), avatar_manifest_id: avatar.manifest_id,
      avatar_manifest_version: avatar.manifest_version, avatar_content_hash: avatar.content_hash, scenery: structuredClone(entry.scenery) as Scenery }
    theme.content_hash = await contentHash(theme); return theme
  }))
  return { map, themes, avatar }
}
