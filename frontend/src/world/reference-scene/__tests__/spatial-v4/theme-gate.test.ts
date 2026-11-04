import { readFile } from 'node:fs/promises'
import { expect, it } from 'vitest'
import { spatialV4Fixture } from './fixture'
import { loadReferenceScene, spatialV4ThemeIds, type SpatialV4ThemeId } from '../..'
import { assertSameWorld, validateReplacement } from '../../../theme/compatibility'

async function load(theme: SpatialV4ThemeId) {
  const f = await spatialV4Fixture(theme)
  return loadReferenceScene({ mapText: JSON.stringify(f.map), themeText: JSON.stringify(f.theme), pin: f.pin,
    resources: { read: async r => new Uint8Array(await readFile(new URL('../../../../../public/'+r.path, import.meta.url))) },
    decode: async bytes => { const png = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
      return { image: { naturalWidth: png.getUint32(16), naturalHeight: png.getUint32(20) } as HTMLImageElement, dispose() {} } },
  }, new AbortController().signal)
}
it.each(spatialV4ThemeIds)('accepts %s packed frame and atlas changes while retaining the immutable world', async id => {
  const current = await load(id)
  try { for (const nextId of spatialV4ThemeIds.filter(next => next !== id)) {
    const next = await load(nextId)
    try { expect(current.theme.scenery!.objects[0]!.atlas_id).not.toBe(next.theme.scenery!.objects[0]!.atlas_id)
      await expect(validateReplacement(current, next)).resolves.toBeUndefined()
    } finally { next.dispose() }
  } } finally { current.dispose() }
})
it.each(['id', 'x', 'y', 'width', 'height', 'depth_y', 'order', 'missing'] as const)('rejects changed world object %s despite allowing texture packing changes', async field => {
  const current = await load('PARADE'), next = await load('DANGER')
  try {
    const theme = structuredClone(next.theme), objects = [...theme.scenery!.objects]
    if (field === 'order') [objects[0], objects[1]] = [objects[1]!, objects[0]!]
    else if (field === 'missing') objects.pop()
    else Object.assign(objects[0]!, { [field]: field === 'id' ? 'other_object' : objects[0]![field] + 1 })
    Object.assign(theme.scenery!, { objects })
    expect(() => assertSameWorld(current, { ...next, theme })).toThrow()
  } finally { current.dispose(); next.dispose() }
})
