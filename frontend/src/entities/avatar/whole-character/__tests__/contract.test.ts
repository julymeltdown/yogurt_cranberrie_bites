import { expect, it } from 'vitest'
import { PRESET_IDS, REVIEWED, sameWholeCharacterConfig, validateWholeCharacterConfig, validateWholeCharacterManifest, wholeSchemaValid } from '../contract'
import { config, manifest, pin } from './fixture'
it('validates the actual published manifest and six exact canonical configurations with stable immutable identity', async () => {
  const checked = await validateWholeCharacterManifest(JSON.stringify(manifest), pin)
  expect(checked.content_hash).toBe(REVIEWED.manifestHash)
  expect(checked.presets.map(p => p.config.preset_id)).toEqual(PRESET_IDS)
  for (const preset of checked.presets) { expect(wholeSchemaValid(preset.config, 'config')).toBe(true); expect(validateWholeCharacterConfig(preset.config)).toEqual(preset.config); expect(Object.isFrozen(preset.config)).toBe(true) }
  expect(sameWholeCharacterConfig(config, { ...config })).toBe(true)
  expect(sameWholeCharacterConfig(config, checked.presets[1]!.config)).toBe(false)
})
it.each([{ preset_id: 'photo01-front-01' }, { preset_id: 'photo01_front_07' }, { contract_version: '1.0.0' }, { representation: 'layered' }, { item_ids: {} }, { skin_tone_id: 'light' }, { direction: 'up' }, { animation: 'walk' }, { manifest_version: 2 }, { url: 'https://other.invalid/a.png' }, { manifest_id: 'other' }])('rejects mixed/unreviewed identities and invented capability %j', patch => {
  expect(() => validateWholeCharacterConfig({ ...config, ...patch })).toThrow()
  expect(sameWholeCharacterConfig(config, { ...config, ...patch })).toBe(false)
})
it('rejects duplicate keys, oversized wire text, mutated frames, source binding and manifest/pin hashes', async () => {
  await expect(validateWholeCharacterManifest(JSON.stringify(manifest).replace('"contract_version":', '"contract_version":"1.1.0","contract_version":'), pin)).rejects.toThrow()
  await expect(validateWholeCharacterManifest(' '.repeat(32769), pin)).rejects.toThrow()
  for (const bad of [{ ...manifest, content_hash: 'sha256:' + '0'.repeat(64) }, { ...manifest, presets: manifest.presets.slice(1) }, { ...manifest, source: { ...manifest.source, source_commit: '0'.repeat(40) } }, { ...manifest, capabilities: { ...manifest.capabilities, poses: ['walk'] } }]) await expect(validateWholeCharacterManifest(bad, pin)).rejects.toThrow()
  await expect(validateWholeCharacterManifest(manifest, { ...pin, manifest_version: 2 })).rejects.toThrow()
  for (const key of Object.keys(config)) { const missing = { ...config } as Record<string, unknown>; delete missing[key]; expect(() => validateWholeCharacterConfig(missing)).toThrow() }
})
