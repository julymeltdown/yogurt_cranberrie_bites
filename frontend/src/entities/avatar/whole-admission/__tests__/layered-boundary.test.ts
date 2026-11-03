import { expect, it } from 'vitest'
import config from '../../../../../../contracts/fixtures/assets/avatar-config.json'
import manifest from '../../../../../../contracts/fixtures/assets/avatar-manifest.json'
import { canonicalJson } from '../../../../world/map/json'
import { validateConfig, validateManifest, type AvatarConfig } from '../../runtime/contract'

it('retains the layered runtime type and canonical layered fixture admission', async () => {
  const checked = await validateManifest(canonicalJson(manifest), manifest)
  const layered: AvatarConfig = validateConfig(config, checked)
  expect(layered.contract_version).toBe('1.0.0')
  expect(layered.item_ids).toEqual(config.item_ids)
  expect(canonicalJson(layered)).toBe(canonicalJson(config))
  expect(Object.isFrozen(layered.item_ids)).toBe(true)
})

it.each(['1.1.0', '2.0.0'])('rejects a layered-shaped value tagged %s instead of rewriting it', async version => {
  const checked = await validateManifest(canonicalJson(manifest), manifest)
  const candidate = { ...config, contract_version: version }
  expect(() => validateConfig(candidate, checked)).toThrow()
  expect(candidate.contract_version).toBe(version)
})
