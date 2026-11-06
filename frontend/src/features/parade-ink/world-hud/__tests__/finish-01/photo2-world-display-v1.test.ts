import { describe, expect, it } from 'vitest'
import { createPhoto2Projection, PHOTO2_DISPLAY, PHOTO2_NATIVE_ANCHORS, PHOTO2_NATIVE,
  PHOTO2_QUALIFIED, PHOTO2_REFERENCE, PHOTO2_SOURCE_MESH, PHOTO2_VERSION, photo2CameraPresentation,
  photo2Shortcut, admitPhoto2Display, type DisplayBinding, type Rect } from '../../photo2-world-display-v1'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'

// Independent frozen authoritative input; no copied fixture can hide drift.
const frozen = execFileSync('git', ['show', '8dee1335a79f831e6a29f005a50f2c0f65777d46:frontend/src/world/reference-scene/spatial-v3/source-inputs.json'])
const source = JSON.parse(frozen.toString()) as { scenes: { PARADE: { scenery: { objects: Record<string, unknown>[] } } } }
const gitShow = (revision: string, path: string) => execFileSync('git', ['show', `${revision}:${path}`])
const producer = JSON.parse(gitShow('3d256d4216327a59dce96e550e4feb355e55b18a',
  'docs/mcr/evidence/pi08-finish-01/photo2-world-contract/producer-input-v3.json').toString())
const hash = (label: string) => 'sha256:' + createHash('sha256').update(label).digest('hex')
const cloneMesh = (): { columns: number[]; rows: number[]; display: { x: number; y: number }[] } => ({
  columns: [...PHOTO2_SOURCE_MESH.columns], rows: [...PHOTO2_SOURCE_MESH.rows],
  display: PHOTO2_SOURCE_MESH.display.map(p => ({ ...p })),
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
  it('keeps renderer mesh, display frame and released ART pins on the same qualified tuple', () => {
    expect(PHOTO2_REFERENCE.sha256).toBe(producer.referenceSha256)
    expect(PHOTO2_NATIVE).toMatchObject({ inputSha256: producer.nativeSource.sha256, geometryHash: producer.nativeSource.geometryHash,
      width: producer.nativeSource.width, height: producer.nativeSource.height })
    expect(PHOTO2_SOURCE_MESH).toEqual(producer.mesh)
    expect(PHOTO2_DISPLAY.groundBounds).toEqual(producer.ground.displayBounds)
    const release = JSON.parse(gitShow('dbc2b43a4', 'backend/releases/photo2-world-display-v1/qualified-inputs.json').toString())
    const binding = JSON.parse(gitShow('dbc2b43a4', 'backend/releases/photo2-world-display-v1/source-binding.json').toString())
    expect(release.art_commit).toBe(PHOTO2_QUALIFIED.artCommit)
    expect(release.interface.sha256).toBe(PHOTO2_QUALIFIED.interface.sha256)
    expect(release.interface.bytes).toBe(PHOTO2_QUALIFIED.interface.bytes)
    expect(release.provenance.sha256).toBe(PHOTO2_QUALIFIED.provenance.sha256)
    expect(release.qualification_sha256).toBe(PHOTO2_QUALIFIED.qualificationSha256)
    expect(release.scenery_hash).toBe(PHOTO2_QUALIFIED.sceneryHash)
    expect(release.theme_hash).toBe(PHOTO2_QUALIFIED.themeHash)
    const parade = binding.scenes.find((scene: { theme_id: string }) => scene.theme_id === 'PARADE')
    const asset = (path: string) => parade.runtime_resources.find((value: { path: string }) => value.path === path)
    expect(asset(PHOTO2_QUALIFIED.ground.path)).toEqual({ bytes: PHOTO2_QUALIFIED.ground.bytes, height_px: PHOTO2_QUALIFIED.ground.height,
      path: PHOTO2_QUALIFIED.ground.path, sha256: PHOTO2_QUALIFIED.ground.sha256, width_px: PHOTO2_QUALIFIED.ground.width })
    expect(asset(PHOTO2_QUALIFIED.atlas.path)).toEqual({ bytes: PHOTO2_QUALIFIED.atlas.bytes, height_px: PHOTO2_QUALIFIED.atlas.height,
      path: PHOTO2_QUALIFIED.atlas.path, sha256: PHOTO2_QUALIFIED.atlas.sha256, width_px: PHOTO2_QUALIFIED.atlas.width })
    const contract = JSON.parse(gitShow('3d256d421', PHOTO2_QUALIFIED.producerContract.path).toString())
    expect(contract.candidateSHA256).toBe(PHOTO2_QUALIFIED.candidate.sha256)
    expect(contract.paintRevision).toBe(PHOTO2_QUALIFIED.paintRevision)
    expect(createHash('sha256').update(gitShow('3d256d421', PHOTO2_QUALIFIED.candidate.path)).digest('hex')).toBe(PHOTO2_QUALIFIED.candidate.sha256)
    expect(createHash('sha256').update(gitShow('3d256d421', PHOTO2_QUALIFIED.producerContract.path)).digest('hex')).toBe(PHOTO2_QUALIFIED.producerContract.sha256)
  })
  it('maps the protected camera into the pinned display frame without prescribing output textures', () => {
    const p = createPhoto2Projection(PHOTO2_SOURCE_MESH)
    const a = p.forward({ x: 160, y: 13 }), b = p.forward({ x: 744, y: 597 })
    const c = p.forward({ x: 160, y: 597 }), d = p.forward({ x: 744, y: 13 })
    expect(Math.min(a!.x, b!.x, c!.x, d!.x)).toBe(PHOTO2_DISPLAY.camera.x)
    expect(Math.max(a!.y, b!.y, c!.y, d!.y)).toBeCloseTo(PHOTO2_DISPLAY.camera.y + PHOTO2_DISPLAY.camera.height, 9)
    expect(Math.max(a!.x, b!.x, c!.x, d!.x) - Math.min(a!.x, b!.x, c!.x, d!.x)).toBeCloseTo(PHOTO2_DISPLAY.camera.width, 9)
    close(p.inverse({ x: PHOTO2_DISPLAY.camera.x, y: a!.y }), { x: 160, y: 13 })
    close(p.inverse({ x: b!.x, y: b!.y }), { x: 744, y: 597 })
  })
  it('frames the admitted camera without letterbox gutters at every supported width', () => {
    for (const [width, height] of [[390, 593], [360, 583], [430, 731], [390, 373]] as const) {
      const view = photo2CameraPresentation(PHOTO2_DISPLAY.camera, { width, height }, 1)
      expect(view.zoom).toBeCloseTo(Math.max(width / PHOTO2_DISPLAY.camera.width, height / PHOTO2_DISPLAY.camera.height), 12)
      expect(width / view.zoom).toBeLessThanOrEqual(PHOTO2_DISPLAY.camera.width + 1e-9)
      expect(height / view.zoom).toBeLessThanOrEqual(PHOTO2_DISPLAY.camera.height + 1e-9)
      expect(view.center).toEqual({ x: PHOTO2_DISPLAY.camera.x + PHOTO2_DISPLAY.camera.width / 2,
        y: PHOTO2_DISPLAY.camera.y + PHOTO2_DISPLAY.camera.height / 2 })
      expect(photo2CameraPresentation(PHOTO2_DISPLAY.camera, { width, height }, 2).zoom).toBeCloseTo(view.zoom * 2, 12)
    }
    const invalid: readonly [Rect, { readonly width: number; readonly height: number }, number][] = [
      [{ ...PHOTO2_DISPLAY.camera, width: 0 }, { width: 390, height: 593 }, 1],
      [PHOTO2_DISPLAY.camera, { width: 0, height: 593 }, 1], [PHOTO2_DISPLAY.camera, { width: 390, height: 593 }, 0.5],
      [PHOTO2_DISPLAY.camera, { width: 390, height: 593 }, 9]]
    for (const [camera, viewport, zoom] of invalid) {
      expect(() => photo2CameraPresentation(camera, viewport, zoom)).toThrow('Invalid Photo2 camera viewport')
    }
  })
  it('round trips every native anchor corner, actor foot, spawn and mesh seam', () => {
    const p = createPhoto2Projection(PHOTO2_SOURCE_MESH)
    const points = PHOTO2_NATIVE_ANCHORS.flatMap(a => [{ x: a.x, y: a.y }, { x: a.x + a.width, y: a.y + a.height }])
    points.push({ x: 472, y: 424 }, { x: 504, y: 760 })
    for (const y of PHOTO2_SOURCE_MESH.rows) for (const x of PHOTO2_SOURCE_MESH.columns) points.push({ x, y })
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
