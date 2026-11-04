// @vitest-environment jsdom
import { webcrypto } from 'node:crypto'
import { spawnSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { canonicalJson, contentHash, digest, MapError, sha256, strictJson } from '../map/json'
import { validateManifests, type ReleasePin } from '../map/manifest'
import { validDateTime, validUri, validUuid } from '../map/formats'
import { schemaValid } from '../map/schema'
import { abortable, createAssetResources, loadMap, pngDimensions, type AssetResources, type LoadedMap } from '../map/resources'
import { validateTiled } from '../map/tiled'
import { createWorldRuntime, type ControlContext, type EngineFactory, type RuntimeOptions, type WorldRuntime } from '../runtime'
import { rehash, synthetic } from './fixtures/synthetic'
import type { AvatarScenePort } from '../avatar/scene'
function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(r => { resolve = r }); return { promise, resolve } }
const runtimes: WorldRuntime[] = []
let frames: Map<number, FrameRequestCallback>, frameId = 0, observers: Set<FakeObserver>
class FakeObserver {
  constructor(readonly callback: ResizeObserverCallback) { observers.add(this) }
  observe() {} unobserve() {} disconnect() { observers.delete(this) }
}
beforeEach(() => {
  frames = new Map(); observers = new Set()
  vi.stubGlobal('crypto', webcrypto); vi.stubGlobal('ResizeObserver', FakeObserver)
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => { frames.set(++frameId, callback); return frameId })
  vi.stubGlobal('cancelAnimationFrame', (id: number) => { frames.delete(id) })
})
afterEach(async () => {
  for (const runtime of runtimes.splice(0)) { runtime.dispose(); await runtime.settled() }
  document.body.replaceChildren(); vi.unstubAllGlobals(); vi.restoreAllMocks()
})
const json = (path: string) => readFileSync(`../contracts/fixtures/assets/${path}.json`, 'utf8')
const pinFrom = (map: Record<string, unknown>, theme: Record<string, unknown>): ReleasePin => ({ mapHash: map.content_hash as string, themeHash: theme.content_hash as string,
  avatar: { manifest_id: theme.avatar_manifest_id as string, manifest_version: theme.avatar_manifest_version as number, content_hash: theme.avatar_content_hash as string } })
it.each(['theme-revenge', 'theme-parade', 'theme-danger'])('checks exact current C06 %s hashes with unavailable resources', async name => {
  const map = strictJson(json('map-manifest')) as Record<string, unknown>, theme = strictJson(json(name)) as Record<string, unknown>
  const pin = pinFrom(map, theme), meta = await validateManifests(json('map-manifest'), json(name), pin)
  expect(meta.widthPx).toBe(160); expect(meta.spawn.cell_x).toBe(1)
  const read = vi.fn()
  await expect(loadMap({ mapText: json('map-manifest'), themeText: json(name), pin, resources: { read } }, new AbortController().signal)).rejects.toMatchObject({ reason: 'unavailable' })
  expect(read).not.toHaveBeenCalled()
})
it.each(['{"a":1,"a":2}', '{"a":1,"\\u0061":2}', '1.0', '1e0', '-2E3', 'NaN', 'Infinity', '[1,]', '{"a":1,}', '9007199254740992', '{} trailing'])('rejects lossy or malformed JSON %s', text => expect(() => strictJson(text)).toThrow(MapError))
it.each(['null', 'true', '-0', '123', '"a\\nb"', '[0,{"a":false}]'])('accepts bounded integer JSON %s', text => expect(canonicalJson(strictJson(text))).toBe(canonicalJson(JSON.parse(text))))
it('matches Python canonical bytes and hashes including Unicode codepoint key ordering and fixture corpus', async () => {
  const corpus = [strictJson(json('map-manifest')), strictJson(json('theme-revenge')), strictJson(json('theme-parade')), strictJson(json('theme-danger')),
    { '\u{10000}': '\u{1f3b8}', '\ue000': 'é\u007f\n', '한글': [false, null, -4, '\ud800'], plain: '"\\/' }]
  const program = "import runpy,sys,json; m=runpy.run_path('../scripts/mcr/validate-assets.py'); value=json.loads(sys.stdin.read(),object_pairs_hook=m['reject_duplicates']); sys.stdout.buffer.write(m['canonical_bytes'](value))"
  for (const value of corpus) {
    const result = spawnSync(process.env.MCR_FE14_PYTHON ?? 'python3', ['-c', program], { input: JSON.stringify(value), encoding: 'utf8' })
    expect(result.status, result.stderr).toBe(0); expect(canonicalJson(value)).toBe(result.stdout)
    expect(await digest(value)).toBe(await sha256(new TextEncoder().encode(result.stdout)))
  }
})
it.each(['1.0', '1e2', '{"a":1,"a":2}'])('agrees with owning Python rejection of %s', text => {
  const result = spawnSync(process.env.MCR_FE14_PYTHON ?? 'python3', ['-c', "import runpy,json,sys;m=runpy.run_path('../scripts/mcr/validate-assets.py');m['canonical_bytes'](json.loads(sys.stdin.read(),object_pairs_hook=m['reject_duplicates']))"], { input: text })
  expect(result.status).not.toBe(0); expect(() => strictJson(text)).toThrow()
})
it.each(['unknown', 'version', 'grid-shape', 'blocked-spawn', 'outside-spawn', 'duplicate-spawn', 'unknown-default', 'unreachable', 'unknown-place', 'duplicate-place', 'physical-fixture', 'tile-overflow', 'texture-alignment', 'texture-size', 'map-dimensions', 'theme-place', 'theme-interaction', 'theme-avatar', 'theme-overrides', 'provenance-review'])('rejects rehashed semantic or schema failure %s', async mutation => {
  const f = await synthetic(), { map, theme } = f, g = map.geometry
  if (mutation === 'unknown') Object.assign(map, { surprise: 1 })
  if (mutation === 'version') map.contract_version = '2.0.0'
  if (mutation === 'grid-shape') g.collision_grid.pop()
  if (mutation === 'blocked-spawn') g.spawns[0]!.cell_x = 0
  if (mutation === 'outside-spawn') g.spawns[0]!.cell_x = 100
  if (mutation === 'duplicate-spawn') g.spawns.push({ ...g.spawns[0]! })
  if (mutation === 'unknown-default') g.default_spawn_id = 'unknown'
  if (mutation === 'unreachable') { g.collision_grid[1]![2] = 1; g.collision_grid[2]![1] = 1 }
  if (mutation === 'unknown-place') g.interactions[0]!.place_id = '00000000-0000-4000-8000-000000000099'
  if (mutation === 'duplicate-place') map.places.push({ ...map.places[0]! })
  if (mutation === 'physical-fixture') { map.places[0]!.place_kind = 'PHYSICAL'; map.places[0]!.verification_state = 'VERIFIED' }
  if (mutation === 'tile-overflow') g.terrain_layer[1]![1] = 4
  if (mutation === 'texture-alignment') theme.texture.width_px = 97
  if (mutation === 'texture-size') theme.texture.width_px = 4096
  if (mutation === 'map-dimensions') g.width_cells = 129
  if (mutation === 'theme-place') theme.place_ids = []
  if (mutation === 'theme-interaction') theme.interaction_ids = ['unknown']
  if (mutation === 'theme-overrides') Object.assign(theme, { collision_grid: [] })
  if (mutation === 'provenance-review') theme.provenance.review_state = 'REVIEWED'
  const pin = await rehash(map, theme)
  if (mutation === 'theme-avatar') pin.avatar.content_hash = `sha256:${'0'.repeat(64)}`
  await expect(validateManifests(JSON.stringify(map), JSON.stringify(theme), pin)).rejects.toMatchObject({ reason: 'invalid' })
})
it('rejects stale self hash, stale geometry hash and an unapproved but self-consistent release', async () => {
  const f = await synthetic(), original = { ...f.pin }
  f.map.geometry.terrain_layer[1]![1] = 2
  await expect(validateManifests(JSON.stringify(f.map), JSON.stringify(f.theme), f.pin)).rejects.toThrow()
  f.map.content_hash = await contentHash(f.map); f.theme.map_content_hash = f.map.content_hash; f.theme.content_hash = await contentHash(f.theme)
  await expect(validateManifests(JSON.stringify(f.map), JSON.stringify(f.theme), pinFrom(f.map, f.theme))).rejects.toThrow()
  const fresh = await rehash(f.map, f.theme)
  await expect(validateManifests(JSON.stringify(f.map), JSON.stringify(f.theme), original)).rejects.toThrow()
  expect((await validateManifests(JSON.stringify(f.map), JSON.stringify(f.theme), fresh)).map.geometry.terrain_layer[1]![1]).toBe(2)
})
it.each(['external', 'embedded-image', 'tileset-resource', 'compressed', 'flipped-gid', 'grid-mismatch', 'infinite', 'offset', 'layer-duplicate', 'extra-layer', 'wrong-image', 'wrong-size', 'isometric'])('rejects unsupported/mismatched Tiled %s', async mutation => {
  const f = await synthetic(), t = f.tiled
  if (mutation === 'external') Object.assign(t.tilesets[0]!, { source: 'remote.tsj' })
  if (mutation === 'embedded-image') t.tilesets[0]!.image = 'data:image/png;base64,unreviewed'
  if (mutation === 'tileset-resource') Object.assign(t.tilesets[0]!, { tiles: [{ image: 'extra.png' }] })
  if (mutation === 'compressed') Object.assign(t.layers[0]!, { encoding: 'base64', compression: 'zlib' })
  if (mutation === 'flipped-gid') t.layers[0]!.data[0] = 2147483649
  if (mutation === 'grid-mismatch') t.layers[0]!.data[1] = 1
  if (mutation === 'infinite') t.infinite = true
  if (mutation === 'offset') t.layers[0]!.x = 1
  if (mutation === 'layer-duplicate') t.layers[1]!.name = 'terrain'
  if (mutation === 'extra-layer') t.layers.push({ ...t.layers[0]! })
  if (mutation === 'wrong-image') t.tilesets[0]!.image = 'world/other.png'
  if (mutation === 'wrong-size') t.width = 6
  if (mutation === 'isometric') t.orientation = 'isometric'
  expect(() => validateTiled(JSON.stringify(t), f.map.geometry, f.theme.texture)).toThrow()
})
it('accepts exact finite Tiled geometry and does not reinterpret collision or foot coordinates', async () => {
  const f = await synthetic(); expect(() => validateTiled(f.tiledText, f.map.geometry, f.theme.texture)).not.toThrow()
  expect(schemaValid(f.map, 'map')).toBe(true); expect(schemaValid(f.theme, 'theme')).toBe(true)
})
function response(bytes: Uint8Array<ArrayBuffer>, url: string, type = 'application/json', headers: Record<string, string> = {}) {
  const response = new Response(bytes, { headers: { 'Content-Type': type, ...headers } }); Object.defineProperty(response, 'url', { value: url }); return response
}
it('loads only allowlisted immutable same-origin bytes with no credentials, redirects, private API or implicit resources', async () => {
  const f = await synthetic(), fetch = vi.fn<typeof globalThis.fetch>(async (input, init) => {
    expect(init).toMatchObject({ credentials: 'omit', mode: 'same-origin', cache: 'force-cache', redirect: 'error', referrerPolicy: 'no-referrer' })
    return response(f.tiledBytes, String(input))
  })
  const resources = createAssetResources({ origin: 'https://mcr.example', allow: f.allow, fetch })
  expect(Array.from(await resources.read(f.allow[0]!, new AbortController().signal))).toEqual(Array.from(f.tiledBytes))
  await expect(resources.read({ ...f.allow[0]!, path: 'api/auth/refresh' }, new AbortController().signal)).rejects.toThrow()
  await expect(resources.read({ ...f.allow[0]!, sha256: `sha256:${'0'.repeat(64)}` }, new AbortController().signal)).rejects.toThrow()
  expect(fetch).toHaveBeenCalledOnce()
})
it.each(['../map.json', 'world/a/../map.json', '/world/map.json', 'world//map.json', 'https://elsewhere/map.json', 'world/%2e%2e/map.json', 'api/private.json', 'world/a.json?token=x'])('refuses unsafe resource path %s at composition', async path => {
  const f = await synthetic()
  expect(() => createAssetResources({ origin: 'https://mcr.example', allow: [{ ...f.allow[0]!, path }] })).toThrow()
})
it.each(['hash', 'url', 'type', 'size', 'status'])('rejects actual resource %s mismatch', async mode => {
  const f = await synthetic(), fetch = vi.fn<typeof globalThis.fetch>(async input => {
    const url = String(input)
    if (mode === 'status') return new Response(null, { status: 503 })
    return response(mode === 'hash' ? new Uint8Array([1]) : f.tiledBytes, mode === 'url' ? 'https://elsewhere/' : url,
      mode === 'type' ? 'text/html' : 'application/json', mode === 'size' ? { 'Content-Length': '99999999' } : {})
  })
  const resources = createAssetResources({ origin: 'https://mcr.example', allow: f.allow, fetch })
  await expect(resources.read(f.allow[0]!, new AbortController().signal)).rejects.toThrow(); expect(fetch).toHaveBeenCalledOnce()
})
it('validates PNG header, actual decoded dimensions and cleans a wrong-size decoder result', async () => {
  const f = await synthetic(), dispose = vi.fn()
  expect(() => pngDimensions(f.png, 96, 32)).not.toThrow(); expect(() => pngDimensions(f.png, 32, 32)).toThrow()
  const resources: AssetResources = { read: async r => r.kind === 'png' ? f.png : f.tiledBytes }
  await expect(loadMap({ mapText: JSON.stringify(f.map), themeText: JSON.stringify(f.theme), pin: f.pin, resources,
    decode: async () => ({ image: { naturalWidth: 1, naturalHeight: 1 } as HTMLImageElement, dispose }) }, new AbortController().signal)).rejects.toThrow()
  expect(dispose).toHaveBeenCalledOnce()
})
async function loaded() {
  const f = await synthetic(), dispose = vi.fn()
  const map = await loadMap({ mapText: JSON.stringify(f.map), themeText: JSON.stringify(f.theme), pin: f.pin,
    resources: { read: async r => r.kind === 'png' ? f.png : f.tiledBytes }, decode: async () => ({ image: { naturalWidth: 96, naturalHeight: 32 } as HTMLImageElement, dispose }) }, new AbortController().signal)
  return { map, dispose }
}
function owner(load: (signal: AbortSignal) => Promise<LoadedMap>, factory?: EngineFactory, createOverview?: RuntimeOptions['createOverview']) {
  const host = document.createElement('div'); document.body.append(host)
  const stats = { created: 0, destroyed: 0, active: 0, peak: 0, sizes: [] as number[][], visible: [] as boolean[] }
  const engine: EngineFactory = (host) => {
    stats.created++; stats.peak = Math.max(stats.peak, ++stats.active)
    const canvas = document.createElement('canvas'); host.append(canvas)
    const closed = deferred<void>(); let stopped = false
    return { ready: Promise.resolve(), closed: closed.promise, resize: (w, h) => stats.sizes.push([w, h]), setVisible: v => stats.visible.push(v),
      destroy() { if (stopped) return; stopped = true; stats.destroyed++; stats.active--; canvas.remove(); closed.resolve() } }
  }
  const importer = vi.fn(async () => factory ?? engine)
  const runtime = createWorldRuntime({ host, load, importEngine: importer, timeoutMs: 60, ...(createOverview ? { createOverview } : {}) }); runtimes.push(runtime)
  return { runtime, host, stats, engine, importer }
}
it('owns one custom overview, receives only ready map leases and withdraws before engine destruction', async () => {
  const data = await loaded(), next = await loaded(), maps: (LoadedMap | null)[] = []
  const overview = { read: () => null, subscribe: () => () => {}, replace: vi.fn((map: LoadedMap | null) => { maps.push(map) }), dispose: vi.fn() }
  const createOverview = vi.fn(() => overview), replaceSkin = vi.fn(), destroy = vi.fn(() => expect(maps.at(-1)).toBeNull())
  const f = owner(async () => data.map, () => ({ ready: Promise.resolve(), closed: Promise.resolve(), resize() {}, setVisible() {}, destroy, replaceSkin }), createOverview)
  expect(createOverview).toHaveBeenCalledOnce(); expect(maps).toEqual([])
  await f.runtime.start(); expect(maps).toEqual([data.map]); expect(f.runtime.overview.read()).toBeNull()
  await f.runtime.replaceSkin(async () => next.map)
  expect(replaceSkin).toHaveBeenCalledWith(next.map); expect(maps).toEqual([data.map, next.map]); expect(data.dispose).toHaveBeenCalledOnce()
  f.runtime.stop(); await f.runtime.settled(); expect(maps.at(-1)).toBeNull(); expect(destroy).toHaveBeenCalledOnce()
  expect(overview.dispose).not.toHaveBeenCalled(); expect(next.dispose).toHaveBeenCalledOnce()
  f.runtime.dispose(); f.runtime.dispose(); expect(overview.dispose).toHaveBeenCalledOnce(); expect(createOverview).toHaveBeenCalledOnce()
})
it('still releases the engine and decoded map when an injected projection fails during withdrawal', async () => {
  const data = await loaded(), projection = { read: () => null, subscribe: () => () => {}, replace() { throw Error('Projection failure') }, dispose() { throw Error('Projection teardown failure') } }
  const f = owner(async () => data.map, undefined, () => projection)
  expect((await f.runtime.start()).phase).toBe('ready')
  f.runtime.dispose(); await f.runtime.settled()
  expect(f.stats.destroyed).toBe(1); expect(data.dispose).toHaveBeenCalledOnce(); expect(f.runtime.read().phase).toBe('disposed')
})
it('reserves one start, renders geometry and releases canvas/observer/frame/image on repeated stop/dispose', async () => {
  const { map, dispose } = await loaded(), { runtime, stats } = owner(async () => map)
  const a = runtime.start(), b = runtime.start(); expect(a).toBe(b)
  expect(await a).toMatchObject({ phase: 'ready', widthPx: 160, heightPx: 160, spawnFoot: { x: 48, y: 48 } })
  expect(stats.created).toBe(1); expect(document.querySelectorAll('canvas')).toHaveLength(1); expect(observers.size).toBe(1)
  runtime.setVisible(false); expect(stats.visible.at(-1)).toBe(false)
  runtime.stop(); runtime.stop(); await runtime.settled()
  expect(document.querySelectorAll('canvas')).toHaveLength(0); expect(observers.size).toBe(0); expect(frames.size).toBe(0); expect(dispose).toHaveBeenCalledOnce()
  runtime.dispose(); runtime.dispose(); expect((await runtime.start()).phase).toBe('disposed')
})
it('forwards the existing engine avatar scene capability without constructing another owner', async () => {
  const { map } = await loaded(), host = document.createElement('div'), attachment = { setEnabled: vi.fn(), dispose: vi.fn() }
  document.body.append(host)
  const avatarScene: AvatarScenePort = { texture: vi.fn(), sprite: vi.fn(), onFrame: vi.fn(), read: () => ({ textures: 0, sprites: 0, listeners: 0 }) }
  const camera = { surface: () => null, center() {}, project: () => null, unproject: () => null, read: () => null }
  const factory = vi.fn(() => ({ ready: Promise.resolve(), closed: Promise.resolve(), resize() {}, setVisible() {}, destroy() {}, camera, avatarScene }))
  const controls = vi.fn((context: ControlContext) => { expect(context.avatarScene).toBe(avatarScene); return attachment })
  const runtime = createWorldRuntime({ host, load: async () => map, importEngine: async () => factory, attachControls: controls })
  runtimes.push(runtime); expect((await runtime.start()).phase).toBe('ready')
  expect(factory).toHaveBeenCalledOnce(); expect(controls).toHaveBeenCalledOnce()
  runtime.dispose(); expect(attachment.dispose).toHaveBeenCalledOnce()
})
it.each(['stop', 'dispose', 'abort'] as const)('never loads or imports after synchronous %s during start reservation', async action => {
  const load = vi.fn(async () => (await loaded()).map), { runtime, importer } = owner(load), controller = new AbortController()
  const pending = runtime.start(controller.signal)
  if (action === 'stop') runtime.stop(); else if (action === 'dispose') runtime.dispose(); else controller.abort()
  await pending; expect(load).not.toHaveBeenCalled(); expect(importer).not.toHaveBeenCalled(); expect(document.querySelector('canvas')).toBeNull()
})
it('honors reentrant loading observer disposal before any resource dispatch', async () => {
  const load = vi.fn(async () => (await loaded()).map), { runtime } = owner(load)
  runtime.subscribe(() => { if (runtime.read().phase === 'loading') runtime.dispose() })
  await runtime.start(); expect(load).not.toHaveBeenCalled(); expect(runtime.read().phase).toBe('disposed')
})
it('cleans a slow signal-ignoring resource completion after stop without importing Phaser', async () => {
  const late = deferred<LoadedMap>(), load = vi.fn(() => late.promise), { runtime, importer } = owner(load), result = runtime.start()
  await vi.waitFor(() => expect(load).toHaveBeenCalledOnce())
  runtime.stop(); const data = await loaded(); late.resolve(data.map); await result; await Promise.resolve()
  expect(data.dispose).toHaveBeenCalledOnce(); expect(importer).not.toHaveBeenCalled()
})
it('fences a slow module import and releases its map before any late game construction', async () => {
  const data = await loaded(), late = deferred<EngineFactory>(), factory = vi.fn<EngineFactory>(), host = document.createElement('div')
  const importer = vi.fn(() => late.promise)
  const runtime = createWorldRuntime({ host, load: async () => data.map, importEngine: importer }); runtimes.push(runtime)
  const pending = runtime.start(); await vi.waitFor(() => expect(importer).toHaveBeenCalledOnce())
  runtime.stop(); late.resolve(factory); await pending
  expect(factory).not.toHaveBeenCalled(); expect(data.dispose).toHaveBeenCalledOnce()
})
it('rejects another owner on a claimed host without replacing existing content or engine', async () => {
  const data = await loaded(), { runtime, host } = owner(async () => data.map); await runtime.start()
  const load = vi.fn(async () => data.map), other = createWorldRuntime({ host, load }); runtimes.push(other)
  expect(await other.start()).toMatchObject({ phase: 'error', reason: 'host-busy' }); expect(load).not.toHaveBeenCalled()
  expect(document.querySelectorAll('canvas')).toHaveLength(1)
})
it('retains the replacement host reservation across the previous engine drain', async () => {
  const a = await loaded(), b = await loaded(), queue = [a.map, b.map], { runtime, host, stats } = owner(async () => queue.shift()!)
  await runtime.start(); runtime.stop(); const next = runtime.start(); await next
  const other = createWorldRuntime({ host, load: async () => b.map }); runtimes.push(other)
  expect((await other.start()).reason).toBe('host-busy'); expect(stats.peak).toBe(1); expect(stats.created).toBe(2)
})
it('cleans an engine that synchronously stops its owner during allocation', async () => {
  const data = await loaded(); let runtime!: WorldRuntime
  const destroy = vi.fn(), close = deferred<void>()
  const factory: EngineFactory = host => { host.append(document.createElement('canvas')); runtime.stop(); return { ready: Promise.resolve(), closed: close.promise, resize() {}, setVisible() {}, destroy } }
  const owned = owner(async () => data.map, factory); runtime = owned.runtime
  await runtime.start(); expect(destroy).toHaveBeenCalledOnce(); expect(document.querySelector('canvas')).toBeNull(); expect(data.dispose).not.toHaveBeenCalled()
  close.resolve(); await runtime.settled(); expect(data.dispose).toHaveBeenCalledOnce()
})
it('reports unavailable/invalid visibly and only retries after an explicit start', async () => {
  let fails = true; const data = await loaded(), load = vi.fn(async () => { if (fails) throw new MapError('unavailable'); return data.map })
  const { runtime, stats } = owner(load)
  expect((await runtime.start()).phase).toBe('unavailable'); expect(stats.created).toBe(0)
  await Promise.resolve(); expect(load).toHaveBeenCalledOnce(); fails = false
  expect((await runtime.start()).phase).toBe('ready'); expect(load).toHaveBeenCalledTimes(2)
})
it('bounds a never-settling injected resource without creating an engine', async () => {
  const { runtime, importer } = owner(() => new Promise(() => {}))
  expect(await runtime.start()).toMatchObject({ phase: 'error', reason: 'timeout' }); expect(importer).not.toHaveBeenCalled()
})
it('bounds an independently aborted promise and ignores late completion', async () => {
  const controller = new AbortController(), late = deferred<number>(), pending = abortable(late.promise, controller.signal, 20)
  controller.abort(); await expect(pending).rejects.toMatchObject({ reason: 'cancelled' }); late.resolve(1)
})

it.each(['aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa', '11111111-1111-0111-1111-111111111111'])('matches C06 positive UUID/provenance semantics with consistent references %s', async placeId => {
  const f = await synthetic()
  f.map.places[0]!.place_id = placeId; f.map.geometry.interactions[0]!.place_id = placeId; f.theme.place_ids = [placeId]
  Object.assign(f.theme.provenance, { review_state: 'REVIEWED', reviewed_by: 'Synthetic review fixture', reviewed_at: '2026-11-03t00:00:00z', source_uri: 'urn:mcr:synthetic:review' })
  const pin = await rehash(f.map, f.theme)
  expect((await validateManifests(JSON.stringify(f.map), JSON.stringify(f.theme), pin)).theme.provenance.review_state).toBe('REVIEWED')
  const program = "import runpy,sys,json;m=runpy.run_path('../scripts/mcr/validate-assets.py');d=json.loads(sys.stdin.read(),object_pairs_hook=m['reject_duplicates']);checks=m['validators']();m['validate_document'](d['map'],checks,'map');m['validate_document'](d['theme'],checks,'theme');m['validate_map'](d['map'])"
  const result = spawnSync(process.env.MCR_FE14_PYTHON ?? 'python3', ['-c', program], { input: JSON.stringify({ map: f.map, theme: f.theme }), encoding: 'utf8' })
  expect(result.status, result.stderr).toBe(0)
})
it.each(['2026-11-03t00:00:00z', '2024-02-29T23:59:59+09:00', '2026-11-03T00:00:00.123456789Z'])('accepts RFC3339 timestamp %s', value => expect(validDateTime(value)).toBe(true))
it.each(['2025-02-29T00:00:00Z', '2026-11-03T24:00:00Z', '2026-11-03T00:00:00+25:00', 'not-a-time', '2026-11-03T00:00:60Z', '2016-12-31T23:59:60Z', '2026-11-03T00:00:00Z\n'])('rejects timestamps outside the pinned C06 checker %s', value => expect(validDateTime(value)).toBe(false))
it.each(['urn:test:x', 'https:', 'https://x/a%20b?x=y#z', 'mailto:a@b', 'file:///tmp/example', 'http://[::1]/', 'x://[v1.ab:cd]/'])('accepts RFC3986 URI %s', value => expect(validUri(value)).toBe(true))
it.each(['not-absolute', 'https://x/a b', 'https://x/%ZZ', 'https://例え.テスト/', 'http://[nonsense]/', 'https://a:bad/', 'https://a/x#y#z', 'https://x/\n'])('rejects malformed URI %s', value => expect(validUri(value)).toBe(false))

// Parent's format-prerequisite venv is opt-in; never silently use missing optional checkers.
it.runIf(Boolean(process.env.MCR_FE14_PYTHON))('differentially rejects fully rehashed invalid provenance with registered pinned Python formats', async () => {
  const cases = [
    { reviewed_at: '2026-11-03T00:00:60Z' }, { reviewed_at: '2016-12-31T23:59:60Z' },
    { reviewed_at: '2025-02-29T00:00:00Z' }, { reviewed_at: 'not-a-time' },
    { source_uri: 'https://x/a b' }, { source_uri: 'https://x/%ZZ' }, { source_uri: 'not-absolute' },
  ]
  for (const change of cases) {
    const f = await synthetic()
    Object.assign(f.theme.provenance, { review_state: 'REVIEWED', reviewed_by: 'Synthetic review fixture', reviewed_at: '2026-11-03t00:00:00z', source_uri: 'urn:mcr:synthetic:review' }, change)
    const pin = await rehash(f.map, f.theme)
    await expect(validateManifests(JSON.stringify(f.map), JSON.stringify(f.theme), pin)).rejects.toMatchObject({ reason: 'invalid' })
    const program = "import json,sys,runpy;from jsonschema import FormatChecker;assert all(f in FormatChecker.checkers for f in ['uri','date-time','uuid']);m=runpy.run_path('../scripts/mcr/validate-assets.py');d=json.loads(sys.stdin.read());m['validate_document'](d,m['validators'](),'theme')"
    const result = spawnSync(process.env.MCR_FE14_PYTHON!, ['-c', program], { input: JSON.stringify(f.theme), encoding: 'utf8' })
    expect(result.status, result.stderr).not.toBe(0)
    expect(result.stderr).not.toContain('AssertionError')
    expect(result.stderr).toContain('AssetError: theme: schema')
  }
})

it('rejects trailing-newline UUID even when regex dollar would match before it', () => expect(validUuid('aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa\n')).toBe(false))

it.each(['stop', 'dispose', 'abort'] as const)('keeps inherited deferred shutdown exclusive after replacement %s', async action => {
  const old = await loaded(), next = await loaded(), close = deferred<void>(), latestClose = deferred<void>()
  const host = document.createElement('div'); document.body.append(host)
  let live = 0, peak = 0
  const oldFactory = vi.fn<EngineFactory>(() => {
    peak = Math.max(peak, ++live)
    return { ready: Promise.resolve(), closed: close.promise, resize() {}, setVisible() {}, destroy() {} }
  })
  const loadA = vi.fn(async () => old.map)
  const a = createWorldRuntime({ host, load: loadA, importEngine: async () => oldFactory }); runtimes.push(a)
  const loadB = vi.fn(async () => next.map), factoryB = vi.fn<EngineFactory>(() => {
    peak = Math.max(peak, ++live)
    return { ready: Promise.resolve(), closed: latestClose.promise, resize() {}, setVisible() {}, destroy() { live--; latestClose.resolve() } }
  })
  const b = createWorldRuntime({ host, load: loadB, importEngine: async () => factoryB }); runtimes.push(b)
  expect((await a.start()).phase).toBe('ready'); a.stop()
  const controller = new AbortController(), replacement = a.start(controller.signal)
  if (action === 'dispose') a.dispose(); else if (action === 'abort') controller.abort(); else a.stop()
  await replacement; await Promise.resolve()
  expect((await b.start()).reason).toBe('host-busy')
  expect(loadA).toHaveBeenCalledOnce(); expect(loadB).not.toHaveBeenCalled(); expect(factoryB).not.toHaveBeenCalled()
  expect(live).toBe(1); expect(old.dispose).not.toHaveBeenCalled()
  live--; close.resolve(); await a.settled()
  expect(old.dispose).toHaveBeenCalledOnce()
  expect((await b.start()).phase).toBe('ready'); expect(peak).toBe(1)
  // Old already-settled disposal and obsolete claim releases cannot evict the new owner.
  a.dispose(); await a.settled()
  const loadC = vi.fn(async () => next.map), c = createWorldRuntime({ host, load: loadC, importEngine: async () => factoryB }); runtimes.push(c)
  expect((await c.start()).reason).toBe('host-busy'); expect(loadC).not.toHaveBeenCalled()
  b.stop(); await b.settled()
  expect((await c.start()).phase).toBe('ready'); expect(peak).toBe(1)
})
it.runIf(Boolean(process.env.MCR_FE14_PYTHON)).each([
  ['http://[::ffff:192.168.001.1]/', true], ['http://[::ffff:192.168.01.1]/', true],
  ['http://[::ffff:192.168.1.1]/', true], ['x://[v1.ab:cd]/', true], ['x://[V1.ab:cd]/', false],
  ['http://[::]/', true], ['http://[1:2:3:4:5:6:192.168.001.1]/', true],
  ['http://[::ffff:192.168.256.1]/', false], ['http://[1:2:3:4:5:6:7:8::]/', false],
  ['http://[192.168.1.1::]/', false], ['http://[:::1]/', false], ['http://[::1%25zone]/', false],
] as const)('matches pinned owner on fully rehashed IPv-literal provenance %s', async (uri, accepted) => {
  const f = await synthetic()
  Object.assign(f.map.provenance, { source_uri: uri })
  Object.assign(f.theme.provenance, { source_uri: uri })
  const pin = await rehash(f.map, f.theme)
  expect(validUri(uri)).toBe(accepted)
  const client = validateManifests(JSON.stringify(f.map), JSON.stringify(f.theme), pin)
  if (accepted) await expect(client).resolves.toHaveProperty('map.content_hash', pin.mapHash)
  else await expect(client).rejects.toMatchObject({ reason: 'invalid' })
  const program = "import runpy,sys,json;from jsonschema import FormatChecker;assert 'uri' in FormatChecker.checkers;m=runpy.run_path('../scripts/mcr/validate-assets.py');d=json.loads(sys.stdin.read());checks=m['validators']();m['validate_document'](d['map'],checks,'map');m['validate_document'](d['theme'],checks,'theme');m['validate_map'](d['map'])"
  const result = spawnSync(process.env.MCR_FE14_PYTHON!, ['-c', program], { input: JSON.stringify({ map: f.map, theme: f.theme }), encoding: 'utf8' })
  expect(result.status === 0, result.stderr).toBe(accepted)
  if (!accepted) expect(result.stderr).toContain('AssetError:')
})

it.each(['stop', 'dispose', 'restart'] as const)('fences readiness after reentrant setVisible %s', async action => {
  const first = await loaded(), second = await loaded(), maps = [first.map, second.map]
  let runtime!: WorldRuntime, created = 0, destroyed = 0, restart: Promise<unknown> | undefined
  const phases: { phase: string; created: number }[] = []
  const factory: EngineFactory = host => {
    const number = ++created, close = deferred<void>(); let closed = false
    host.append(document.createElement('canvas'))
    return { ready: Promise.resolve(), closed: close.promise, resize() {}, setVisible() {
      if (number !== 1) return
      if (action === 'dispose') runtime.dispose()
      else { runtime.stop(); if (action === 'restart') restart = runtime.start() }
    }, destroy() { if (!closed) { closed = true; destroyed++; close.resolve() } } }
  }
  runtime = owner(async () => maps.shift()!, factory).runtime
  runtime.subscribe(() => phases.push({ phase: runtime.read().phase, created }))
  await runtime.start()
  if (action === 'restart') {
    await restart
    expect(runtime.read().phase).toBe('ready'); expect(created).toBe(2)
    expect(phases.filter(s => s.phase === 'ready')).toEqual([{ phase: 'ready', created: 2 }])
  } else {
    await runtime.settled()
    expect(runtime.read().phase).toBe(action === 'dispose' ? 'disposed' : 'stopped')
    expect(phases.some(s => s.phase === 'ready')).toBe(false)
    expect(document.querySelector('canvas')).toBeNull()
    if (action === 'dispose') expect((await runtime.start()).phase).toBe('disposed')
  }
  expect(first.dispose).toHaveBeenCalledOnce(); expect(destroyed).toBe(1)
})
