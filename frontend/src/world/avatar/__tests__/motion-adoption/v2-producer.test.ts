import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { expect, it } from 'vitest'
import layered from '../../../../../../contracts/fixtures/assets/avatar-config.json'
import staticManifest from '../../../../../../contracts/fixtures/assets/selected-static/avatar-manifest.json'
import { canonicalJson, sha256 } from '../../../map/json'
import { parseConfig } from '../../../../entities/avatar/whole-admission'
import { validateWholeCharacterConfig } from '../../../../entities/avatar/whole-character/contract'
import { schemaValid } from '../../../../entities/avatar/runtime/schema'
import { MOTION_ART, validateMotionArt } from '../../../../entities/avatar/runtime/motion-adoption/art'
import { decodeMotionV2ConfigText, motionV2Config, motionV2Selection } from '../../../../entities/avatar/runtime/motion-adoption/v2'
import { createMotionV2CandidateAuthority, decodeMotionV2Candidate, MOTION_V2_CANDIDATE, type MotionV2Candidate } from '../../../../entities/avatar/runtime/motion-adoption/v2-candidate'
import { producerCheckout as checkout, producerInputs, readPinnedProducer as producer } from './producer-inputs'

const text = (bytes: Uint8Array) => new TextDecoder('utf-8', { fatal: true }).decode(bytes)
const manifestBytes = producer(MOTION_V2_CANDIDATE.path)
const fixturePaths = Object.keys(producerInputs.files).filter(path => path.startsWith('contracts/fixtures/assets/motion-v2/'))

it('pins the actual producer schemas and byte-for-byte const candidate without granting production eligibility', async () => {
  for (const resource of [MOTION_V2_CANDIDATE, MOTION_V2_CANDIDATE.configSchema, MOTION_V2_CANDIDATE.manifestSchema]) {
    const bytes = producer(resource.path)
    expect(bytes.byteLength).toBe(resource.bytes); expect(await sha256(bytes)).toBe(resource.sha256)
  }
  const candidate = await decodeMotionV2Candidate(manifestBytes)
  const schema = JSON.parse(text(producer(MOTION_V2_CANDIDATE.manifestSchema.path)))
  expect(canonicalJson(candidate.manifest)).toBe(canonicalJson(schema.const))
  expect(candidate.manifest).toMatchObject({ production_eligible: false, qualification: 'CANDIDATE_ONLY', fallback: 'NONE' })
  expect(Object.isFrozen(candidate.manifest.items)).toBe(true)
  expect(createMotionV2CandidateAuthority(candidate).qualification).toBe('TEST_ONLY')
  expect(() => createMotionV2CandidateAuthority({ manifest: candidate.manifest } as MotionV2Candidate)).toThrow()
})

it.each(fixturePaths)('follows the immutable producer configuration fixture %s', path => {
  const bytes = producer(path)
  if (path.includes('/invalid/')) expect(() => decodeMotionV2ConfigText(text(bytes))).toThrow()
  else {
    const config = decodeMotionV2ConfigText(text(bytes))
    expect(motionV2Config(motionV2Selection(config))).toEqual(config)
    expect(canonicalJson(config)).toBe(canonicalJson(JSON.parse(text(bytes))))
  }
})

it('preserves both legacy DTO families and the static-only guard while rejecting v2 through unactivated old public decoders', () => {
  expect(parseConfig(layered)).toEqual(layered)
  for (const preset of staticManifest.presets) expect(validateWholeCharacterConfig(parseConfig(preset.config))).toEqual(preset.config)
  for (const path of fixturePaths.filter(path => path.includes('/valid/'))) {
    const config = decodeMotionV2ConfigText(text(producer(path)))
    expect(schemaValid(config, 'config')).toBe(false)
    expect(() => validateWholeCharacterConfig(config)).toThrow()
    expect(() => parseConfig(config)).toThrow()
  }
})

it('rejects private fields, duplicate keys, lexical floats, oversized text and mutated candidate admission flags', async () => {
  const config = text(producer(fixturePaths.find(path => path.endsWith('/preset-01.json'))!))
  for (const wire of [config.replace('"kind":', '"access_token":"private","kind":'),
    config.replace('"kind":', '"kind":"avatar_config","kind":'),
    config.replace('"manifest_version": 1', '"manifest_version": 1.0'),
    config.replace('"manifest_version": 1', '"manifest_version": 1e0'), ' '.repeat(4097)]) {
    expect(() => decodeMotionV2ConfigText(wire)).toThrow()
  }
  for (const wire of [text(manifestBytes).replace('"production_eligible": false', '"production_eligible": true'),
    text(manifestBytes).replace('"fallback": "NONE"', '"fallback": "LEGACY"'), text(manifestBytes) + '\n']) {
    await expect(decodeMotionV2Candidate(new TextEncoder().encode(wire))).rejects.toThrow()
  }
})

it('binds every producer UV, semantic palette and canonical identifier to the same hash-verified selected artwork', async () => {
  const art = await validateMotionArt(new Uint8Array(readFileSync(resolve(checkout, 'frontend/public', MOTION_ART.descriptor.path))))
  const candidate = await decodeMotionV2Candidate(manifestBytes), authority = createMotionV2CandidateAuthority(candidate)
  const aliases = ['front', 'left', 'right', 'rear']
  const presets = authority.presets(art)
  expect(presets).toHaveLength(6); expect(candidate.manifest.items).toHaveLength(28)
  for (const preset of candidate.manifest.presets) for (const [direction, row] of preset.directions.entries()) {
    expect(row.frames).toEqual(art.descriptor.presets.frames.filter(frame => frame.preset === preset.local_key && frame.direction === aliases[direction]).map(frame => frame.atlasIndex))
  }
  for (const item of candidate.manifest.items) {
    const source = art.descriptor.wardrobe.items.find(row => row.key === item.local_key && row.kind === item.slot)!
    expect(source).toBeDefined()
    for (const [direction, row] of item.directions.entries()) {
      expect(row.frames).toEqual(source.directions[direction]!.frames)
      expect(row.hand_frames).toEqual(source.directions[direction]!.handFrames)
    }
    expect(item.skin_palette?.colors).toEqual(source.skinPalette?.colors)
  }
})
