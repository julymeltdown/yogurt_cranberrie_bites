export interface MuralProductionConfiguration {
  readonly version: 1
  readonly kind: 'PRODUCT_VIRTUAL'
  readonly sourceRevision: string
  readonly placementHash: string
  readonly map: Readonly<{ id: string; revision: number; contentHash: string; geometryHash: string; widthPx: number; heightPx: number }>
  readonly anchor: Readonly<{ x: number; y: number; width: number; height: number; depth: number }>
  readonly textureKey: string
}
const record = (value: unknown): value is Record<string, unknown> => value !== null && typeof value === 'object'
  && !Array.isArray(value) && (Object.getPrototypeOf(value) === Object.prototype || Object.getPrototypeOf(value) === null)
  && Object.getOwnPropertySymbols(value).length === 0
  && Object.values(Object.getOwnPropertyDescriptors(value)).every(descriptor => Object.hasOwn(descriptor, 'value'))
const keys = (value: Record<string, unknown>, expected: string[]) => Object.keys(value).length === expected.length && expected.every(key => Object.hasOwn(value, key))
const hash = (value: unknown) => typeof value === 'string' && /^sha256:[a-f0-9]{64}$/.test(value)
const integer = (value: unknown, minimum: number, maximum: number): value is number => typeof value === 'number' && Number.isSafeInteger(value) && value >= minimum && value <= maximum
/** Explicit product-art configuration, never a grant of identity, physical location or write authority. */
export function validateMuralProductionConfiguration(value: unknown): MuralProductionConfiguration | null {
  if (!record(value) || !keys(value, ['version', 'kind', 'sourceRevision', 'placementHash', 'map', 'anchor', 'textureKey'])
    || value.version !== 1 || value.kind !== 'PRODUCT_VIRTUAL' || typeof value.sourceRevision !== 'string'
    || !/^[a-f0-9]{40}$/.test(value.sourceRevision) || !hash(value.placementHash) || typeof value.textureKey !== 'string'
    || !/^mcr-product-virtual-wall-[a-z0-9-]{1,48}$/.test(value.textureKey)) return null
  const map = value.map, anchor = value.anchor
  if (!record(map) || !keys(map, ['id', 'revision', 'contentHash', 'geometryHash', 'widthPx', 'heightPx'])
    || typeof map.id !== 'string' || !/^[a-zA-Z0-9_-]{1,128}$/.test(map.id) || !integer(map.revision, 1, Number.MAX_SAFE_INTEGER)
    || !hash(map.contentHash) || !hash(map.geometryHash) || !integer(map.widthPx, 1, 16384) || !integer(map.heightPx, 1, 16384)
    || !record(anchor) || !keys(anchor, ['x', 'y', 'width', 'height', 'depth'])
    || !integer(anchor.x, 0, map.widthPx) || !integer(anchor.y, 0, map.heightPx)
    || !integer(anchor.width, 1, 2048) || !integer(anchor.height, 1, 2048)
    || anchor.x + anchor.width > map.widthPx || anchor.y + anchor.height > map.heightPx
    || typeof anchor.depth !== 'number' || !Number.isFinite(anchor.depth) || anchor.depth < 0 || anchor.depth > 10000) return null
  return Object.freeze({ version: 1, kind: 'PRODUCT_VIRTUAL', sourceRevision: value.sourceRevision, placementHash: value.placementHash as string,
    textureKey: value.textureKey, map: Object.freeze({ id: map.id, revision: map.revision, contentHash: map.contentHash as string,
      geometryHash: map.geometryHash as string, widthPx: map.widthPx, heightPx: map.heightPx }),
    anchor: Object.freeze({ x: anchor.x, y: anchor.y, width: anchor.width, height: anchor.height, depth: anchor.depth }) })
}
export function readMuralProductionConfiguration(raw: unknown): MuralProductionConfiguration | null {
  if (typeof raw !== 'string' || raw.length > 8192) return null
  try { return validateMuralProductionConfiguration(JSON.parse(raw)) } catch { return null }
}
