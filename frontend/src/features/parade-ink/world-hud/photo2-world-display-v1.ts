/** Versioned display preparation. No registration, asset admission or runtime owner.
 * Native coordinates remain authoritative. Display units are Photo2 scene pixels,
 * not a mandated output texture size. Every spatial consumer must use one mapping.
 */
export interface Point { readonly x: number; readonly y: number }
export interface Rect extends Point { readonly width: number; readonly height: number }
export type Era = 'REVENGE' | 'PARADE' | 'DANGER'
export const PHOTO2_VERSION = 'photo2-world-display-v1' as const
export const PHOTO2_REFERENCE = Object.freeze({
  sha256: '9e7f17a666e1ecfc738a14b75bb3a166235a05c401f206aa24f26cf7463cbc27',
  sourceWidth: 1280, sourceHeight: 853,
  scene: Object.freeze({ x: 441, y: 173, width: 384, height: 504 }),
  // Inclusive visual landmarks measured from the supplied source. These are
  // art review regions, never server coordinates or new collision rectangles.
  regions: Object.freeze({
    stage: Object.freeze({ x: 0, y: 4, width: 211, height: 211 }),
    merch: Object.freeze({ x: 275, y: 42, width: 109, height: 141 }),
    meet: Object.freeze({ x: 7, y: 358, width: 85, height: 83 }),
    wall: Object.freeze({ x: 237, y: 382, width: 88, height: 76 }),
  }),
})
export const PHOTO2_NATIVE = Object.freeze({
  mapId: 'mcr_reference_scene_r3', revision: 4,
  geometryHash: 'sha256:899aca9bd6ce499c6f46aff5eaf21e30ab34b301e988771b978a6fab7c2d469f',
  inputSha256: '847d9c92a4ddfa22b8187bb6afe9a9fc2e6e09b1c7fe8e5f85c5a7f5c0d846a9',
  width: 768, height: 1296, tileWidth: 16, tileHeight: 16,
  // Current protected camera readset; this is not a future texture-size mandate.
  referenceCamera: Object.freeze({ x: 160, y: 13, width: 584, height: 584 }),
  spawns: Object.freeze({ courtyard_entry: Object.freeze({ x: 29, y: 26 }), community_entry: Object.freeze({ x: 31, y: 47 }) }),
})
export const PHOTO2_NATIVE_ANCHORS = Object.freeze((
[
  {
    "id": "main_stage",
    "x": 160,
    "y": 37,
    "width": 300,
    "height": 336,
    "depth_y": 360
  },
  {
    "id": "north_white_food",
    "x": 665,
    "y": 31,
    "width": 100,
    "height": 120,
    "depth_y": 150
  },
  {
    "id": "west_white_merch",
    "x": 160,
    "y": 381,
    "width": 132,
    "height": 154,
    "depth_y": 530
  },
  {
    "id": "culture_park_plaque",
    "x": 585,
    "y": 319,
    "width": 156,
    "height": 96,
    "depth_y": 411
  },
  {
    "id": "cross_banner_east",
    "x": 522,
    "y": 141,
    "width": 50,
    "height": 134,
    "depth_y": 271
  },
  {
    "id": "meeting_pavilion",
    "x": 256,
    "y": 779,
    "width": 236,
    "height": 206,
    "depth_y": 985
  },
  {
    "id": "giveaway_pavilion",
    "x": 440,
    "y": 1044,
    "width": 236,
    "height": 206,
    "depth_y": 1250
  },
  {
    "id": "rooted_tree_01",
    "x": 146,
    "y": -22,
    "width": 166,
    "height": 159,
    "depth_y": 137
  },
  {
    "id": "rooted_tree_02",
    "x": 328,
    "y": -25,
    "width": 166,
    "height": 162,
    "depth_y": 137
  },
  {
    "id": "rooted_tree_03",
    "x": 558,
    "y": -28,
    "width": 166,
    "height": 165,
    "depth_y": 137
  },
  {
    "id": "rooted_tree_04",
    "x": 2,
    "y": 220,
    "width": 166,
    "height": 162,
    "depth_y": 382
  },
  {
    "id": "rooted_tree_05",
    "x": 634,
    "y": 220,
    "width": 134,
    "height": 134,
    "depth_y": 354
  },
  {
    "id": "rooted_tree_06",
    "x": 640,
    "y": 455,
    "width": 128,
    "height": 125,
    "depth_y": 580
  },
  {
    "id": "rooted_tree_07",
    "x": 5,
    "y": 535,
    "width": 166,
    "height": 162,
    "depth_y": 697
  },
  {
    "id": "rooted_tree_08",
    "x": 40,
    "y": 983,
    "width": 166,
    "height": 165,
    "depth_y": 1148
  },
  {
    "id": "rooted_tree_09",
    "x": 594,
    "y": 809,
    "width": 166,
    "height": 144,
    "depth_y": 953
  },
  {
    "id": "rooted_tree_10",
    "x": 8,
    "y": 1137,
    "width": 166,
    "height": 144,
    "depth_y": 1281
  },
  {
    "id": "rooted_tree_11",
    "x": 410,
    "y": 455,
    "width": 166,
    "height": 162,
    "depth_y": 617
  },
  {
    "id": "food_counters",
    "x": 568,
    "y": 611,
    "width": 190,
    "height": 164,
    "depth_y": 775
  },
  {
    "id": "northern_hall",
    "x": 12,
    "y": -94,
    "width": 248,
    "height": 190,
    "depth_y": 96
  },
  {
    "id": "north_eastern_hall",
    "x": 472,
    "y": -94,
    "width": 248,
    "height": 190,
    "depth_y": 96
  },
  {
    "id": "merch_venue_hall",
    "x": 8,
    "y": 551,
    "width": 212,
    "height": 180,
    "depth_y": 731
  }
] as const).map(anchor => Object.freeze(anchor)))
export type NativeAnchorId = typeof PHOTO2_NATIVE_ANCHORS[number]['id']

/** Rectangular topological grid, one diagonal per cell. Boundary is a rectangle;
 * positive triangle orientation gives a continuous, globally invertible PL map.
 * No independent actor, art, hit-test, collision or affordance offset is allowed.
 */
export interface DisplayMesh {
  readonly columns: readonly number[]
  readonly rows: readonly number[]
  readonly display: readonly Point[] // Row-major; source vertex is columns[x],rows[y].
}
export interface Projection {
  readonly forward: (native: Point) => Point | null
  readonly inverse: (display: Point) => Point | null
}
const finite = (p: Point) => Number.isFinite(p.x) && Number.isFinite(p.y)
const cross = (a: Point, b: Point, c: Point) => (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
type Triangle = readonly [Point, Point, Point]
function weights(p: Point, t: Triangle): readonly [number, number, number] | null {
  const area = cross(t[0], t[1], t[2])
  const u = cross(p, t[1], t[2]) / area
  const v = cross(t[0], p, t[2]) / area
  const w = 1 - u - v
  return Math.min(u, v, w) >= -1e-9 ? [u, v, w] : null
}
function interpolate(t: Triangle, w: readonly [number, number, number]): Point {
  return { x: t[0].x * w[0] + t[1].x * w[1] + t[2].x * w[2],
    y: t[0].y * w[0] + t[1].y * w[1] + t[2].y * w[2] }
}
function increasing(values: readonly number[]) {
  return values.length >= 2 && values.length <= 17 && values.every((v, i) => Number.isFinite(v) && (!i || v > values[i - 1]!))
}
export function createPhoto2Projection(mesh: DisplayMesh): Projection {
  const { columns: xs, rows: ys } = mesh
  if (!increasing(xs) || !increasing(ys) || mesh.display.length !== xs.length * ys.length
    || xs[0] !== -128 || ys[0] !== -128 || xs.at(-1) !== 896 || ys.at(-1) !== 1424) throw new Error('Invalid Photo2 native mesh coverage')
  // Copy coordinates: later caller mutation cannot silently alter an admitted map.
  const points = mesh.display.map(p => ({ x: p.x, y: p.y }))
  if (!points.every(p => finite(p) && Math.abs(p.x) <= 1048576 && Math.abs(p.y) <= 1048576)) throw new Error('Invalid Photo2 display vertex')
  const at = (x: number, y: number) => points[y * xs.length + x]!
  const left = at(0, 0).x, right = at(xs.length - 1, 0).x
  const top = at(0, 0).y, bottom = at(0, ys.length - 1).y
  if (!(right > left && bottom > top)) throw new Error('Invalid Photo2 boundary')
  // Exact collinearity plus increasing boundary positions prevents a crossing or
  // multiply-covered boundary. Positive interior triangles then forbid folds.
  for (let x = 0; x < xs.length; x++) {
    if (at(x, 0).y !== top || at(x, ys.length - 1).y !== bottom
      || (x > 0 && (at(x, 0).x <= at(x - 1, 0).x || at(x, ys.length - 1).x <= at(x - 1, ys.length - 1).x))) throw new Error('Invalid Photo2 horizontal boundary')
  }
  for (let y = 0; y < ys.length; y++) {
    if (at(0, y).x !== left || at(xs.length - 1, y).x !== right
      || (y > 0 && (at(0, y).y <= at(0, y - 1).y || at(xs.length - 1, y).y <= at(xs.length - 1, y - 1).y))) throw new Error('Invalid Photo2 vertical boundary')
  }
  const triangles: { native: Triangle; display: Triangle }[] = []
  for (let y = 0; y < ys.length - 1; y++) for (let x = 0; x < xs.length - 1; x++) {
    const a = { x: xs[x]!, y: ys[y]! }, b = { x: xs[x + 1]!, y: ys[y]! }
    const c = { x: xs[x + 1]!, y: ys[y + 1]! }, d = { x: xs[x]!, y: ys[y + 1]! }
    const pairs: { native: Triangle; display: Triangle }[] = [
      { native: [a, b, c], display: [at(x, y), at(x + 1, y), at(x + 1, y + 1)] },
      { native: [a, c, d], display: [at(x, y), at(x + 1, y + 1), at(x, y + 1)] },
    ]
    for (const pair of pairs) {
      if (!Number.isFinite(cross(...pair.display)) || cross(...pair.display) <= 1e-6) throw new Error('Folded or degenerate Photo2 display triangle')
      triangles.push(pair)
    }
  }
  function project(p: Point, source: 'native' | 'display', target: 'native' | 'display') {
    if (!finite(p)) return null
    for (const triangle of triangles) {
      const w = weights(p, triangle[source])
      if (w) return interpolate(triangle[target], w)
    }
    return null // No clamping, extrapolation or silent physical-coordinate fallback.
  }
  return Object.freeze({ forward: (p: Point) => project(p, 'native', 'display'),
    inverse: (p: Point) => project(p, 'display', 'native') })
}

/** Concrete initial mapping: protected native camera rectangle -> source scene.
 * It is useful for contract/runtime integration, but is NOT Photo2 art readiness.
 * A later reviewed non-affine mesh needs a new descriptor hash and qualification.
 */
export const PHOTO2_INITIAL_MESH: DisplayMesh = Object.freeze({
  columns: Object.freeze([-128, 160, 310, 472, 665, 896]),
  rows: Object.freeze([-128, 137, 360, 424, 530, 985, 1424]),
  display: Object.freeze([-128, 137, 360, 424, 530, 985, 1424].flatMap(y =>
    [-128, 160, 310, 472, 665, 896].map(x => Object.freeze({ x: (x - 160) * 384 / 584, y: (y - 13) * 504 / 584 })))),
})

export interface ShortcutPermission {
  readonly available: boolean
  readonly disabled: boolean
  readonly pending: boolean
}
/** Use the exact current authorized destination state, never derive permission
 * from proximity or room membership. Rendering must use ShellLink/navigation
 * memory, native keyboard focus, a readable name and >=44 CSS-pixel hit bounds.
 */
export function photo2Shortcut(board: 'MEET' | 'WALL', permission: ShortcutPermission | null) {
  const enabled = permission?.available === true && permission.disabled === false && permission.pending === false
  return Object.freeze({ board, label: board === 'MEET' ? 'Open Meet' : 'Open Wall',
    href: enabled ? board === 'MEET' ? '/meet' : '/wall' : null,
    disabled: !enabled, minimumHitCssPixels: 44, interactionId: null,
    physicalArrival: false, proximityRequired: false })
}

export interface DisplayAsset {
  readonly path: string
  readonly sha256: string
  readonly bytes: number
  readonly width: number
  readonly height: number
}
export interface DisplayBinding {
  readonly version: typeof PHOTO2_VERSION
  readonly revision: number
  readonly era: Era
  readonly geometryHash: string
  readonly nativeInputSha256: string
  readonly referenceSha256: string
  readonly descriptorSha256: string
  readonly themeRevision: number
  readonly themeHash: string
  readonly mesh: DisplayMesh
  readonly anchors: readonly { readonly id: string; readonly x: number; readonly y: number; readonly width: number; readonly height: number; readonly depth_y: number }[]
  readonly assets: readonly DisplayAsset[]
  readonly frameContractSha256: string
}
const digest = (s: string) => /^sha256:[a-f0-9]{64}$/.test(s) && !/^sha256:(.)\1{63}$/.test(s)
const prefix = 'world/reference-scene-v2/photo2-world-display-v1/'
/** Admission is a pure comparison against a separately reviewed descriptor pin.
 * Digest computation, C03/catalog verification and loading belong to the existing
 * entry/asset owners. This module never treats a self-reported digest as proof.
 */
export function admitPhoto2Display(binding: DisplayBinding, expected: {
  readonly era: Era; readonly revision: number; readonly descriptorSha256: string
  readonly themeRevision: number; readonly themeHash: string
  readonly frameContractSha256: string; readonly mesh: DisplayMesh; readonly assets: readonly DisplayAsset[]
  readonly loaded: readonly DisplayAsset[]; readonly loading: boolean
}): Projection | null {
  if (expected.loading || binding.version !== PHOTO2_VERSION || binding.revision !== expected.revision || !Number.isSafeInteger(binding.revision) || binding.revision < 1
    || binding.era !== expected.era || binding.geometryHash !== PHOTO2_NATIVE.geometryHash
    || binding.nativeInputSha256 !== PHOTO2_NATIVE.inputSha256 || binding.referenceSha256 !== PHOTO2_REFERENCE.sha256
    || !digest(binding.descriptorSha256) || binding.descriptorSha256 !== expected.descriptorSha256
    || !digest(binding.themeHash) || binding.themeHash !== expected.themeHash || binding.themeRevision !== expected.themeRevision
    || !Number.isInteger(binding.themeRevision) || binding.themeRevision < 2
    || !digest(binding.frameContractSha256) || binding.frameContractSha256 !== expected.frameContractSha256
    || binding.anchors.length !== PHOTO2_NATIVE_ANCHORS.length || binding.assets.length < 1 || binding.assets.length > 4) return null
  if (!PHOTO2_NATIVE_ANCHORS.every(anchor => {
    const matches = binding.anchors.filter(candidate => candidate.id === anchor.id)
    return matches.length === 1 && ['x', 'y', 'width', 'height', 'depth_y'].every(key =>
      matches[0]![key as 'x'] === anchor[key as 'x'])
  })) return null
  let transferred = 0, pixels = 0
  const paths = new Set<string>()
  const valid = (asset: DisplayAsset) => {
    if (!asset.path.startsWith(prefix) || !/^[a-z0-9/_-]+\.png$/.test(asset.path)
      || asset.path.includes('..') || paths.has(asset.path) || !digest(asset.sha256)
      || !Number.isSafeInteger(asset.bytes) || asset.bytes <= 0
      || !Number.isInteger(asset.width) || !Number.isInteger(asset.height)
      || asset.width <= 0 || asset.height <= 0 || asset.width > 2048 || asset.height > 2048) return false
    paths.add(asset.path); transferred += asset.bytes; pixels += asset.width * asset.height
    return true
  }
  if (!binding.assets.every(valid) || transferred > 2097152 || pixels > 2097152) return null
  const sameAssets = (other: readonly DisplayAsset[]) => other.length === binding.assets.length && binding.assets.every(asset => {
    const matches = other.filter(candidate => candidate.path === asset.path)
    return matches.length === 1 && matches[0]!.sha256 === asset.sha256 && matches[0]!.bytes === asset.bytes
      && matches[0]!.width === asset.width && matches[0]!.height === asset.height
  })
  if (!sameAssets(expected.assets) || !sameAssets(expected.loaded)) return null
  if (binding.mesh.columns.length !== expected.mesh.columns.length || binding.mesh.rows.length !== expected.mesh.rows.length
    || !binding.mesh.columns.every((v, i) => v === expected.mesh.columns[i])
    || !binding.mesh.rows.every((v, i) => v === expected.mesh.rows[i])
    || binding.mesh.display.length !== expected.mesh.display.length
    || !binding.mesh.display.every((p, i) => p.x === expected.mesh.display[i]!.x && p.y === expected.mesh.display[i]!.y)) return null
  try { return createPhoto2Projection(binding.mesh) } catch { return null }
}
