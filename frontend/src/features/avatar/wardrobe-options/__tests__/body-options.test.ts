import { readFile } from 'node:fs/promises'
import { URL as FileURL } from 'node:url'
import { beforeAll, expect, it } from 'vitest'
import { validateMotionArt, type MotionArt, type MotionItem, type MotionSelection } from '../../../../entities/avatar/runtime/motion-adoption/art'
import { motionBodyOptions, motionBodySelection } from '../body-options'
import schema from '../../../../../../contracts/assets/v2/reviewed/avatar-manifest.schema.json'

let art: MotionArt
beforeAll(async () => { art = await validateMotionArt(new Uint8Array(await readFile(new FileURL('../../../../../public/world/avatars/derived-v1/motion-candidates-v1/combined-release-candidate.json', import.meta.url)))) })
const selection: MotionSelection = { mode: 'wardrobe', body: 'body-0', hair: 'hair-5', eyes: 'eyes-3', outfit: 'outfit-4', accessory: null }
it('shows all8 qualified body RGB triplets exactly as the C06 semantic hand/body index specifies', () => {
  const manifest = schema.$defs.ReviewedMotionAvatarManifest.const
  expect(manifest.composition.hand_palette_color_index).toBe(2)
  const body = manifest.items.filter(item => item.slot === 'body'), options = motionBodyOptions(art.descriptor.wardrobe.items)
  expect(options).toHaveLength(8)
  body.forEach((item, index) => {
    const palette = 'skin_palette' in item ? item.skin_palette : undefined
    if (!palette) throw Error('Missing reviewed body palette')
    expect(options[index]).toEqual({ key: item.local_key, label: `Skin tone ${index + 1}`, color: `rgb(${palette.colors[2]!.join(', ')})` })
  })
  expect(Object.isFrozen(options)).toBe(true); expect(options.every(Object.isFrozen)).toBe(true)
})
it('changes only body across every authorized key, preserving all other selection fields and inputs', () => {
  const options = motionBodyOptions(art.descriptor.wardrobe.items)
  for (const option of options) {
    const next = motionBodySelection(selection, option.key, options)
    expect(next).toEqual({ ...selection, body: option.key }); expect(Object.isFrozen(next)).toBe(true)
  }
  expect(selection.body).toBe('body-0')
  expect(motionBodySelection({ mode: 'preset', preset: 'photo01-front-01' }, 'body-0', options)).toBeNull()
  expect(motionBodySelection(selection, 'foreign-body', options)).toBeNull()
})
it.each(['absent', 'short', 'long', 'short-rgb', 'negative', 'large', 'fraction', 'nan'] as const)('preserves the real key but marks %s palette metadata unavailable', kind => {
  const original = art.descriptor.wardrobe.items.find(item => item.kind === 'body')!
  const colors: number[][] = [[1, 2, 3], [4, 5, 6], [7, 8, 9], [10, 11, 12]]
  if (kind === 'short') colors.pop()
  if (kind === 'long') colors.push([1, 2, 3])
  if (kind === 'short-rgb') colors[2]!.pop()
  if (kind === 'negative') colors[2]![0] = -1
  if (kind === 'large') colors[2]![0] = 256
  if (kind === 'fraction') colors[2]![0] = 1.5
  if (kind === 'nan') colors[2]![0] = Number.NaN
  const { skinPalette: _palette, ...withoutPalette } = original
  const item: MotionItem = kind === 'absent' ? withoutPalette : { ...original, skinPalette: { colors } }
  const options = motionBodyOptions([item])
  expect(options).toEqual([{ key: original.key, label: 'Skin tone 1', color: null }])
  expect(motionBodySelection(selection, original.key, options)).toBeNull()
})
it('does not infer palettes from keys, non-body artwork, unknown items or duplicate body keys', () => {
  const body = art.descriptor.wardrobe.items.filter(item => item.kind === 'body')
  expect(motionBodyOptions([])).toEqual([])
  expect(motionBodyOptions(art.descriptor.wardrobe.items.filter(item => item.kind !== 'body'))).toEqual([])
  expect(motionBodyOptions([body[0]!, body[0]!])).toEqual([])
  expect(motionBodyOptions([{ ...body[0]!, key: '' }])).toEqual([])
})
