import { expect, it, vi } from 'vitest'
import { readFile } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import inputs from '../source-inputs.json'
import { referenceFixture } from './fixture'
import { loadReferenceScene, requireReferenceScene, referenceSceneFor } from '..'
import { contentHash, digest } from '../../map/json'
import { validateManifests } from '../../map/manifest'
import type { AssetResources, DecodedImage } from '../../map/resources'

const base = new URL('../../../../public/', import.meta.url)
const bytes = async (path: string) => new Uint8Array(await readFile(new URL(path, base)))
const hash = (b: Uint8Array) => 'sha256:' + createHash('sha256').update(b).digest('hex')
async function setup() {
  const f = await referenceFixture(), live = new Set<HTMLImageElement>(), read = vi.fn(async (r: Parameters<AssetResources['read']>[0]) => r.kind === 'json' ? f.tiled.slice() : bytes(r.path))
  const decode = vi.fn(async (png: Uint8Array<ArrayBuffer>) => {
    const view = new DataView(png.buffer, png.byteOffset, png.byteLength)
    const image = { naturalWidth: view.getUint32(16), naturalHeight: view.getUint32(20) } as HTMLImageElement; live.add(image)
    return { image, dispose: vi.fn(() => { live.delete(image) }) }
  })
  const input = { mapText: JSON.stringify(f.map), themeText: JSON.stringify(f.theme), pin: f.pin, resources: { read }, decode }
  return { f, live, read, decode, input }
}

it('binds the exact immutable artist metadata and actual packed PNG bytes without changing C06 budgets', async () => {
  for (const [path, sha] of Object.entries(inputs.provenance.inputs)) {
    const file = await bytes('world/reference-scene-v2/' + path)
    expect(hash(file), path).toBe(sha)
  }
  const geometry = JSON.parse(new TextDecoder().decode(await bytes('world/reference-scene-v2/geometry-proposal.json')))
  expect(inputs.geometry).toEqual(geometry.geometry); expect(await digest(inputs.geometry)).toBe(inputs.geometry_hash)
  const packed = inputs.scenery, resources = [packed.ground, ...packed.atlases]
  for (const r of resources) { const file = await bytes(r.path); expect(file.length).toBe(r.bytes); expect(hash(file)).toBe(r.sha256) }
  expect(resources.reduce((n, r) => n + r.bytes, 0)).toBe(1947817)
  expect(resources.reduce((n, r) => n + r.width_px * r.height_px, 0)).toBe(2043904)
})
it('loads genuine source bytes through the incumbent C06/Tiled loader and binds distinct WORLD/MAP projections', async () => {
  const s = await setup(), map = await loadReferenceScene(s.input, new AbortController().signal), binding = requireReferenceScene(map)
  expect(binding.loaded.manifest).toEqual(inputs.scenery); expect(binding.loaded.assets).toHaveLength(2)
  expect(binding.loaded.manifest.objects).toHaveLength(22); expect(binding.overview).toHaveLength(27)
  expect(binding.overview.some(o => o.id === 'main_stage')).toBe(false)
  expect(binding.overview.filter(o => o.id === 'stage_venue_representation')).toHaveLength(1)
  expect(binding.loaded.manifest.objects.some(o => o.id === 'stage_venue_representation')).toBe(false)
  expect(binding.plaza).toEqual(inputs.plaza_parcel); expect(binding.worldCamera).toMatchObject({ x: 160, y: 192, width: 584, height: 584 })
  expect(s.live.size).toBe(3); map.dispose(); map.dispose(); expect(s.live.size).toBe(0)
  expect(s.decode.mock.results.every(r => r.type === 'return')).toBe(true)
})
it.each(['geometry', 'revision', 'theme', 'frame', 'foot-depth', 'asset-hash', 'source-url'] as const)('rejects %s mismatch before any fetch/decode', async kind => {
  const s = await setup(), { map, theme } = s.f
  if (kind === 'geometry') { map.geometry.collision_grid[0]![0] = 0; map.geometry_hash = await digest(map.geometry); theme.geometry_hash = map.geometry_hash }
  if (kind === 'revision') { map.map_revision++; theme.map_revision++ }
  if (kind === 'theme') theme.theme_id = 'DANGER'
  if (kind === 'frame') Object.assign(theme.scenery!.objects[0]!.frame, { x: 9 })
  if (kind === 'foot-depth') Object.assign(theme.scenery!.objects[0]!, { depth_y: 540 })
  if (kind === 'asset-hash') Object.assign(theme.scenery!.atlases[0]!, { sha256: 'sha256:' + '0'.repeat(64) })
  if (kind === 'source-url') Object.assign(theme.scenery!.atlases[0]!, { path: 'world/reference-scene-v2/painted/parade-source-atlas-v3.png' })
  map.content_hash = await contentHash(map); theme.map_content_hash = map.content_hash; theme.content_hash = await contentHash(theme)
  await expect(loadReferenceScene({ ...s.input, mapText: JSON.stringify(map), themeText: JSON.stringify(theme),
    pin: { ...s.input.pin, mapHash: map.content_hash, themeHash: theme.content_hash } }, new AbortController().signal)).rejects.toThrow()
  expect(s.read).not.toHaveBeenCalled(); expect(s.decode).not.toHaveBeenCalled()
})
it('does not classify ordinary incumbent releases as reference scenes or admit an incomplete injected lease', async () => {
  const s = await setup(), metadata = await validateManifests(s.input.mapText, s.input.themeText, s.input.pin)
  const map = { ...metadata, image: {} as HTMLImageElement, dispose: vi.fn() }
  expect(() => requireReferenceScene(map)).toThrow()
  const ordinary = { ...map, map: { ...map.map, map_id: 'old', geometry_hash: 'sha256:' + '0'.repeat(64) }, theme: { ...map.theme, scenery: undefined } }
  expect(referenceSceneFor(ordinary as unknown as Parameters<typeof referenceSceneFor>[0])).toBeNull()
})
it('fences an abort-resistant late decoder and releases its result without completing a scene lease', async () => {
  const s = await setup(), controller = new AbortController(); let release!: (lease: DecodedImage) => void
  const pending = loadReferenceScene({ ...s.input, decode: () => new Promise<DecodedImage>(r => { release = r }) }, controller.signal)
  const failed = expect(pending).rejects.toThrow()
  await vi.waitFor(() => expect(release).toBeTypeOf('function')); controller.abort(); await failed
  const dispose = vi.fn(); release({ image: {} as HTMLImageElement, dispose }); await Promise.resolve(); await Promise.resolve(); expect(dispose).toHaveBeenCalledOnce()
})
