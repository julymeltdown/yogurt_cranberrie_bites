import assert from 'node:assert/strict'
import { JSDOM } from 'jsdom'
import { validateTiled, THEME_IMAGE_MARKER } from '/Users/lhs/Desktop/mcr-worktrees/qa-fe-theme/frontend/src/world/map/tiled.ts'
import { assertSameWorld } from '/Users/lhs/Desktop/mcr-worktrees/qa-fe-theme/frontend/src/world/theme/compatibility.ts'
import { createLocalThemePreferences } from '/Users/lhs/Desktop/mcr-worktrees/qa-fe-theme/frontend/src/features/theme/model/preferences.ts'
import { createWorldRuntime } from '/Users/lhs/Desktop/mcr-worktrees/qa-fe-theme/frontend/src/world/runtime.ts'
import { MapError } from '/Users/lhs/Desktop/mcr-worktrees/qa-fe-theme/frontend/src/world/map/json.ts'

const dom = new JSDOM('<!DOCTYPE html><html><body></body></html>', { url: 'http://localhost' })
globalThis.window = dom.window
globalThis.document = dom.window.document
globalThis.HTMLDivElement = dom.window.HTMLDivElement
globalThis.HTMLImageElement = dom.window.HTMLImageElement
globalThis.Image = dom.window.Image
globalThis.URL = dom.window.URL
globalThis.Blob = dom.window.Blob

const results = {}

// 1. Tiled Validator Checks
const geometry = {
  tile_width_px: 32,
  tile_height_px: 32,
  width_cells: 5,
  height_cells: 5,
  collision_id: 'col_1',
  collision_grid: [[0]],
  terrain_layer: [[1]],
  overdraw_layer: [[0]],
  spawns: [],
  interactions: [],
  default_spawn_id: 's_1'
}

const texture = {
  image_state: 'PROVIDED',
  path: 'world/revenge.png',
  sha256: 'sha256:0000000000000000000000000000000000000000000000000000000000000000',
  width_px: 96,
  height_px: 32
}

function makeTiled(imagePath, extra = {}) {
  return JSON.stringify({
    compressionlevel: -1,
    height: 5,
    infinite: false,
    layers: [
      { data: [1], height: 5, id: 1, name: 'terrain', opacity: 1, type: 'tilelayer', visible: true, width: 5, x: 0, y: 0 },
      { data: [0], height: 5, id: 2, name: 'overdraw', opacity: 1, type: 'tilelayer', visible: true, width: 5, x: 0, y: 0 }
    ],
    nextlayerid: 3,
    nextobjectid: 1,
    orientation: 'orthogonal',
    renderorder: 'right-down',
    tiledversion: '1.10.2',
    tileheight: 32,
    tilewidth: 32,
    type: 'map',
    version: '1.10',
    width: 5,
    tilesets: [
      {
        columns: 3,
        firstgid: 1,
        image: imagePath,
        imageheight: 32,
        imagewidth: 96,
        margin: 0,
        name: 'theme',
        spacing: 0,
        tilecount: 3,
        tileheight: 32,
        tilewidth: 32,
        ...extra
      }
    ]
  })
}

// Marker accepts
validateTiled(makeTiled(THEME_IMAGE_MARKER), geometry, texture)
// Exact texture path accepts
validateTiled(makeTiled(texture.path), geometry, texture)

// Other paths reject
const invalidPaths = [
  'world/parade.png',
  '../evil.png',
  'http://evil.com/theme.png',
  'data:image/png;base64,xxxx',
  '',
  'mcr-theme.jpg',
  'world/mcr-theme.png'
]

for (const p of invalidPaths) {
  assert.throws(() => validateTiled(makeTiled(p), geometry, texture), /Map/i, `Should have rejected image path: ${p}`)
}

// Altered dimensions / count reject
assert.throws(() => validateTiled(makeTiled(THEME_IMAGE_MARKER, { columns: 4 }), geometry, texture))
assert.throws(() => validateTiled(makeTiled(THEME_IMAGE_MARKER, { tilecount: 4 }), geometry, texture))
assert.throws(() => validateTiled(makeTiled(THEME_IMAGE_MARKER, { firstgid: 2 }), geometry, texture))

results.tiledValidator = 'PASS'

// 2. assertSameWorld checks
const baseMap = {
  contract_version: '1.0.0',
  kind: 'map_manifest',
  map_id: 'map_fixture',
  map_revision: 1,
  content_hash: 'sha256:map1',
  geometry_hash: 'sha256:geo1',
  tilemap: { format: 'TILED_JSON', resource_state: 'PROVIDED', path: 'world/map.json', sha256: 'sha256:tiled1' },
  geometry: { width_cells: 5, height_cells: 5, tile_width_px: 32, tile_height_px: 32 },
  places: [{ place_id: '00000000-0000-4000-8000-000000000001', label: 'Place' }]
}

const baseTheme = {
  theme_id: 'REVENGE',
  avatar_manifest_id: 'avatar_1',
  avatar_manifest_version: 1,
  avatar_content_hash: 'sha256:avatar1'
}

const current = {
  map: baseMap,
  theme: baseTheme,
  widthPx: 160,
  heightPx: 160,
  spawn: { x: 1, y: 1 }
}

// Same map + avatar -> passes
assertSameWorld(current, { ...current, theme: { ...baseTheme, theme_id: 'PARADE' } })

// Altered map content_hash -> throws
assert.throws(() => assertSameWorld(current, {
  ...current,
  map: { ...baseMap, content_hash: 'sha256:map2' }
}))

// Altered map tilemap path -> throws
assert.throws(() => assertSameWorld(current, {
  ...current,
  map: { ...baseMap, tilemap: { ...baseMap.tilemap, path: 'world/parade_map.json' } }
}))

// Altered avatar manifest id / version / hash -> throws
assert.throws(() => assertSameWorld(current, {
  ...current,
  theme: { ...baseTheme, avatar_manifest_id: 'avatar_2' }
}))
assert.throws(() => assertSameWorld(current, {
  ...current,
  theme: { ...baseTheme, avatar_content_hash: 'sha256:avatar2' }
}))

results.assertSameWorld = 'PASS'

// 3. Preference Scoping Checks
const storageMap = new Map()
const fakeStorage = {
  getItem: k => storageMap.get(k) ?? null,
  setItem: (k, v) => storageMap.set(k, v)
}
const prefs = createLocalThemePreferences(fakeStorage)

prefs.write({ userId: 'alice', eventId: 'concert:1' }, 'PARADE')
assert.equal(prefs.read({ userId: 'alice', eventId: 'concert:1' }), 'PARADE')
// Delimiter escaping: 'alice:concert' + '1' must not collide with 'alice' + 'concert:1'
assert.equal(prefs.read({ userId: 'alice:concert', eventId: '1' }), null)

// Empty userId or eventId throws
assert.throws(() => prefs.read({ userId: '', eventId: 'e' }))
assert.throws(() => prefs.read({ userId: 'u', eventId: '' }))

results.preferenceScoping = 'PASS'

// 4. Atomic Runtime ReplaceSkin Tests
let initialDisposed = false
let candidate1Disposed = false
let candidate2Disposed = false

const mockMapInitial = {
  map: baseMap,
  theme: baseTheme,
  widthPx: 160,
  heightPx: 160,
  spawn: { x: 1, y: 1 },
  image: { naturalWidth: 96, naturalHeight: 32 },
  dispose: () => { initialDisposed = true }
}

const mockMapCand1 = {
  map: baseMap,
  theme: { ...baseTheme, theme_id: 'PARADE' },
  widthPx: 160,
  heightPx: 160,
  spawn: { x: 1, y: 1 },
  image: { naturalWidth: 96, naturalHeight: 32 },
  dispose: () => { candidate1Disposed = true }
}

const mockMapCand2 = {
  map: baseMap,
  theme: { ...baseTheme, theme_id: 'DANGER' },
  widthPx: 160,
  heightPx: 160,
  spawn: { x: 1, y: 1 },
  image: { naturalWidth: 96, naturalHeight: 32 },
  dispose: () => { candidate2Disposed = true }
}

let activeSkinInEngine = 'REVENGE'
let engineReplaced = false

const host = document.createElement('div')
const runtime = createWorldRuntime({
  host,
  load: async () => mockMapInitial,
  importEngine: async () => () => ({
    ready: Promise.resolve(),
    closed: new Promise(() => {}),
    resize: () => {},
    setVisible: () => {},
    destroy: () => {},
    replaceSkin: map => {
      engineReplaced = true
      activeSkinInEngine = map.theme.theme_id
    }
  }),
  timeoutMs: 500
})

await runtime.start()
assert.equal(runtime.readSkin(), 'REVENGE')

// A. Supersession test: slow load superseded by fast load
let resolveSlow
const slowPromise = new Promise(r => { resolveSlow = r })

const p1 = runtime.replaceSkin(() => slowPromise)
const p2 = runtime.replaceSkin(async () => mockMapCand2)

await p2
assert.equal(runtime.readSkin(), 'DANGER')
assert.equal(activeSkinInEngine, 'DANGER')
assert.equal(initialDisposed, true)

// Now resolve slow candidate late
resolveSlow(mockMapCand1)
await assert.rejects(p1, /cancelled/)
assert.equal(candidate1Disposed, true, 'Superseded candidate must be disposed')
assert.equal(runtime.readSkin(), 'DANGER', 'Skin remains DANGER')

// B. Rollback test when engine throws in replaceSkin
const host2 = document.createElement('div')
let init2Disposed = false
const mockInit2 = { ...mockMapInitial, dispose: () => { init2Disposed = true } }
const runtime2 = createWorldRuntime({
  host: host2,
  load: async () => mockInit2,
  importEngine: async () => () => ({
    ready: Promise.resolve(),
    closed: new Promise(() => {}),
    resize: () => {},
    setVisible: () => {},
    destroy: () => {},
    replaceSkin: () => { throw new Error('Phaser Texture Upload Failed') }
  })
})
await runtime2.start()

let cand4Disposed = false
const mockCand4 = { ...mockMapCand1, dispose: () => { cand4Disposed = true } }
await assert.rejects(runtime2.replaceSkin(async () => mockCand4), /Phaser Texture Upload Failed/)

assert.equal(runtime2.readSkin(), 'REVENGE', 'Active skin must remain REVENGE on rollback')
assert.equal(cand4Disposed, true, 'Failed candidate must be disposed')
assert.equal(init2Disposed, false, 'Previous skin must not be disposed on rollback')

results.atomicRollbackAndSupersession = 'PASS'

import { writeFileSync } from 'node:fs'
writeFileSync('/tmp/mcr-qa-fe06-agy8u3i5/contract-and-atomic-result.json', JSON.stringify(results, null, 2))
console.log(JSON.stringify(results, null, 2))

