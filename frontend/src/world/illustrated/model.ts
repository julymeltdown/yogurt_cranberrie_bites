import { canonicalJson, requireMap } from '../map/json'
import type { Resource } from '../map/resources'
import { registeredPhoto2Ground } from '../reference-scene/photo2-world-display-v1/model'

export interface SceneryTexture {
  readonly id: string; readonly path: string; readonly sha256: string
  readonly width_px: number; readonly height_px: number; readonly bytes: number
}
export interface SceneryObject {
  readonly id: string; readonly atlas_id: string
  readonly frame: Readonly<{ x: number; y: number; width: number; height: number }>
  readonly x: number; readonly y: number; readonly width: number; readonly height: number; readonly depth_y: number
}
export interface Scenery {
  readonly version: 1; readonly ground: SceneryTexture
  readonly atlases: readonly SceneryTexture[]; readonly objects: readonly SceneryObject[]
}
export interface LoadedScenery {
  readonly manifest: Scenery
  readonly assets: readonly Readonly<{ descriptor: SceneryTexture; image: HTMLImageElement }>[]
  readonly transferBytes: number; readonly decodedBytes: number
}
export interface SceneryDiagnostics {
  readonly textures: number; readonly decodedBytes: number; readonly objects: number
  readonly peakTextures: number; readonly peakDecodedBytes: number
}
/** Bound both steady-state and the two simultaneously resident skins during an atomic swap. */
export const sceneryLimits = Object.freeze({ textures: 4, objects: 96, transferBytes: 2_097_152,
  pixels: 2_097_152, side: 2048, objectSide: 1024, overhang: 128 })
export const sceneryDescriptors = (theme: { readonly scenery?: Scenery }) => theme.scenery
  ? [theme.scenery.ground, ...theme.scenery.atlases] : []
/** Pure allowlist projection. It does not confer bootstrap, provenance or privacy authority. */
export const sceneryResources = (theme: { readonly scenery?: Scenery }): Resource[] =>
  sceneryDescriptors(theme).map(t => ({ path: t.path, sha256: t.sha256, kind: 'png' }))

/** Called after closed C06 schema validation and before any resource fetch. */
export function validateScenery(s: Scenery | undefined, width: number, height: number) {
  if (!s) return
  requireMap(s.version === 1 && s.atlases.length > 0 && s.atlases.length < sceneryLimits.textures
    && s.objects.length > 0 && s.objects.length <= sceneryLimits.objects)
  const descriptors = [s.ground, ...s.atlases], ids = new Set<string>(), paths = new Set<string>()
  let pixels = 0, bytes = 0
  for (const t of descriptors) {
    requireMap(!ids.has(t.id) && !paths.has(t.path)); ids.add(t.id); paths.add(t.path)
    requireMap(t.width_px <= sceneryLimits.side && t.height_px <= sceneryLimits.side
      && t.bytes <= sceneryLimits.transferBytes)
    pixels += t.width_px * t.height_px; bytes += t.bytes
  }
  requireMap(bytes <= sceneryLimits.transferBytes && pixels <= sceneryLimits.pixels
    && ((s.ground.width_px === width && s.ground.height_px === height) || registeredPhoto2Ground(s, width, height)))
  const atlases = new Map(s.atlases.map(t => [t.id, t])), objects = new Set<string>()
  for (const o of s.objects) {
    const t = atlases.get(o.atlas_id), f = o.frame
    requireMap(t && !objects.has(o.id)); objects.add(o.id)
    requireMap(f.x + f.width <= t.width_px && f.y + f.height <= t.height_px
      && o.width <= sceneryLimits.objectSide && o.height <= sceneryLimits.objectSide
      && o.x >= -sceneryLimits.overhang && o.y >= -sceneryLimits.overhang
      && o.x < width && o.y < height && o.x + o.width > 0 && o.y + o.height > 0
      && o.x + o.width <= width + sceneryLimits.overhang && o.y + o.height <= height + sceneryLimits.overhang
      && o.depth_y <= height + sceneryLimits.overhang)
  }
}

/** Same scale supplied by the incumbent avatar renderer. Ground details stay below players. */
export const sceneryDepth = (footY: number, height: number) => footY === -1 ? .5 : 1 + footY / (height + 1) * .9

/** Renderer/replacement fence for an injected or incomplete decoded lease. */
export function validateLoadedScenery(next: { theme: { scenery?: Scenery }; scenery?: LoadedScenery; widthPx: number; heightPx: number }) {
  const s = next.theme.scenery, loaded = next.scenery
  requireMap(Boolean(s) === Boolean(loaded))
  if (!s || !loaded) return
  validateScenery(s, next.widthPx, next.heightPx)
  requireMap(canonicalJson(s) === canonicalJson(loaded.manifest))
  const descriptors = sceneryDescriptors(next.theme)
  requireMap(loaded.assets.length === descriptors.length)
  for (const [i, t] of descriptors.entries()) {
    const a = loaded.assets[i]!
    requireMap(canonicalJson(a.descriptor) === canonicalJson(t)
      && a.image.naturalWidth === t.width_px && a.image.naturalHeight === t.height_px)
  }
  requireMap(loaded.transferBytes === descriptors.reduce((n, t) => n + t.bytes, 0)
    && loaded.decodedBytes === descriptors.reduce((n, t) => n + t.width_px * t.height_px * 4, 0))
}
