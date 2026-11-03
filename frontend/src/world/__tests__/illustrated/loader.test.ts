import { beforeEach, expect, it, vi } from 'vitest'
import { webcrypto } from 'node:crypto'
import { readFile } from 'node:fs/promises'
import { illustratedReleases } from './fixtures'
import { loadMap, type Resource } from '../../map/resources'
import { contentHash } from '../../map/json'
import { sceneryDescriptors } from '../../illustrated/model'
beforeEach(() => vi.stubGlobal('crypto', webcrypto))
const bytes = async (r: Resource) => new Uint8Array(await readFile(new URL(`../../../../public/${r.path}`, import.meta.url)))
const fixture = () => illustratedReleases(async path => JSON.parse(await readFile(new URL(`../../../../public/${path}`, import.meta.url), 'utf8')))
const decoded = () => {
  const disposals: ReturnType<typeof vi.fn>[] = []
  const decode = vi.fn(async (data: Uint8Array<ArrayBuffer>) => {
    const view = new DataView(data.buffer, data.byteOffset), dispose = vi.fn(); disposals.push(dispose)
    return { image: { naturalWidth: view.getUint32(16), naturalHeight: view.getUint32(20) } as HTMLImageElement, dispose }
  }); return { decode, disposals }
}
it.each(['REVENGE', 'PARADE', 'DANGER'])('loads exact bounded original %s sources and releases every decoded lease once', async id => {
  const { map, themes, avatar } = await fixture(), theme = themes.find(t => t.theme_id === id)!, d = decoded()
  const result = await loadMap({ mapText: JSON.stringify(map), themeText: JSON.stringify(theme), pin: { mapHash: map.content_hash, themeHash: theme.content_hash, avatar }, resources: { read: bytes }, decode: d.decode }, new AbortController().signal)
  expect(result.scenery?.assets).toHaveLength(3); expect(d.decode).toHaveBeenCalledTimes(4)
  expect(result.scenery!.transferBytes).toBeLessThan(600_000)
  expect(result.scenery!.decodedBytes).toBe(6_225_920)
  expect(result.map.geometry).toEqual(map.geometry)
  result.dispose(); result.dispose(); expect(d.disposals.every(f => f.mock.calls.length === 1)).toBe(true)
})
it.each(['digest', 'bytes', 'dimensions', 'atlas-frame', 'geometry', 'absent-binding'])(
  'fails closed on %s with no retained decoded assets', async kind => {
    const { map, themes, avatar } = await fixture(), theme = themes[0]!, d = decoded()
    if (kind === 'bytes') Object.assign(theme.scenery!.ground, { bytes: 1 })
    if (kind === 'dimensions') Object.assign(theme.scenery!.atlases[1]!, { width_px: 513 })
    if (kind === 'atlas-frame') Object.assign(theme.scenery!.objects[0]!.frame, { width: 2048 })
    if (kind === 'geometry') map.geometry.collision_grid[0]![0] = 0
    theme.content_hash = await contentHash(theme)
    const read = async (r: Resource) => {
      const value = await bytes(r)
      if (kind === 'digest' && r.path === theme.scenery!.ground.path) value[value.length - 1]! ^= 1
      if (kind === 'absent-binding' && r.path === theme.scenery!.ground.path) throw Error('Not allowlisted')
      return value
    }
    await expect(loadMap({ mapText: JSON.stringify(map), themeText: JSON.stringify(theme), pin: { mapHash: map.content_hash, themeHash: theme.content_hash, avatar }, resources: { read }, decode: d.decode }, new AbortController().signal)).rejects.toBeTruthy()
    expect(d.disposals.every(f => f.mock.calls.length === 1)).toBe(true)
  })
it('disposes earlier assets and a decoder ignoring cancellation when its late result arrives', async () => {
  const { map, themes, avatar } = await fixture(), theme = themes[0]!, controller = new AbortController(), d = decoded()
  let finish!: (value: { image: HTMLImageElement; dispose: () => void }) => void
  const decode = vi.fn(async (data: Uint8Array<ArrayBuffer>) => {
    if (decode.mock.calls.length === 2) return new Promise<{ image: HTMLImageElement; dispose: () => void }>(r => { finish = r })
    return d.decode(data)
  })
  const pending = loadMap({ mapText: JSON.stringify(map), themeText: JSON.stringify(theme), pin: { mapHash: map.content_hash, themeHash: theme.content_hash, avatar }, resources: { read: bytes }, decode }, controller.signal)
  const rejection = expect(pending).rejects.toMatchObject({ reason: 'cancelled' })
  await vi.waitFor(() => expect(finish).toBeTypeOf('function')); controller.abort(); await rejection
  const late = vi.fn(); finish({ image: {} as HTMLImageElement, dispose: late }); await vi.waitFor(() => expect(late).toHaveBeenCalledOnce())
  expect(d.disposals[0]).toHaveBeenCalledOnce()
})
it('never admits editing originals or composite preview resources', async () => {
  const { themes } = await fixture()
  for (const theme of themes) expect(sceneryDescriptors(theme).map(t => t.path).join('\n')).not.toMatch(/original|scene\.png|landmarks\.png|terrain-tiles/)
})
