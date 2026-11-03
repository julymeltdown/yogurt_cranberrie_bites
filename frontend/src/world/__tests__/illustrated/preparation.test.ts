import { beforeEach, expect, it, vi } from 'vitest'
import { webcrypto } from 'node:crypto'
import { readFile } from 'node:fs/promises'
import type Phaser from 'phaser'
import { illustratedReleases } from './fixtures'
import { prepareIllustratedSkin } from '../../illustrated/phaser'
import { sceneryDescriptors } from '../../illustrated/model'
import { validateManifests } from '../../map/manifest'
import { validateReplacement } from '../../theme/compatibility'
import type { LoadedMap } from '../../map/resources'
import { contentHash } from '../../map/json'
beforeEach(() => vi.stubGlobal('crypto', webcrypto))
async function candidate() {
  const { map, themes, avatar } = await illustratedReleases(async path => JSON.parse(await readFile(new URL(`../../../../public/${path}`, import.meta.url), 'utf8')))
  const theme = themes[0]!, metadata = await validateManifests(JSON.stringify(map), JSON.stringify(theme), { mapHash: map.content_hash, themeHash: theme.content_hash, avatar })
  const assets = sceneryDescriptors(theme).map(descriptor => ({ descriptor, image: { naturalWidth: descriptor.width_px, naturalHeight: descriptor.height_px } as HTMLImageElement }))
  const result: LoadedMap = { ...metadata, image: { naturalWidth: 256, naturalHeight: 128 } as HTMLImageElement, dispose() {},
    scenery: { manifest: metadata.theme.scenery!, assets, transferBytes: assets.reduce((n, a) => n + a.descriptor.bytes, 0), decodedBytes: 6_225_920 } }
  return result
}
function scene(failure: 'none' | 'upload' | 'frame' = 'none') {
  const textures = new Map<string, object>([['old-valid', {}]]), rows: { visible: boolean; destroyed: boolean }[] = []
  let uploads = 0, frames = 0
  const stub = { textures: {
    addImage(key: string) { if (++uploads === 2 && failure === 'upload') throw Error('GPU allocation failed'); const texture = { setFilter() {}, add() { if (++frames === 3 && failure === 'frame') throw Error('Frame allocation failed'); return {} } }; textures.set(key, texture); return texture },
    exists: (key: string) => textures.has(key), remove: (key: string) => textures.delete(key), get: (key: string) => textures.get(key),
  }, add: { image() {
    const row = { visible: true, destroyed: false, setOrigin() { return this }, setDepth() { return this }, setDisplaySize() { return this }, setVisible(value: boolean) { this.visible = value; return this }, destroy() { this.destroyed = true } }; rows.push(row); return row
  } } }
  return { scene: stub as unknown as Phaser.Scene, textures, rows }
}
it('prepares every object hidden, then shows/removes one skin without touching the previous texture', async () => {
  const next = await candidate(), s = scene(), prepared = prepareIllustratedSkin(s.scene, next, 'new', 0, () => {}, () => true)
  expect(s.rows).toHaveLength(30); expect(s.rows.every(r => !r.visible)).toBe(true)
  prepared.show(); expect(s.rows.every(r => r.visible)).toBe(true)
  prepared.remove(); prepared.remove(); expect(s.rows.every(r => r.destroyed)).toBe(true); expect([...s.textures.keys()]).toEqual(['old-valid'])
})
it.each(['upload', 'frame'] as const)('cleans partial %s allocation failure while keeping old resources', async failure => {
  const next = await candidate(), s = scene(failure)
  expect(() => prepareIllustratedSkin(s.scene, next, 'new', 0, () => {}, () => true)).toThrow()
  expect([...s.textures.keys()]).toEqual(['old-valid']); expect(s.rows.every(r => r.destroyed)).toBe(true)
})
it('fences synchronous lifetime withdrawal during texture upload', async () => {
  const next = await candidate(), s = scene(); let alive = true
  expect(() => prepareIllustratedSkin(s.scene, next, 'new', 0, () => { alive = false }, () => alive)).toThrow()
  expect([...s.textures.keys()]).toEqual(['old-valid'])
})
it('refuses a rehashed era that moves a bound object before any visual commit', async () => {
  const before = await candidate(), next = await candidate(), theme = structuredClone(next.theme)
  Object.assign(theme.scenery!.objects[0]!, { x: theme.scenery!.objects[0]!.x + 1 }); theme.content_hash = await contentHash(theme)
  await expect(validateReplacement(before, { ...next, theme })).rejects.toMatchObject({ reason: 'invalid' })
})
