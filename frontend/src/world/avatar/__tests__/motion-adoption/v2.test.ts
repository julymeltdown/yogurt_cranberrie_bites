import { expect, it } from 'vitest'
import { decodeMotionV2Config, motionV2Config, motionV2Selection, MOTION_V2_PRESETS } from '../../../../entities/avatar/runtime/motion-adoption/v2'

it('round trips the producer-assigned six preset namespaces without accepting art labels as wire IDs', () => {
  for (const preset of MOTION_V2_PRESETS) {
    const config = motionV2Config({ mode: 'preset', preset: `photo01-front-${preset.slice(-2)}` })
    expect(config).toMatchObject({ contract_version: '2.0.0', representation: 'motion_preset', preset_id: preset })
    expect(motionV2Config(motionV2Selection(config))).toEqual(config)
    expect(() => decodeMotionV2Config({ ...config, preset_id: 'photo01-front-01' })).toThrow()
  }
})
it('round trips all 6720 assigned layered combinations including explicit null outfit/accessory', () => {
  let compared = 0
  for (let body = 0; body < 8; body++) for (let eyes = 0; eyes < 4; eyes++) for (let hair = 0; hair < 6; hair++)
    for (const outfit of [null, 0, 1, 2, 3, 4, 5]) for (const accessory of [null, 0, 1, 2, 3]) {
    const selection = { mode: 'wardrobe' as const, body: `body-${body}`, eyes: `eyes-${eyes}`, hair: `hair-${hair}`,
      outfit: outfit === null ? null : `outfit-${outfit}`, accessory: accessory === null ? null : `accessory-${accessory}` }
    const config = motionV2Config(selection)
    expect(motionV2Selection(config)).toEqual(selection); expect(Object.isFrozen(config)).toBe(true)
    compared++
  }
  expect(compared).toBe(6720)
})
it('rejects mixed branches, old versions, arbitrary URLs, over-range and source-only IDs', () => {
  const preset = motionV2Config({ mode: 'preset', preset: 'photo01-front-01' })
  const layered = motionV2Config({ mode: 'wardrobe', body: 'body-0', eyes: 'eyes-0', hair: 'hair-0', outfit: 'outfit-0', accessory: null })
  if (layered.representation !== 'motion_layered') throw Error('Wrong fixture')
  for (const value of [{ ...preset, contract_version: '1.1.0' }, { ...preset, manifest_id: 'mcr_selected_static_20261103' }, { ...preset, manifest_version: 2 },
    { ...preset, item_ids: layered.item_ids }, { ...layered, preset_id: MOTION_V2_PRESETS[0] }, { ...layered, skin_tone_id: 'tone_0' },
    { ...layered, item_ids: { ...layered.item_ids, body: 'body-0' } }, { ...layered, item_ids: { ...layered.item_ids, body: 'motion_v2_body_8' } },
    { ...layered, item_ids: { ...layered.item_ids, eyes: null } }, { ...layered, item_ids: { ...layered.item_ids, hair: 'motion_v2_outfit_0' } },
    { ...layered, item_ids: { ...layered.item_ids, accessory: 'https://example.invalid/a.png' } }, { ...layered, item_ids: { ...layered.item_ids, extra: null } }]) expect(() => decodeMotionV2Config(value)).toThrow()
  for (const config of [preset, layered]) for (const key of Object.keys(config)) {
    const missing: Record<string, unknown> = { ...config }; delete missing[key]
    expect(() => decodeMotionV2Config(missing)).toThrow()
  }
  for (const key of Object.keys(layered.item_ids)) {
    const items: Record<string, unknown> = { ...layered.item_ids }; delete items[key]
    expect(() => decodeMotionV2Config({ ...layered, item_ids: items })).toThrow()
  }
})
