/** Pure derived-art admission only. No registry, resource loader or consumer wiring. */
export type Rect = readonly [number, number, number, number]
type Pin = { path: string; bytes: number; sha256: string }
type Decoration = {
  id: string; frame: string; rect: Rect; depth_y: number; interactive: boolean
  place_id: string | null; interaction_id: string | null; world_solid: boolean
  crop_role: string; density_group?: string
}
type Anchor = { x: number; y: number; exclusionRect: { x: number; y: number; width: number; height: number } }
export interface DensityLayout {
  [key: string]: unknown
  version: string; decorations: Decoration[]; serviceLabelAnchors: Anchor[]
  canonicalDisplayAnchors: { x: number; y: number }[]
  paths: number[][][]; gardenParcels: number[][][]
  derivation?: { sourceCommit: string; previousPresentationHash: string; addedDecorationIds: string[]; baselinePins: Pin[] }
}
type Crop = { themeId: string; frame: string; sourceRect: unknown; rgbaSha256: string; atlas: Pin }
export interface DensityProvenance {
  sourceCommit: string; previousPresentationHash: string; baselinePins: Pin[]
  sourceInputs: Pin[]; originalCropPins: Crop[]; additionCropPins: Crop[]
  outputs: Pin[]; recipe: Pin; addedDecorations: number
  changedRasterPixelsByTheme: Record<string, number>
  newSubstratePaintedPixelsByTheme: Record<string, number>
}
export type OriginalProvenance = Pick<DensityProvenance, 'sourceInputs' | 'originalCropPins'>
const SOURCE = 'af298e3246096c4b7a3ae5c7e439f5f33988a6be'
const PRESENTATION = 'sha256:d8437dda63dd5f0cb554bcfcc1c267782db6630fab39c541291bb3c368d154b6'
const FROZEN: Record<string, string> = {
  'README.md': 'dd5915ec730d780b20f9d164328760c39520dfdd57b5c2c0ee26c32bf1d9060c',
  'build.py': '6bf696b7c3edd5adf35e0b5995ca9cd33f915e87898de0e9266f2923cffc0433',
  'layout.json': '77da03bcddba15cb88a250a3cad4dbe9bdfadac519af1f1d4ac22623a8aa6141',
  'source-provenance.json': '0b9c5663e1fa182aae4012be465c448832ff798f926e930ee33c08cd456c084e',
  'revenge-scene.png': '0b6da66ac646ec28469c29d309f62e725ba24d48eee9b4b24e0c0383aedd3083',
  'parade-scene.png': '1d7cc73a1cdaf0931d446206a5424dd8f131c5c819e1f147a0a150583e98b493',
  'danger-scene.png': '8846875dabb8a48df8b8d8d5433eab5f682e215c02ac50a0fb6dcc2d7da54edf',
  'revenge-scene.svg': 'd3b6bb973996848be191579f3a8f018efda8cea4f8fe428c99f48f1721b1e0ed',
  'parade-scene.svg': '2d75bd6c760e140abc39da9bf6e0079edede28aeb0935ad8bc52bfffee92855c',
  'danger-scene.svg': '27ab6aa807bc6c63a6bff3c82d6d70189f20d3f93b7a09a536cf92c044e394a1',
}
const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b)
const overlaps = (a: Rect, b: Rect) => a[0] < b[0]+b[2] && b[0] < a[0]+a[2] && a[1] < b[1]+b[3] && b[1] < a[1]+a[3]

export function segmentHitsRect(a: readonly number[], b: readonly number[], rect: Rect, padding: number) {
  let low = 0, high = 1
  for (const axis of [0, 1]) {
    const start = a[axis]!, delta = b[axis]!-start
    const min = rect[axis]!-padding, max = rect[axis]!+rect[axis+2]!+padding
    if (!delta) { if (start < min || start > max) return false }
    else {
      const p = (min-start)/delta, q = (max-start)/delta
      low = Math.max(low, Math.min(p, q)); high = Math.min(high, Math.max(p, q))
      if (low > high) return false
    }
  }
  return true
}

/** Expects parsed records; callers must parse their document boundary separately. */
export function qualifyDensity(base: DensityLayout, candidate: DensityLayout, proof: DensityProvenance, original: OriginalProvenance): readonly string[] {
  const reasons = new Set<string>()
  const fail = (reason: string) => { reasons.add(reason) }
  if (candidate.version !== 'map-density-derived-v1' || proof.sourceCommit !== SOURCE
    || proof.previousPresentationHash !== PRESENTATION || candidate.derivation?.sourceCommit !== SOURCE
    || candidate.derivation?.previousPresentationHash !== PRESENTATION) fail('SOURCE_IDENTITY')
  for (const key of ['schemaVersion', 'coordinateSpace', 'renderMode', 'dimensions', 'originalMap4',
    'serviceLabelAnchors', 'canonicalDisplayAnchors', 'sourceInputs', 'paths', 'gardenParcels', 'plantingParcels', 'authority']) {
    if (!same(base[key], candidate[key])) fail('IMMUTABLE_LAYOUT:'+key)
  }
  const expected = Object.entries(FROZEN).map(([name, hash]) => ['frontend/public/world/map-reference-composition-v1/'+name, 'sha256:'+hash])
  const actual = new Map(proof.baselinePins.map(pin => [pin.path, pin.sha256]))
  if (actual.size !== 10 || proof.baselinePins.length !== 10 || expected.some(([path, hash]) => actual.get(path!) !== hash)
    || !same(proof.baselinePins, candidate.derivation?.baselinePins)) fail('BASELINE_PINS')
  if (!same(original.sourceInputs, proof.sourceInputs) || !same(original.originalCropPins, proof.originalCropPins)) fail('ORIGINAL_INPUT_OR_CROP_PINS')
  const old = new Map(base.decorations.map(row => [row.id, row]))
  const all = new Map(candidate.decorations.map(row => [row.id, row]))
  if (all.size !== candidate.decorations.length || [...old].some(([id, row]) => !same(row, all.get(id)))) fail('INCUMBENT_DECORATION')
  if (candidate.decorations.some((row, i, rows) => i > 0 && (rows[i-1]!.depth_y > row.depth_y
    || rows[i-1]!.depth_y === row.depth_y && rows[i-1]!.id > row.id))) fail('GLOBAL_FOOT_ORDER')
  const added = candidate.decorations.filter(row => !old.has(row.id))
  if (added.length < 12 || added.length !== proof.addedDecorations || !same(added.map(row => row.id).sort(), candidate.derivation?.addedDecorationIds.slice().sort())
    || new Set(added.map(row => row.density_group)).size < 6 || !added.some(row => row.density_group?.includes('junction'))) fail('DENSITY_INCREMENT')
  for (const row of added) {
    const [x, y, w, h] = row.rect, r = row.rect
    if (!row.id.startsWith('density_') || !['tree_green', 'tree_dark', 'tree_yellow', 'shrub_bed'].includes(row.frame)
      || row.interactive !== false || row.world_solid !== false || row.place_id !== null || row.interaction_id !== null || row.crop_role !== 'whole') fail('DECORATION_AUTHORITY')
    if (!r.every(Number.isFinite) || !r.every(Number.isInteger) || w <= 0 || h <= 0 || x < 0 || y < 0 || x+w > 768 || y+h > 1296 || row.depth_y !== y+h) fail('DECORATION_BOUNDS')
    if (base.serviceLabelAnchors.some(({exclusionRect:e}) => overlaps(r, [e.x, e.y, e.width, e.height]))) fail('LABEL_RESERVE')
    if (base.canonicalDisplayAnchors.some(p => Math.hypot(x+w/2-p.x, y+h/2-p.y) < Math.hypot(w/2, h/2)+34)) fail('CANONICAL_CLEARANCE')
    for (const path of base.paths) for (let i = 1; i < path.length; i++) if (segmentHitsRect(path[i-1]!, path[i]!, r, 25)) fail('PATH_CORRIDOR')
    for (const garden of base.gardenParcels) for (let i = 0; i < garden.length; i++) if (segmentHitsRect(garden[i]!, garden[(i+1)%garden.length]!, r, 13)) fail('GARDEN_BORDER')
    for (const previous of base.decorations) if (overlaps(r, previous.rect)
      && !previous.frame.startsWith('tree_')) fail('ARCHITECTURE_OVERPAINT')
  }
  for (const era of ['REVENGE', 'PARADE', 'DANGER']) {
    if (!(proof.changedRasterPixelsByTheme[era]! > 5000) || proof.newSubstratePaintedPixelsByTheme[era] !== 0) fail('ACTUAL_DELTA')
    for (const frame of new Set(added.map(row => row.frame))) {
      const crop = proof.additionCropPins.find(p => p.themeId === era && p.frame === frame)
      const frozen = original.originalCropPins.find(p => p.themeId === era && p.frame === frame)
      if (!crop || !frozen || crop.rgbaSha256 !== frozen.rgbaSha256 || !same(crop.sourceRect, frozen.sourceRect) || !same(crop.atlas, frozen.atlas)) fail('WHOLE_SOURCE_CROP')
    }
  }
  return [...reasons]
}

/** Hashing is pure in-memory WebCrypto; IO remains with the validation owner. */
export async function verifyArtifacts(proof: DensityProvenance, bytes: ReadonlyMap<string, Uint8Array>): Promise<readonly string[]> {
  const failures: string[] = []
  const pins = [...proof.baselinePins, ...proof.sourceInputs, ...proof.outputs, proof.recipe]
  const outputNames = new Set(proof.outputs.map(pin => pin.path))
  const prefix = 'frontend/public/world/map-density-derived-v1/'
  const expected = ['layout.json', ...['revenge','parade','danger'].flatMap(era => [era+'-scene.png', era+'-scene.svg'])]
  if (proof.outputs.length !== 7 || expected.some(name => !outputNames.has(prefix+name))) failures.push('OUTPUT_SET')
  for (const pin of pins) {
    const raw = bytes.get(pin.path)
    if (!raw || raw.byteLength !== pin.bytes) { failures.push('BYTES:'+pin.path); continue }
    const hash = await crypto.subtle.digest('SHA-256', Uint8Array.from(raw))
    const digest = 'sha256:'+Array.from(new Uint8Array(hash), n => n.toString(16).padStart(2, '0')).join('')
    if (digest !== pin.sha256) failures.push('SHA256:'+pin.path)
  }
  return failures
}
