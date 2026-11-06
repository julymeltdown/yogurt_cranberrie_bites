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
/** Display-side source pins for the qualified paint revision 3 producer contract.
 * These are literal source comparisons only; they admit no pixels and no release.
 */
export const PHOTO2_DISPLAY = Object.freeze({
  groundBounds: Object.freeze({ x: -112, y: -20, width: 499, height: 1200 }),
  // Bounding box of the protected native camera rectangle through the pinned mesh.
  camera: Object.freeze({ x: 0, y: -11.612903225806452, width: 361.11, height: 545.8594785682723 }),
})
/** Qualified ART tuple mirrored from the release catalog. Registry admission stays closed.
 * Renderer source and release pins must describe the same bytes before any future release.
 */
export const PHOTO2_QUALIFIED = Object.freeze({
  paintRevision: 3,
  artCommit: 'a8027217ec9bc5b33a9219b74726ea707f33e0cc',
  producerContract: Object.freeze({ path: 'docs/mcr/evidence/pi08-finish-01/photo2-world-contract/producer-contract-v3.json',
    sha256: 'cc12d028b4615246ea457722d13af6d35fc505e44b6d1946adcf1811a37ab98e', bytes: 39962 }),
  candidate: Object.freeze({ path: 'docs/mcr/evidence/pi08-finish-01/photo2-world-contract/producer-input-v3.json',
    sha256: '4a41acdb1f331c668e2a9b017d1627d0aa970258aa2c5abbae893b2baa9e1f0d', bytes: 38938 }),
  interface: Object.freeze({ path: 'world/reference-scene-v2/photo2-world-display-v1/parade/asset-interface.json',
    sha256: 'sha256:dd2d09f2aebe3c631fcd0eed8cc09e11df8661550a38fa214ee50e6c39b81600', bytes: 46552 }),
  provenance: Object.freeze({ path: 'world/reference-scene-v2/photo2-world-display-v1/parade/source-provenance.json',
    sha256: 'sha256:8cebea0e8f5adcb9fa479b01936e89e65fa15a1a625f5afc47bd0a898e9ed465', bytes: 3469 }),
  ground: Object.freeze({ path: 'world/reference-scene-v2/photo2-world-display-v1/parade/ground.png',
    sha256: 'sha256:5d4c6c56186088fd86de836dc0c66a9aff7df1cf373615f070e2a63b9272d9c0', bytes: 61501, width: 499, height: 1200 }),
  atlas: Object.freeze({ path: 'world/reference-scene-v2/photo2-world-display-v1/parade/atlas.png',
    sha256: 'sha256:cb4eb5dab44b925dd71bec7a5821d9e607d74f4078679b2bb982644a7b32fb8a', bytes: 27372, width: 512, height: 1024 }),
  qualificationSha256: 'sha256:7ef35cbbbff44ae3fe0a0681f984f611fb1d9f744d2432145ff0ec245164894f',
  sceneryHash: 'sha256:d714d971dc45ea2d191552db9ccdad0c8a62441f3962b161301686c9584aeef6',
  themeHash: 'sha256:146d1fdc816052d3a78ef7b8c90d7cb1a6eeaff29c7dec9a1be458c0b377508f',
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

/** Exact reviewed producer mesh (paint revision 3) as source pins, not admission.
 * Pixel/alpha/focus clearance and catalog release remain with the literal registry.
 */
const PHOTO2_SOURCE_DISPLAY_ROWS = [
  [[-180, -128], [-112, -128], [0, -128], [36, -128], [105.5, -128], [211, -128], [270, -128], [275, -128], [329.5, -128], [384, -128], [387, -128], [480, -128]],
  [[-180, -20], [-112, -20], [0, -20], [36, -20], [105.5, -20], [211, -20], [270, -4.7560975609756095], [275, 5], [329.5, 5], [384, 5], [387, 5], [480, 5]],
  [[-180, 0], [-112, 0], [0, 0], [36, 0], [105.5, 0], [211, 0], [270, 25.609756097560975], [275, 42], [329.5, 42], [384, 42], [387, 42], [480, 42]],
  [[-180, 4], [-112, 4], [0, 4], [36, 4], [105.5, 4], [211, 4], [270, 31.439024390243905], [275, 49], [329.5, 49], [384, 49], [387, 49], [480, 49]],
  [[-180, 81.3157894736842], [-112, 81.3157894736842], [0, 81.3157894736842], [36, 81.3157894736842], [105.5, 81.3157894736842], [211, 81.3157894736842], [270, 143.3183568677792], [275, 183], [329.5, 183], [384, 183], [387, 183], [480, 183]],
  [[-180, 82], [-112, 82], [0, 82], [36, 82], [105.5, 82], [211, 82], [270, 144.1951219512195], [275, 184], [329.5, 184], [384, 184], [387, 184], [480, 184]],
  [[-180, 180], [-112, 180], [0, 180], [36, 180], [105.5, 180], [211, 180], [270, 217.8048780487805], [275, 242], [329.5, 242], [384, 242], [387, 242], [480, 242]],
  [[-180, 190], [-112, 190], [0, 190], [36, 190], [105.5, 190], [211, 190], [270, 226.58536585365854], [275, 250], [329.5, 250], [384, 250], [387, 250], [480, 250]],
  [[-180, 215], [-112, 215], [0, 215], [36, 215], [105.5, 215], [211, 215], [270, 246.70731707317074], [275, 267], [329.5, 267], [384, 267], [387, 267], [480, 267]],
  [[-180, 223], [-112, 223], [0, 223], [36, 223], [105.5, 223], [211, 223], [270, 254.70731707317074], [275, 275], [329.5, 275], [384, 275], [387, 275], [480, 275]],
  [[-180, 300], [-112, 300], [0, 300], [36, 300], [105.5, 300], [211, 300], [270, 300], [275, 300], [329.5, 300], [384, 300], [387, 300], [480, 300]],
  [[-180, 335], [-112, 335], [0, 335], [36, 335], [105.5, 335], [211, 335], [270, 344.1463414634146], [275, 350], [329.5, 350], [384, 350], [387, 350], [480, 350]],
  [[-180, 344], [-112, 344], [0, 344], [36, 344], [105.5, 344], [211, 344], [270, 353.7560975609756], [275, 360], [329.5, 360], [384, 360], [387, 360], [480, 360]],
  [[-180, 600], [-112, 600], [0, 600], [36, 600], [105.5, 600], [211, 600], [270, 600], [275, 600], [329.5, 600], [384, 600], [387, 600], [480, 600]],
  [[-180, 860], [-112, 860], [0, 860], [36, 860], [105.5, 860], [211, 860], [270, 860], [275, 860], [329.5, 860], [384, 860], [387, 860], [480, 860]],
  [[-180, 1180], [-112, 1180], [0, 1180], [36, 1180], [105.5, 1180], [211, 1180], [270, 1180], [275, 1180], [329.5, 1180], [384, 1180], [387, 1180], [480, 1180]],
  [[-180, 1308], [-112, 1308], [0, 1308], [36, 1308], [105.5, 1308], [211, 1308], [270, 1308], [275, 1308], [329.5, 1308], [384, 1308], [387, 1308], [480, 1308]],
] as const
export const PHOTO2_SOURCE_MESH: DisplayMesh = Object.freeze({
  columns: Object.freeze([-128, 0, 160, 226, 310, 460, 585, 665, 715, 765, 768, 896]),
  rows: Object.freeze([-128, 0, 31, 37, 150, 151, 319, 336, 360, 373, 424, 530, 544, 617, 985, 1296, 1424]),
  display: Object.freeze(PHOTO2_SOURCE_DISPLAY_ROWS.flat().map(([x, y]) => Object.freeze({ x, y }))),
})

/** Cover the admitted camera rectangle exactly: no letterbox gutters and no art
 * beyond the qualified camera frame. The caller still owns local zoom and look-ahead.
 */
export function photo2CameraPresentation(camera: Rect, viewport: { readonly width: number; readonly height: number }, zoom: number) {
  if (![camera.x, camera.y, camera.width, camera.height, viewport.width, viewport.height].every(Number.isFinite)
    || camera.width <= 0 || camera.height <= 0 || viewport.width <= 0 || viewport.height <= 0
    || !Number.isFinite(zoom) || zoom < 1 || zoom > 8) throw new Error('Invalid Photo2 camera viewport')
  return Object.freeze({ zoom: zoom * Math.max(viewport.width / camera.width, viewport.height / camera.height),
    center: Object.freeze({ x: camera.x + camera.width / 2, y: camera.y + camera.height / 2 }) })
}

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
