import bootstrap from './source-inputs/operator-bootstrap.json'
import { validateManifests, type MapManifest, type ThemeManifest } from '../../../../world/map/manifest'
import type { LoadedMap } from '../../../../world/map/resources'
import { sceneryDescriptors } from '../../../../world/illustrated/model'

/** Fake decoded handles for unit checks only; public constructor DTOs remain unmodified. */
export async function loadedMap4(themeId: 'REVENGE' | 'PARADE' | 'DANGER' = 'PARADE'): Promise<LoadedMap> {
  const map = structuredClone(bootstrap.map_manifest) as MapManifest
  const theme = structuredClone(bootstrap.theme_manifests.find(t => t.theme_id === themeId)!) as ThemeManifest
  const metadata = await validateManifests(JSON.stringify(map), JSON.stringify(theme), {
    mapHash: map.content_hash, themeHash: theme.content_hash, avatar: bootstrap.avatar_manifest,
  })
  const descriptors = sceneryDescriptors(theme)
  const assets = descriptors.map(descriptor => ({ descriptor, image: {
    naturalWidth: descriptor.width_px, naturalHeight: descriptor.height_px,
  } as HTMLImageElement }))
  return { ...metadata, image: assets[0]!.image, dispose() {}, scenery: {
    manifest: theme.scenery!, assets,
    transferBytes: descriptors.reduce((n, d) => n + d.bytes, 0),
    decodedBytes: descriptors.reduce((n, d) => n + d.width_px * d.height_px * 4, 0),
  } }
}
