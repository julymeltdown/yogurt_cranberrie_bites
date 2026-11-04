import { beforeAll, expect, expectTypeOf, it, vi } from 'vitest'
import type { AvatarState as WireAvatarState } from '../../../network/transport/wire-types'
import { canonicalJson } from '../../../map/json'
import { AvatarError, decodeAvatarState, decodeConfig, parseConfig, validateConfig, validateManifest, type AvatarConfig, type AvatarManifest } from '../../../../entities/avatar/whole-admission'
import { schemaValid } from '../../../../entities/avatar/runtime/schema'
import { validateWholeCharacterConfig } from '../../../../entities/avatar/whole-character/contract'
import { REVIEWED_MOTION } from '../../../../entities/avatar/runtime/motion-adoption/reviewed-manifest'
import { readReviewedProducer, reviewedInputs } from './producer-inputs'

const text = (path: string) => new TextDecoder('utf-8', { fatal: true }).decode(readReviewedProducer(path))
const raw = text(REVIEWED_MOTION.manifest.path), document = JSON.parse(raw)
const pin = { manifest_id: REVIEWED_MOTION.manifestId, manifest_version: 2, content_hash: REVIEWED_MOTION.contentHash }
const fixtures = Object.keys(reviewedInputs.files).filter(path => path.startsWith('contracts/fixtures/assets/'))
let manifest: AvatarManifest
beforeAll(async () => { manifest = await validateManifest(raw, pin) })

it('exports exactly canonical AvatarState.config and admits the reviewed document without fetching or activating resources', async () => {
  expectTypeOf<AvatarConfig>().toEqualTypeOf<WireAvatarState['config']>()
  const fetch = vi.spyOn(globalThis, 'fetch').mockRejectedValue(Error('Public DTO validation must not fetch'))
  try {
    const checked = await validateManifest(canonicalJson(document), pin)
    expect(checked).toEqual(document); expect(Object.isFrozen(checked.presets)).toBe(true)
    expect(checked).toMatchObject({ contract_version: '2.0.0', manifest_id: pin.manifest_id, manifest_version: 2 })
    expect(fetch).not.toHaveBeenCalled()
  } finally { fetch.mockRestore() }
})

it.each(fixtures)('preserves canonical reviewed public fixture validation %s', path => {
  const wire = text(path)
  if (path.includes('/invalid/')) expect(() => decodeConfig(wire, manifest)).toThrow(AvatarError)
  else {
    const input = JSON.parse(wire), config = parseConfig(input)
    expect(config).toEqual(input); expect(config).not.toBe(input); expect(Object.isFrozen(config)).toBe(true)
    expect(validateConfig(input, manifest)).toEqual(input); expect(decodeConfig(wire, manifest)).toEqual(input)
    expect(decodeAvatarState({ revision: 2, config: input })).toEqual({ revision: 2, config: input })
    // Old rendering contracts remain genuinely restricted to their original families.
    expect(schemaValid(input, 'config')).toBe(false)
    expect(() => validateWholeCharacterConfig(input)).toThrow()
  }
})

it('rejects wrong document pins, branch mismatches and missing inventory membership through the public entry points', async () => {
  const input = JSON.parse(text(fixtures.find(path => path.endsWith('/layered-max.json'))!))
  for (const patch of [{ manifest_id: 'other' }, { manifest_version: 1 }]) {
    await expect(validateManifest(raw, { ...pin, ...patch })).rejects.toMatchObject({ reason: 'version' })
    expect(() => validateConfig({ ...input, ...patch }, manifest)).toThrow(AvatarError)
  }
  await expect(validateManifest(raw, { ...pin, content_hash: `sha256:${'0'.repeat(64)}` })).rejects.toMatchObject({ reason: 'invalid' })
  const reviewed = manifest
  if (reviewed.contract_version !== '2.0.0') throw Error('Wrong reviewed fixture')
  expect(() => validateConfig(input, { ...reviewed, items: reviewed.items.filter(item => item.item_id !== input.item_ids.body) })).toThrow(AvatarError)
  const preset = document.presets[0].config
  expect(() => validateConfig(preset, { ...reviewed, presets: [] })).toThrow(AvatarError)
  for (const state of [{ revision: 0, config: input }, { revision: 1.5, config: input }, { revision: 1, config: input, private: true },
    { revision: 1 }, { config: input }]) expect(() => decodeAvatarState(state)).toThrow(AvatarError)
})

it('keeps strict public text rejection and error classes for malformed reviewed input', async () => {
  const wire = text(fixtures.find(path => path.endsWith('/preset-01.json'))!)
  for (const value of [wire.replace('"kind":', '"kind":"avatar_config","kind":'),
    wire.replace('"manifest_version": 2', '"manifest_version": 2.0'), `${wire} null`]) {
    expect(() => decodeConfig(value, manifest)).toThrow(AvatarError)
  }
  for (const value of [null, [], false, { contract_version: '2.0.0' }]) expect(() => parseConfig(value)).toThrow(AvatarError)
  for (const value of [raw.replace('"kind":', '"kind":"avatar_manifest","kind":'),
    canonicalJson({ ...document, production_eligible: false }), canonicalJson({ ...document, review: { ...document.review, path: 'other' } })]) {
    await expect(validateManifest(value, pin)).rejects.toThrow(AvatarError)
  }
})
