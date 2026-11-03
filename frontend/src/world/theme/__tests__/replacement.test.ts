// @vitest-environment jsdom
import { webcrypto } from 'node:crypto'
import { spawnSync } from 'node:child_process'
import avatarManifest from '../../../../../contracts/fixtures/assets/avatar-manifest.json'
import avatarConfig from '../../../../../contracts/fixtures/assets/avatar-config.json'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { MapError, sha256 } from '../../map/json'
import { loadMap, type LoadedMap } from '../../map/resources'
import { createWorldRuntime, type WorldRuntime, type EngineFactory } from '../../runtime'
import { syntheticRelease } from './fixtures/releases'
import { rehash } from '../../__tests__/fixtures/synthetic'
import { validateReplacement } from '../compatibility'
import { THEME_IMAGE_MARKER, validateTiled } from '../../map/tiled'
import type { ThemeId } from '../../../features/theme/model/preferences'
const owners: WorldRuntime[] = []
function deferred<T>() { let resolve!: (v: T) => void; const promise = new Promise<T>(r => { resolve = r }); return { promise, resolve } }
beforeEach(() => {
  vi.stubGlobal('crypto', webcrypto)
  vi.stubGlobal('requestAnimationFrame', () => 1); vi.stubGlobal('cancelAnimationFrame', () => {})
})
afterEach(async () => { for (const owner of owners.splice(0)) { owner.dispose(); await owner.settled() } document.body.replaceChildren(); vi.unstubAllGlobals() })
async function loaded(id: ThemeId) {
  const f = await syntheticRelease(id), dispose = vi.fn()
  const result = await loadMap({ mapText: JSON.stringify(f.map), themeText: JSON.stringify(f.theme), pin: f.pin,
    resources: { read: async r => r.kind === 'png' ? f.png : f.tiledBytes },
    decode: async () => ({ image: { naturalWidth: 96, naturalHeight: 32 } as HTMLImageElement, dispose }) }, new AbortController().signal)
  return { result, dispose, f }
}
async function owner() {
  const initial = await loaded('REVENGE'), replace = vi.fn(), close = deferred<void>(), controlsDispose = vi.fn(), host = document.createElement('div')
  document.body.append(host)
  const camera = { surface: () => null, center() {}, project: () => null, unproject: () => null, read: () => null }
  const factory = vi.fn<EngineFactory>(() => ({ camera, ready: Promise.resolve(), closed: close.promise, resize() {}, setVisible() {}, destroy: () => close.resolve(), replaceSkin: replace }))
  const attach = vi.fn(() => ({ dispose: controlsDispose, setEnabled() {} }))
  const runtime = createWorldRuntime({ host, load: async () => initial.result, importEngine: async () => factory, attachControls: attach, timeoutMs: 30 })
  owners.push(runtime); await runtime.start()
  return { runtime, initial, replace, factory, attach, controlsDispose }
}
it.runIf(Boolean(process.env.MCR_FE14_PYTHON))('uses exactly one immutable map and three textures in a C06-valid bundle', async () => {
  const releases = await Promise.all((['REVENGE', 'PARADE', 'DANGER'] as const).map(syntheticRelease))
  expect(new Set(releases.map(r => JSON.stringify(r.map))).size).toBe(1)
  expect(new Set(releases.map(r => r.theme.texture.sha256)).size).toBe(3)
  const documents = Object.fromEntries(releases.map(r => [r.theme.theme_id, r.theme]))
  Object.assign(documents, { map: releases[0]!.map, avatar: avatarManifest, config: avatarConfig })
  const program = "import runpy,sys,json;m=runpy.run_path('../scripts/mcr/validate-assets.py');m['validate_bundle'](json.loads(sys.stdin.read(),object_pairs_hook=m['reject_duplicates']))"
  const result = spawnSync(process.env.MCR_FE14_PYTHON!, ['-c', program], { input: JSON.stringify(documents), encoding: 'utf8' })
  expect(result.status, result.stderr).toBe(0)
})
it('accepts only the fixed neutral image marker or the exact legacy texture binding', async () => {
  const f = await syntheticRelease('PARADE')
  expect(f.tiled.tilesets[0]!.image).toBe(THEME_IMAGE_MARKER)
  expect(() => validateTiled(f.tiledText, f.map.geometry, f.theme.texture)).not.toThrow()
  f.tiled.tilesets[0]!.image = f.theme.texture.path!
  expect(() => validateTiled(JSON.stringify(f.tiled), f.map.geometry, f.theme.texture)).not.toThrow()
})
it.each(['world/other.png', 'https://example.com/tiles.png', 'data:image/png;base64,AA==', '../mcr-theme.png', './mcr-theme.png', 'mcr-theme.png?x=1'])('rejects rehashed unrecognized Tiled image binding %s before fetching a PNG', async image => {
  const f = await syntheticRelease('PARADE'), decode = vi.fn()
  f.tiled.tilesets[0]!.image = image
  const bytes = new TextEncoder().encode(JSON.stringify(f.tiled))
  f.map.tilemap.sha256 = await sha256(bytes)
  const pin = await rehash(f.map, f.theme), read = vi.fn(async () => bytes)
  await expect(loadMap({ mapText: JSON.stringify(f.map), themeText: JSON.stringify(f.theme), pin, resources: { read }, decode }, new AbortController().signal)).rejects.toMatchObject({ reason: 'invalid' })
  expect(read).toHaveBeenCalledOnce(); expect(decode).not.toHaveBeenCalled()
})
it('replaces the visual lease only; keeps engine, camera, controls and readiness', async () => {
  const o = await owner(), next = await loaded('PARADE'), before = o.runtime.read()
  await o.runtime.replaceSkin(async () => next.result)
  expect(o.runtime.read()).toBe(before); expect(o.runtime.readSkin()).toBe('PARADE')
  expect(o.factory).toHaveBeenCalledOnce(); expect(o.attach).toHaveBeenCalledOnce(); expect(o.controlsDispose).not.toHaveBeenCalled()
  expect(o.replace).toHaveBeenCalledWith(next.result); expect(o.initial.dispose).toHaveBeenCalledOnce(); expect(next.dispose).not.toHaveBeenCalled()
  o.runtime.dispose(); await o.runtime.settled(); expect(next.dispose).toHaveBeenCalledOnce()
})
it.each(['geometry', 'places', 'avatar', 'interaction', 'spawn', 'hash', 'tilemap-binding'] as const)('rejects a fully rehashed incompatible %s skin before commit', async change => {
  const o = await owner(), next = await loaded('DANGER'), map = structuredClone(next.result.map), theme = structuredClone(next.result.theme)
  if (change === 'geometry') map.geometry.collision_grid[0]![0] = 0
  if (change === 'places') map.places[0]!.label = 'Changed place'
  if (change === 'tilemap-binding') map.tilemap.path = 'world/changed.json'
  if (change === 'avatar') theme.avatar_manifest_id = 'other_avatar'
  if (change === 'interaction') map.geometry.interactions[0]!.cell_x = 2
  if (change === 'spawn') map.geometry.spawns[0]!.cell_x = 2
  await rehash(map, theme)
  if (change === 'hash') theme.palette.accent = '#123456'
  await expect(o.runtime.replaceSkin(async () => ({ ...next.result, map, theme }))).rejects.toMatchObject({ reason: 'invalid' })
  expect(o.runtime.readSkin()).toBe('REVENGE'); expect(o.replace).not.toHaveBeenCalled(); expect(o.initial.dispose).not.toHaveBeenCalled(); expect(next.dispose).toHaveBeenCalledOnce()
})
it('rolls back failed preparation without releasing the active resource', async () => {
  const o = await owner(), next = await loaded('DANGER')
  o.replace.mockImplementation(() => { throw new Error('prepare failed') })
  await expect(o.runtime.replaceSkin(async () => next.result)).rejects.toThrow('prepare failed')
  expect(o.runtime.readSkin()).toBe('REVENGE'); expect(next.dispose).toHaveBeenCalledOnce(); expect(o.initial.dispose).not.toHaveBeenCalled()
})
it.each(['stop', 'dispose'] as const)('fences reentrant injected engine %s during commit', async action => {
  const o = await owner(), next = await loaded('DANGER')
  o.replace.mockImplementation(() => o.runtime[action]())
  await expect(o.runtime.replaceSkin(async () => next.result)).rejects.toMatchObject({ reason: 'cancelled' })
  await o.runtime.settled()
  expect(o.runtime.readSkin()).toBeNull(); expect(next.dispose).toHaveBeenCalledOnce(); expect(o.initial.dispose).toHaveBeenCalledOnce()
})
it('fences superseded ignored-abort loads and releases late candidates', async () => {
  const o = await owner(), stale = await loaded('PARADE'), next = await loaded('DANGER'), late = deferred<LoadedMap>()
  const first = o.runtime.replaceSkin(() => late.promise); const rejected = expect(first).rejects.toMatchObject({ reason: 'cancelled' })
  await Promise.resolve(); await o.runtime.replaceSkin(async () => next.result); await rejected
  late.resolve(stale.result); await new Promise(resolve => setTimeout(resolve, 0))
  expect(stale.dispose).toHaveBeenCalledOnce(); expect(o.replace).toHaveBeenCalledOnce(); expect(o.runtime.readSkin()).toBe('DANGER')
})
it.each(['stop', 'dispose', 'abort', 'timeout'] as const)('releases late resources on %s', async action => {
  const o = await owner(), late = deferred<LoadedMap>(), next = await loaded('PARADE'), controller = new AbortController()
  const task = o.runtime.replaceSkin(() => late.promise, controller.signal)
  const rejected = expect(task).rejects.toBeInstanceOf(MapError)
  await Promise.resolve()
  if (action === 'stop') o.runtime.stop(); if (action === 'dispose') o.runtime.dispose(); if (action === 'abort') controller.abort()
  await rejected; late.resolve(next.result); await new Promise(resolve => setTimeout(resolve, 0))
  expect(next.dispose).toHaveBeenCalledOnce(); expect(o.replace).not.toHaveBeenCalled()
})
it('requires provided image resources, even for a valid metadata-only release', async () => {
  const o = await owner(), next = await loaded('PARADE')
  const theme = structuredClone(next.result.theme); theme.texture.image_state = 'NOT_PROVIDED'
  await expect(validateReplacement(o.initial.result, { ...next.result, theme })).rejects.toMatchObject({ reason: 'unavailable' })
})
