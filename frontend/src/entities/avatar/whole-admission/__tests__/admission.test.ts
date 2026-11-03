import { afterEach, expect, expectTypeOf, it, vi } from 'vitest'
import layeredConfig from '../../../../../../contracts/fixtures/assets/avatar-config.json'
import layeredManifest from '../../../../../../contracts/fixtures/assets/avatar-manifest.json'
import wholeManifest from '../../../../../../contracts/fixtures/assets/selected-static/avatar-manifest.json'
import { canonicalJson, contentHash } from '../../../../world/map/json'
import { schemaValid } from '../../runtime/schema'
import {
  validateConfig as validateLayeredConfig, validateManifest as validateLayeredManifest,
  type AvatarConfig as RuntimeLayeredConfig,
} from '../../runtime/contract'
import { createAvatarCompositor } from '../../runtime/compositor'
import { validateWholeCharacterConfig, validateWholeCharacterManifest } from '../../whole-character/contract'
import {
  AvatarError, decodeConfig, validateConfig, validateManifest,
  type AvatarConfig, type LayeredAvatarConfig, type WholeCharacterConfig,
} from '..'
import type { AvatarConfig as WireAvatarConfig } from '../../../../world/network/transport/wire-types'

afterEach(() => vi.restoreAllMocks())
const configs = wholeManifest.presets.map(preset => preset.config)
it('exports the genuine wire union while the legacy runtime stays explicitly layered', () => {
  expectTypeOf<AvatarConfig>().toEqualTypeOf<WireAvatarConfig>()
  expectTypeOf<RuntimeLayeredConfig>().toEqualTypeOf<Extract<WireAvatarConfig, { contract_version: '1.0.0' }>>()
  expectTypeOf<LayeredAvatarConfig>().toEqualTypeOf<RuntimeLayeredConfig>()
  expectTypeOf<WholeCharacterConfig>().toEqualTypeOf<Extract<WireAvatarConfig, { contract_version: '1.1.0' }>>()
  // @ts-expect-error A persisted whole-character config cannot enter the layered runtime API.
  const forbidden: RuntimeLayeredConfig = {} as WholeCharacterConfig
  expect(forbidden).toEqual({})
})

it('admits both actual manifest versions without fetching artwork or external schema references', async () => {
  const fetcher = vi.spyOn(globalThis, 'fetch').mockRejectedValue(new Error('No admission fetch'))
  for (const source of [layeredManifest, wholeManifest]) {
    expect(schemaValid(source, 'manifest')).toBe(true)
    const manifest = await validateManifest(canonicalJson(source), source)
    expect(canonicalJson(manifest)).toBe(canonicalJson(source))
    expect(Object.isFrozen(manifest.presets)).toBe(true)
    expect(Object.isFrozen(manifest.atlas.texture)).toBe(true)
  }
  expect(fetcher).not.toHaveBeenCalled()
})

it.each(configs.map(c => [c.preset_id, c] as const))('preserves canonical %s, all fields and ownership', async (_, config) => {
  const manifest = await validateManifest(canonicalJson(wholeManifest), wholeManifest)
  const input = structuredClone(config), checked = validateConfig(input, manifest)
  expect(schemaValid(input, 'config')).toBe(true)
  expect(checked).toEqual(config)
  expect(checked).not.toBe(input)
  expect(canonicalJson(decodeConfig(canonicalJson(checked), manifest))).toBe(canonicalJson(config))
  expect(Object.isFrozen(checked)).toBe(true)
  input.preset_id = 'not_a_preset'
  expect(checked).toEqual(config)
})

it('retains layered membership validation through the canonical union entrypoint', async () => {
  const manifest = await validateManifest(canonicalJson(layeredManifest), layeredManifest)
  expect(decodeConfig(canonicalJson(layeredConfig), manifest)).toEqual(layeredConfig)
  for (const input of [
    { ...layeredConfig, skin_tone_id: 'missing' },
    { ...layeredConfig, item_ids: { ...layeredConfig.item_ids, body: 'unknown' } },
    { ...layeredConfig, item_ids: { ...layeredConfig.item_ids, hair: layeredConfig.item_ids.body } },
  ]) expect(() => validateConfig(input, manifest)).toThrow(AvatarError)
})

it('rejects branch/manifest mismatches and never admits a whole config into the layered fallback compositor', async () => {
  const layered = await validateLayeredManifest(canonicalJson(layeredManifest), layeredManifest)
  const whole = await validateManifest(canonicalJson(wholeManifest), wholeManifest)
  expect(() => validateConfig(configs[0], layered)).toThrow('version')
  expect(() => validateConfig(layeredConfig, whole)).toThrow('version')
  expect(() => validateLayeredConfig(configs[0], layered)).toThrow('version')
  await expect(validateLayeredManifest(canonicalJson(wholeManifest), wholeManifest)).rejects.toMatchObject({ reason: 'version' })
  const compositor = createAvatarCompositor({ manifest: layered, image: {} as CanvasImageSource,
    bakedSkinToneId: layeredConfig.skin_tone_id, fallbackPresetId: layered.fallback_preset_id, disposeAtlas() {} })
  try {
    expect(() => compositor.acquire(configs[0])).toThrow('version')
    expect(compositor.read()).toMatchObject({ entries: 0, created: 0, references: 0 })
  } finally { compositor.dispose() }
})

const invalidConfigs: [string, unknown][] = [
  ['unknown preset', { ...configs[0], preset_id: 'photo01_front_07' }],
  ['art ID used as config ID', { ...configs[0], preset_id: 'photo01-front-01' }],
  ['wrong type', { ...configs[0], preset_id: 1 }],
  ['URL ID', { ...configs[0], preset_id: 'https://legacy.invalid/avatar.png' }],
  ['unknown field', { ...configs[0], url: '/legacy.png' }],
  ['layer mix', { ...configs[0], item_ids: layeredConfig.item_ids }],
  ['tone mix', { ...configs[0], skin_tone_id: layeredConfig.skin_tone_id }],
  ['whole tagged layered', { ...configs[0], contract_version: '1.0.0' }],
  ['layered tagged whole', { ...layeredConfig, contract_version: '1.1.0' }],
  ['layered with whole fields', { ...layeredConfig, representation: 'whole_character', preset_id: configs[0]!.preset_id }],
  ['unknown version', { ...configs[0], contract_version: '2.0.0' }],
  ['wrong representation', { ...configs[0], representation: 'layered' }],
  ['wrong manifest', { ...configs[0], manifest_id: 'other' }],
  ['wrong revision', { ...configs[0], manifest_version: 2 }],
  ['null', null], ['array', configs],
]
it.each(invalidConfigs)('fails closed on %s without mutation or defaulting', async (_, input) => {
  const manifest = await validateManifest(canonicalJson(wholeManifest), wholeManifest), before = canonicalJson(input)
  expect(schemaValid(input, 'config')).toBe(false)
  expect(() => validateConfig(input, manifest)).toThrow(AvatarError)
  expect(() => decodeConfig(before, manifest)).toThrow(AvatarError)
  expect(canonicalJson(input)).toBe(before)
})

it.each([
  ['duplicate field', (text: string) => text.replace('"manifest_version":1', '"manifest_version":1,"manifest_version":1')],
  ['escaped duplicate', (text: string) => text.replace('"preset_id":', '"preset_id":"photo01_front_02","preset_\\u0069d":')],
  ['float revision', (text: string) => text.replace('"manifest_version":1', '"manifest_version":1.0')],
  ['exponent revision', (text: string) => text.replace('"manifest_version":1', '"manifest_version":1e0')],
  ['trailing value', (text: string) => `${text} null`],
  ['oversize', () => ' '.repeat(1_048_577)],
] as const)('strict config text rejects %s', async (_, mutate) => {
  const manifest = await validateManifest(canonicalJson(wholeManifest), wholeManifest)
  expect(() => decodeConfig(mutate(canonicalJson(configs[0])), manifest)).toThrow(AvatarError)
})

it.each(['id', 'version', 'hash'] as const)('rejects an incorrect manifest %s pin', async field => {
  const pin = { ...wholeManifest }
  if (field === 'id') pin.manifest_id = 'other'
  if (field === 'version') pin.manifest_version = 2
  if (field === 'hash') pin.content_hash = `sha256:${'0'.repeat(64)}`
  await expect(validateManifest(canonicalJson(wholeManifest), pin)).rejects.toThrow(AvatarError)
})

it('recomputes the manifest hash rather than trusting a matching input/pin hash', async () => {
  const candidate = { ...wholeManifest, content_hash: `sha256:${'0'.repeat(64)}` }
  expect(schemaValid(candidate, 'manifest')).toBe(true)
  await expect(validateManifest(canonicalJson(candidate), candidate)).rejects.toMatchObject({ reason: 'invalid' })
})

it('feeds unchanged admitted whole values into the actual published selected-art contract and requires its reviewed manifest pin', async () => {
  const checked = await validateManifest(canonicalJson(wholeManifest), wholeManifest)
  expect(await validateWholeCharacterManifest(checked, wholeManifest)).toEqual(wholeManifest)
  for (const config of configs) expect(validateWholeCharacterConfig(validateConfig(config, checked))).toEqual(config)
  const altered = structuredClone(wholeManifest)
  altered.default_preset_id = altered.presets[1]!.preset_id
  altered.content_hash = await contentHash(altered)
  expect(schemaValid(altered, 'manifest')).toBe(true)
  await expect(validateManifest(canonicalJson(altered), altered)).rejects.toThrow(AvatarError)
})

const mutations: [string, (m: typeof wholeManifest) => void][] = [
  ['unknown field', m => { Object.assign(m, { extra: true }) }],
  ['mixed layers', m => { Object.assign(m, { layer_order: layeredManifest.layer_order }) }],
  ['unsupported version', m => { m.contract_version = '1.2.0' }],
  ['wrong kind', m => { m.kind = 'avatar_config' }],
  ['preset overflow', m => { m.presets.push(m.presets[0]!) }],
  ['missing preset', m => { m.presets.pop() }],
  ['duplicate preset', m => { m.presets[1] = m.presets[0]! }],
  ['unknown preset', m => { m.presets[0]!.preset_id = 'unknown' }],
  ['mismatched preset config', m => { m.presets[0]!.config = m.presets[1]!.config }],
  ['mismatched art ID', m => { m.presets[0]!.art_id = m.presets[1]!.art_id }],
  ['mismatched frame', m => { m.presets[0]!.frame.x = 64 }],
  ['missing default', m => { m.default_preset_id = 'missing' }],
  ['frame numeric type', m => { Object.assign(m.presets[0]!.frame, { x: '0' }) }],
  ['wrong texture hash', m => { m.atlas.texture.sha256 = `sha256:${'f'.repeat(64)}` }],
  ['invented animation', m => { m.capabilities.poses.push('walk') }],
  ['unsupported source', m => { m.source.source_pack_revision++ }],
  ['external schema injection', m => { Object.assign(m, { $ref: 'https://evil.invalid/schema.json' }) }],
]
it.each(mutations)('rejects canonical-but-invalid manifest %s even after rehashing', async (_, mutate) => {
  const candidate = structuredClone(wholeManifest); mutate(candidate); candidate.content_hash = await contentHash(candidate)
  expect(schemaValid(candidate, 'manifest')).toBe(false)
  await expect(validateManifest(canonicalJson(candidate), candidate)).rejects.toMatchObject({ reason: 'invalid' })
})

it('rejects duplicate manifest keys and lexical float metadata at the text boundary', async () => {
  const text = canonicalJson(wholeManifest)
  for (const value of [text.replace('"manifest_version":1', '"manifest_version":1,"manifest_version":1'),
    text.replace('"width_px":64', '"width_px":64.0')]) {
    await expect(validateManifest(value, wholeManifest)).rejects.toMatchObject({ reason: 'invalid' })
  }
})
