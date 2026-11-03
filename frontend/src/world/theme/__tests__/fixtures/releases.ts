import { synthetic, rehash } from '../../../__tests__/fixtures/synthetic'
import { testPng } from './png'
import { sha256 } from '../../../map/json'
import { THEME_IMAGE_MARKER } from '../../../map/tiled'
import type { ThemeId } from '../../../../features/theme/model/preferences'
/** Synthetic FE06 resource catalog ONLY. No character art or production manifest. */
export async function syntheticRelease(id: ThemeId) {
  const f = await synthetic()
  f.png = Uint8Array.from(atob(testPng[id]), c => c.charCodeAt(0))
  f.theme.texture.sha256 = await sha256(f.png)
  f.theme.theme_id = id
  f.theme.palette.background = { REVENGE: '#511427', PARADE: '#151515', DANGER: '#DCE9E0' }[id]
  f.theme.texture.path = `world/fe06/${id.toLowerCase()}.png`
  f.tiled.tilesets[0]!.image = THEME_IMAGE_MARKER
  f.tiledText = JSON.stringify(f.tiled); f.tiledBytes = new TextEncoder().encode(f.tiledText)
  f.map.tilemap.path = 'world/fe06/map.json'
  f.map.tilemap.sha256 = await sha256(f.tiledBytes)
  f.pin = await rehash(f.map, f.theme)
  f.allow = [{ path: f.map.tilemap.path, sha256: f.map.tilemap.sha256, kind: 'json' }, { path: f.theme.texture.path, sha256: f.theme.texture.sha256!, kind: 'png' }]
  return f
}
