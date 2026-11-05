import { describe, expect, it } from 'vitest'
import { createPhoto2Projection, PHOTO2_INITIAL_MESH, PHOTO2_NATIVE_ANCHORS, PHOTO2_NATIVE,
  PHOTO2_REFERENCE, PHOTO2_VERSION, photo2Shortcut, admitPhoto2Display,
  type DisplayBinding } from '../../photo2-world-display-v1'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'

// Independent frozen authoritative input; no copied fixture can hide drift.
const frozen = execFileSync('git', ['show', '8dee1335a79f831e6a29f005a50f2c0f65777d46:frontend/src/world/reference-scene/spatial-v3/source-inputs.json'])
const source = JSON.parse(frozen.toString()) as { scenes: { PARADE: { scenery: { objects: Record<string, unknown>[] } } } }
const hash = (label: string) => 'sha256:' + createHash('sha256').update(label).digest('hex')
const cloneMesh = (): { columns: number[]; rows: number[]; display: { x: number; y: number }[] } => ({
  columns: [...PHOTO2_INITIAL_MESH.columns], rows: [...PHOTO2_INITIAL_MESH.rows],
  display: PHOTO2_INITIAL_MESH.display.map(p => ({ ...p })),
})
const close = (actual: { x: number; y: number } | null, expected: { x: number; y: number }) => {
  expect(actual).not.toBeNull(); expect(actual!.x).toBeCloseTo(expected.x, 7); expect(actual!.y).toBeCloseTo(expected.y, 7)
}
function admission() {
  const assets = [{ path: 'world/reference-scene-v2/photo2-world-display-v1/test-only-ground.png',
    sha256: hash('synthetic pixels only; no product art admitted'), bytes: 100, width: 384, height: 504 }]
  const binding: DisplayBinding = {
    version: PHOTO2_VERSION, revision: 1, era: 'PARADE', geometryHash: PHOTO2_NATIVE.geometryHash,
    nativeInputSha256: PHOTO2_NATIVE.inputSha256, referenceSha256: PHOTO2_REFERENCE.sha256,
    descriptorSha256: hash('test-only descriptor'), themeRevision: 2, themeHash: hash('test-only theme'),
    mesh: cloneMesh(), anchors: PHOTO2_NATIVE_ANCHORS.map(a => ({ ...a })), assets,
    frameContractSha256: hash('test-only frame contract'),
  }
  const expected = { era: binding.era, revision: 1, descriptorSha256: binding.descriptorSha256,
    themeRevision: 2, themeHash: binding.themeHash, frameContractSha256: binding.frameContractSha256,
    mesh: cloneMesh(), assets: assets.map(a => ({ ...a })), loaded: assets.map(a => ({ ...a })), loading: false }
  return { binding, expected }
}
describe('Photo2 authoritative/display contract', () => {
  it('retains exactly22 native identities/coordinates and exact frozen input hash', () => {
    expect(PHOTO2_NATIVE_ANCHORS).toHaveLength(22)
    expect(createHash('sha256').update(frozen).digest('hex')).toBe(PHOTO2_NATIVE.inputSha256)
    expect(PHOTO2_NATIVE_ANCHORS).toEqual(source.scenes.PARADE.scenery.objects.map(o =>
      Object.fromEntries(['id', 'x', 'y', 'width', 'height', 'depth_y'].map(k => [k, o[k]]))))
    expect(Object.isFrozen(PHOTO2_NATIVE_ANCHORS)).toBe(true)
  })
  it('maps protected camera corners to measured source-scene units without prescribing output textures', () => {
    const p = createPhoto2Projection(PHOTO2_INITIAL_MESH)
    close(p.forward({ x: 160, y: 13 }), { x: 0, y: 0 })
    close(p.forward({ x: 744, y: 597 }), { x: 384, y: 504 })
    close(p.inverse({ x: 0, y: 0 }), { x: 160, y: 13 })
    close(p.inverse({ x: 384, y: 504 }), { x: 744, y: 597 })
  })
  it('round trips every native anchor corner, actor foot, spawn and mesh seam', () => {
    const p = createPhoto2Projection(PHOTO2_INITIAL_MESH)
    const points = PHOTO2_NATIVE_ANCHORS.flatMap(a => [{ x: a.x, y: a.y }, { x: a.x + a.width, y: a.y + a.height }])
    points.push({ x: 472, y: 424 }, { x: 504, y: 760 })
    for (const y of PHOTO2_INITIAL_MESH.rows) for (const x of PHOTO2_INITIAL_MESH.columns) points.push({ x, y })
    for (const point of points) close(p.inverse(p.forward(point)!), point)
    expect(p.forward({ x: NaN, y: 0 })).toBeNull()
    expect(p.forward({ x: 10000, y: 0 })).toBeNull()
    expect(p.inverse({ x: 10000, y: 0 })).toBeNull()
  })
  it('supports a non-affine interior mesh and snapshots caller mutations', () => {
    const mesh = cloneMesh(), i = mesh.columns.length * 3 + 3
    mesh.display[i] = { x: mesh.display[i]!.x + 12, y: mesh.display[i]!.y + 5 }
    const p = createPhoto2Projection(mesh), foot = { x: 472, y: 424 }, before = p.forward(foot)!
    close(p.inverse(before), foot)
    mesh.display[i]!.x += 200
    close(p.forward(foot), before)
  })
  it('rejects folds, crossing boundary, missing coverage and degenerate source cells', () => {
    const mutations: ((mesh: ReturnType<typeof cloneMesh>) => void)[] = [
      m => { m.display[0]!.y += 1 }, m => { m.display[m.columns.length * 3 + 3]!.x += 1000 },
      m => { m.columns[0] = 0 }, m => { m.rows[2] = m.rows[1]! },
      m => { m.display[2]!.x = m.display[1]!.x }, m => { m.display[4]!.x = Infinity },
    ]
    for (const mutate of mutations) { const m = cloneMesh(); mutate(m); expect(() => createPhoto2Projection(m)).toThrow() }
  })
  it('admits only fully loaded exact era/native/mesh/art bindings', () => {
    const { binding, expected } = admission()
    expect(admitPhoto2Display(binding, expected)).not.toBeNull()
    expect(admitPhoto2Display(binding, { ...expected, loading: true })).toBeNull()
    expect(admitPhoto2Display(binding, { ...expected, era: 'REVENGE' })).toBeNull()
    expect(admitPhoto2Display(binding, { ...expected, loaded: [] })).toBeNull()
    expect(admitPhoto2Display(binding, { ...expected, themeHash: hash('other era') })).toBeNull()
    expect(admitPhoto2Display(binding, { ...expected, descriptorSha256: hash('other descriptor') })).toBeNull()
    expect(admitPhoto2Display(binding, { ...expected, frameContractSha256: hash('other frame') })).toBeNull()
    expect(admitPhoto2Display({ ...binding, anchors: binding.anchors.slice(1) }, expected)).toBeNull()
    expect(admitPhoto2Display({ ...binding, anchors: binding.anchors.map((a, i) => i ? a : { ...a, x: a.x + 1 }) }, expected)).toBeNull()
    const mesh = cloneMesh(); mesh.display[mesh.columns.length * 3 + 3]!.x += 1
    expect(admitPhoto2Display({ ...binding, mesh }, expected)).toBeNull()
  })
  it('rejects old/map-only paths, relabeled hashes, old revisions and budget violations', () => {
    const { binding, expected } = admission(), asset = binding.assets[0]!
    const altered = [
      { ...asset, path: 'world/map-reference-composition-v1/parade-scene.png' },
      { ...asset, path: 'world/reference-scene-v2/quality-v6/ground.png' },
      { ...asset, path: 'world/reference-scene-v2/photo2-world-display-v1/../ground.png' },
      { ...asset, sha256: hash('changed bytes') }, { ...asset, bytes: 2097153 },
      { ...asset, width: 2049 }, { ...asset, height: 0 },
    ]
    for (const candidate of altered) expect(admitPhoto2Display({ ...binding, assets: [candidate] }, expected)).toBeNull()
    expect(admitPhoto2Display({ ...binding, themeRevision: 1 }, expected)).toBeNull()
    expect(admitPhoto2Display({ ...binding, assets: [asset, asset] }, expected)).toBeNull()
  })
  it('rejects non-finite geometry arithmetic and fractional versions', () => {
    const mesh = cloneMesh()
    mesh.display[mesh.columns.length * 3 + 3] = { x: Number.MAX_VALUE, y: Number.MAX_VALUE }
    expect(() => createPhoto2Projection(mesh)).toThrow()
    mesh.columns[mesh.columns.length - 1] = Number.MAX_VALUE
    expect(() => createPhoto2Projection(mesh)).toThrow()
    const { binding, expected } = admission()
    expect(admitPhoto2Display({ ...binding, revision: 1.5 }, { ...expected, revision: 1.5 })).toBeNull()
    expect(admitPhoto2Display({ ...binding, revision: Infinity }, { ...expected, revision: Infinity })).toBeNull()
  })
  it('copies existing panel authority without inventing physical board interactions', () => {
    for (const board of ['MEET', 'WALL'] as const) {
      const ready = photo2Shortcut(board, { available: true, disabled: false, pending: false })
      expect(ready.href).toBe(board === 'MEET' ? '/meet' : '/wall')
      expect(ready.interactionId).toBeNull(); expect(ready.physicalArrival).toBe(false)
      expect(ready.minimumHitCssPixels).toBe(44)
      for (const state of [null, { available: false, disabled: false, pending: false },
        { available: true, disabled: true, pending: false }, { available: true, disabled: false, pending: true }]) {
        expect(photo2Shortcut(board, state).href).toBeNull()
        expect(photo2Shortcut(board, state).disabled).toBe(true)
      }
    }
  })
})
