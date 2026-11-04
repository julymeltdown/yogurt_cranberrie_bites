import { readFile } from 'node:fs/promises'
import { expect, it } from 'vitest'
import { MOTION_ART, motionFrame, validateMotionArt, validateMotionSelection } from '../../../../entities/avatar/runtime/motion-adoption/art'

const source = () => readFile(new URL('../../../../../public/' + MOTION_ART.descriptor.path, import.meta.url))
it('accepts only the frozen descriptor bytes; the candidate remains explicitly ineligible', async () => {
  const bytes = new Uint8Array(await source()), art = await validateMotionArt(bytes)
  expect(art.descriptor.productionEligible).toBe(false)
  expect(Object.isFrozen(art.descriptor.wardrobe.items[0])).toBe(true)
  for (const edited of [bytes.slice(0, -1), bytes.map((b, i) => i === 100 ? b ^ 1 : b)]) await expect(validateMotionArt(edited)).rejects.toThrow()
})
it('local artwork selection is strict and never accepts public configs, URLs or unknown items', async () => {
  const art = await validateMotionArt(new Uint8Array(await source()))
  expect(validateMotionSelection({ mode: 'preset', preset: 'photo01-front-01' }, art)).toEqual({ mode: 'preset', preset: 'photo01-front-01' })
  const wardrobe = { mode: 'wardrobe', body: 'body-7', hair: 'hair-5', eyes: 'eyes-3', outfit: 'outfit-5', accessory: null }
  expect(validateMotionSelection(wardrobe, art)).toEqual(wardrobe)
  for (const selection of [{ ...wardrobe, body: 'body-8' }, { ...wardrobe, eyes: null }, { ...wardrobe, accessory: 'https://evil/image.png' },
    { ...wardrobe, url: 'world/arbitrary.png' }, { mode: 'preset', preset: 'photo01-front-01', productionEligible: true }, { contract_version: '1.1.0', preset_id: 'photo01_front_01' }]) {
    expect(() => validateMotionSelection(selection, art)).toThrow()
  }
})
it('selects actual directional idle/four-walk cells from caller time, without scheduling a loop', () => {
  for (const [row, direction] of (['down', 'left', 'right', 'up'] as const).entries()) {
    expect(motionFrame(direction, false, 560)).toBe(row * 5)
    expect([0, 140, 280, 420, 560].map(time => motionFrame(direction, true, time))).toEqual([1, 2, 3, 4, 1].map(x => row * 5 + x))
  }
  for (const time of [-1, NaN, Infinity]) expect(() => motionFrame('down', true, time)).toThrow()
  expect(() => motionFrame('diagonal' as 'down', false, 0)).toThrow()
})
