import { requireMap } from '../../map/json'
import { map4Parade, map4ThemeHashes } from '../map4-service-v1/identity'
import { canonicalCells, compositionVersion, serviceSemantics, type CanonicalInteraction, type ServiceKey } from './semantics'

export type ThemeId = keyof typeof map4ThemeHashes
export interface ResourcePin { readonly path: string; readonly bytes: number; readonly sha256: string }
export interface ScenePin extends ResourcePin {
  readonly themeId: ThemeId; readonly width: 768; readonly height: 1296; readonly mime: 'image/png'
}
export interface CompositionRegistration {
  readonly version: typeof compositionVersion
  readonly layout: ResourcePin
  readonly assets: readonly ScenePin[]
  readonly presentationHash: string
}
export interface Rect { readonly x: number; readonly y: number; readonly width: number; readonly height: number }
export interface ServiceAnchor {
  readonly key: ServiceKey; readonly x: number; readonly y: number; readonly width: number; readonly exclusionRect: Rect
}
export interface CanonicalAnchor {
  readonly interactionId: CanonicalInteraction; readonly x: number; readonly y: number
  readonly worldCell: Readonly<{ x: number; y: number }>; readonly authority: 'EXISTING_CANONICAL_IDENTITY_ONLY'
}
export interface CompositionLayout {
  readonly serviceLabelAnchors: readonly ServiceAnchor[]
  readonly canonicalDisplayAnchors: readonly CanonicalAnchor[]
}
const sha = (value: unknown): value is string => typeof value === 'string' && /^sha256:[0-9a-f]{64}$/.test(value)
const integer = (v: unknown, max: number): v is number => Number.isSafeInteger(v) && (v as number) > 0 && (v as number) <= max
function record(value: unknown): Record<string, unknown> {
  requireMap(!!value && typeof value === 'object' && !Array.isArray(value)); return value as Record<string, unknown>
}
function list(value: unknown): unknown[] { requireMap(Array.isArray(value)); return value }
function point(value: unknown, max: number): number {
  requireMap(typeof value === 'number' && Number.isFinite(value) && value >= 0 && value <= max); return value
}
function rectangle(value: unknown): Rect {
  const r = record(value), x = point(r.x, 768), y = point(r.y, 1296), width = point(r.width, 768), height = point(r.height, 1296)
  requireMap(width > 0 && height > 0 && x + width <= 768 && y + height <= 1296)
  return Object.freeze({ x, y, width, height })
}
export function validateCompositionRegistration(value: CompositionRegistration) {
  requireMap(value.version === compositionVersion && sha(value.presentationHash)
    && value.layout.path === 'world/map-roof-street-derived-v2/layout.json'
    && integer(value.layout.bytes, 524_288) && sha(value.layout.sha256))
  const themes = new Set<string>()
  requireMap(value.assets.length === 3)
  for (const asset of value.assets) {
    requireMap(Object.hasOwn(map4ThemeHashes, asset.themeId) && !themes.has(asset.themeId)
      && asset.path === `world/map-roof-street-derived-v2/${asset.themeId.toLowerCase()}-scene.png`
      && integer(asset.bytes, 2_097_152) && sha(asset.sha256) && asset.width === 768 && asset.height === 1296 && asset.mime === 'image/png')
    themes.add(asset.themeId)
  }
}

/** Parse presentation coordinates only. Original manifests remain the sole World authority. */
export function decodeCompositionLayout(value: unknown, registration: CompositionRegistration): CompositionLayout {
  validateCompositionRegistration(registration)
  const document = record(value), dimensions = record(document.dimensions), original = record(document.originalMap4)
  requireMap(document.schemaVersion === 1 && document.version === compositionVersion
    && document.coordinateSpace === 'MAP_ILLUSTRATION_ONLY' && document.renderMode === 'full-composition'
    && dimensions.width === 768 && dimensions.height === 1296
    && original.mapId === map4Parade.mapId && original.mapRevision === 4
    && original.mapContentHash === map4Parade.mapHash && original.geometryHash === map4Parade.geometryHash)
  const binding = record(original.sourceBinding)
  requireMap(typeof binding.path === 'string' && !binding.path.includes('..') && !binding.path.includes('://')
    && integer(binding.bytes, 2_097_152) && sha(binding.sha256))
  const themes = list(original.themes), seenThemes = new Set<string>()
  requireMap(themes.length === 3)
  for (const value of themes) {
    const theme = record(value)
    requireMap(typeof theme.themeId === 'string' && Object.hasOwn(map4ThemeHashes, theme.themeId)
      && !seenThemes.has(theme.themeId) && theme.themeRevision === 1
      && theme.themeContentHash === map4ThemeHashes[theme.themeId as ThemeId])
    seenThemes.add(theme.themeId)
  }
  const assets = list(document.assets)
  requireMap(assets.length === 3)
  const assetThemes = new Set<string>()
  for (const value of assets) {
    const asset = record(value), expected = registration.assets.find(p => p.themeId === asset.themeId)
    requireMap(expected && !assetThemes.has(expected.themeId)
      && ['themeId', 'path', 'bytes', 'sha256', 'width', 'height', 'mime'].every(key => asset[key] === Reflect.get(expected, key)))
    assetThemes.add(expected.themeId)
  }
  const labels = list(document.serviceLabelAnchors), labelKeys = new Set<string>()
  requireMap(labels.length === 6)
  const serviceLabelAnchors = labels.map(value => {
    const a = record(value), semantic = serviceSemantics.find(s => s.key === a.key)
    requireMap(semantic && !labelKeys.has(semantic.key)); labelKeys.add(semantic.key)
    const x = point(a.x, 768), y = point(a.y, 1296), width = point(a.width, 352), exclusionRect = rectangle(a.exclusionRect)
    requireMap(width >= 128 && x - width / 2 >= 5 && x + width / 2 <= 763 && y - 39 >= 5 && y + 39 <= 1291
      && exclusionRect.x <= x - width / 2 && exclusionRect.x + exclusionRect.width >= x + width / 2
      && exclusionRect.y <= y - 39 && exclusionRect.y + exclusionRect.height >= y + 39 && exclusionRect.height >= 104)
    return Object.freeze({ key: semantic.key, x, y, width, exclusionRect })
  })
  const canonical = list(document.canonicalDisplayAnchors), canonicalIds = new Set<string>()
  requireMap(canonical.length === 4)
  const canonicalDisplayAnchors = canonical.map(value => {
    const a = record(value), cell = record(a.worldCell)
    requireMap(typeof a.interactionId === 'string' && Object.hasOwn(canonicalCells, a.interactionId)
      && !canonicalIds.has(a.interactionId) && a.authority === 'EXISTING_CANONICAL_IDENTITY_ONLY')
    canonicalIds.add(a.interactionId)
    const interactionId = a.interactionId as CanonicalInteraction, expected = canonicalCells[interactionId]
    requireMap(cell.x === expected.x && cell.y === expected.y)
    return Object.freeze({ interactionId, x: point(a.x, 768), y: point(a.y, 1296),
      worldCell: Object.freeze({ ...expected }), authority: 'EXISTING_CANONICAL_IDENTITY_ONLY' as const })
  })
  return Object.freeze({ serviceLabelAnchors: Object.freeze(serviceLabelAnchors), canonicalDisplayAnchors: Object.freeze(canonicalDisplayAnchors) })
}
