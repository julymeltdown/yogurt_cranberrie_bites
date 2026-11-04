import { expect, it } from 'vitest'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { canonicalJson, contentHash, sha256 } from '../../../map/json'
import { MOTION_ART, validateMotionArt } from '../../../../entities/avatar/runtime/motion-adoption/art'
import { decodeMotionV2Config, motionV2Config } from '../../../../entities/avatar/runtime/motion-adoption/v2'
import { decodeReviewedMotionConfig, decodeReviewedMotionConfigText, reviewedMotionConfig, reviewedMotionSelection } from '../../../../entities/avatar/runtime/motion-adoption/reviewed-config'
import { createReviewedMotionFixtureAuthority, decodeReviewedMotionManifest, REVIEWED_MOTION, type ReviewedMotionStage } from '../../../../entities/avatar/runtime/motion-adoption/reviewed-manifest'
import { decodeMotionV2Candidate, MOTION_V2_CANDIDATE } from '../../../../entities/avatar/runtime/motion-adoption/v2-candidate'
import { producerCheckout, readPinnedProducer, readReviewedProducer, reviewedInputs } from './producer-inputs'

const text = (bytes: Uint8Array) => new TextDecoder('utf-8', { fatal: true }).decode(bytes)
const pin = Object.freeze({ manifest_id: REVIEWED_MOTION.manifestId, manifest_version: 2, content_hash: REVIEWED_MOTION.contentHash })
const manifestBytes = readReviewedProducer(REVIEWED_MOTION.manifest.path), document = JSON.parse(text(manifestBytes))
const fixtures = Object.keys(reviewedInputs.files).filter(path => path.startsWith('contracts/fixtures/assets/'))

it.each(fixtures)('uses the distinct reviewed producer config fixture %s', path => {
  const wire = text(readReviewedProducer(path))
  if (path.includes('/invalid/')) expect(() => decodeReviewedMotionConfigText(wire)).toThrow()
  else {
    const config = decodeReviewedMotionConfigText(wire)
    expect(config.manifest_id).toBe(REVIEWED_MOTION.manifestId); expect(config.manifest_version).toBe(2)
    expect(reviewedMotionConfig(reviewedMotionSelection(config))).toEqual(config)
    expect(() => decodeMotionV2Config(config)).toThrow()
  }
})

it('pins the actual canonical successor/receipt/schemas and matches the complete const definition', async () => {
  for (const [path, expected] of Object.entries(reviewedInputs.files)) {
    const bytes = readReviewedProducer(path)
    expect(bytes.byteLength).toBe(expected.bytes); expect(await sha256(bytes)).toBe(`sha256:${expected.sha256}`)
  }
  const schema = JSON.parse(text(readReviewedProducer('contracts/assets/v2/reviewed/avatar-manifest.schema.json')))
  expect(canonicalJson(schema.$defs.ReviewedMotionAvatarManifest.const)).toBe(canonicalJson(document))
  expect(await sha256(manifestBytes)).toBe(REVIEWED_MOTION.manifest.sha256)
  expect(await sha256(readReviewedProducer(REVIEWED_MOTION.review.path))).toBe(REVIEWED_MOTION.review.sha256)
  expect(await contentHash(document)).toBe(REVIEWED_MOTION.contentHash)
})

it('verifies canonical, pretty and object manifests without treating staged eligibility as runtime admission', async () => {
  for (const input of [document, text(manifestBytes), canonicalJson(document)]) {
    const stage = await decodeReviewedMotionManifest(input, pin)
    expect(stage.qualification).toBe('STAGED_NOT_ADMITTED')
    expect(stage.manifest).toMatchObject({ manifest_id: REVIEWED_MOTION.manifestId, manifest_version: 2, production_eligible: true, qualification: 'VISUALLY_REVIEWED' })
    expect(Object.isFrozen(stage.manifest.presets)).toBe(true)
    expect(createReviewedMotionFixtureAuthority(stage).qualification).toBe('TEST_ONLY')
    expect(() => createReviewedMotionFixtureAuthority({ manifest: stage.manifest, qualification: 'STAGED_NOT_ADMITTED' } as ReviewedMotionStage)).toThrow()
  }
  // The staged successor does not relabel or mutate the old immutable candidate.
  const candidate = await decodeMotionV2Candidate(readPinnedProducer(MOTION_V2_CANDIDATE.path))
  expect(candidate.manifest.production_eligible).toBe(false)
  expect(candidate.manifest.items).toEqual(document.items)
  expect(candidate.manifest.presets).toEqual(document.presets.map(({ config, ...art }: { config: unknown }) => {
    expect(reviewedMotionConfig(reviewedMotionSelection(config))).toEqual(config)
    return art
  }))
})

it('rejects family/revision aliases, duplicate keys, lexical floats and private config fields', () => {
  const candidate = motionV2Config({ mode: 'preset', preset: 'photo01-front-01' })
  expect(() => decodeReviewedMotionConfig(candidate)).toThrow()
  const config = reviewedMotionConfig({ mode: 'preset', preset: 'photo01-front-01' })
  expect(() => decodeReviewedMotionConfig({ ...config, manifest_version: 1 })).toThrow()
  expect(() => decodeReviewedMotionConfig({ ...candidate, manifest_version: 2 })).toThrow()
  expect(() => decodeReviewedMotionConfig({ ...config, access_token: 'private' })).toThrow()
  const wire = JSON.stringify(config)
  for (const value of [wire.replace('"kind":', '"kind":"avatar_config","kind":'), wire.replace('"manifest_version":2', '"manifest_version":2.0'),
    wire.replace('"manifest_version":2', '"manifest_version":2e0'), ' '.repeat(4097)]) expect(() => decodeReviewedMotionConfigText(value)).toThrow()
})

it('exercises all six embedded reviewed presets and explicit wardrobe replacement against the frozen artwork', async () => {
  const art = await validateMotionArt(new Uint8Array(readFileSync(resolve(producerCheckout, 'frontend/public', MOTION_ART.descriptor.path))))
  const authority = createReviewedMotionFixtureAuthority(await decodeReviewedMotionManifest(document, pin))
  const presets = authority.presets(art)
  expect(presets.map(preset => preset.config)).toEqual(document.presets.map((preset: { config: unknown }) => preset.config))
  expect(presets).toHaveLength(6)
  for (const preset of presets) {
    expect(authority.decodeConfig(preset.config)).toEqual(preset.config)
    expect(authority.selection(preset.config)).toMatchObject({ mode: 'preset' })
    const selected = { mode: 'wardrobe' as const, body: 'body-7', eyes: 'eyes-3', hair: 'hair-5', outfit: null, accessory: null }
    const next = authority.replace(preset.config, selected)
    expect(authority.selection(next)).toEqual(selected)
    expect(next).toMatchObject({ manifest_id: REVIEWED_MOTION.manifestId, manifest_version: 2 })
  }
  expect(authority.manifestHash).toBe(pin.content_hash)
  expect(authority.qualification).toBe('TEST_ONLY')
})

it('rejects modified review, art, inventory, qualification, extras and self-rehashed or duplicate manifest inputs', async () => {
  for (const patch of [{ review: { ...document.review, sha256: '0'.repeat(64) } }, { atlas: { ...document.atlas, path: 'https://other.invalid/a.png' } },
    { production_eligible: false }, { qualification: 'ACCEPTED' }, { manifest_version: 1 }, { presets: document.presets.slice(1) },
    { items: document.items.slice(1) }, { access_token: 'private' }]) {
    const changed = { ...document, ...patch }
    await expect(decodeReviewedMotionManifest(changed, pin)).rejects.toThrow()
    changed.content_hash = await contentHash(changed)
    await expect(decodeReviewedMotionManifest(changed, { ...pin, content_hash: changed.content_hash })).rejects.toThrow()
  }
  const wire = text(manifestBytes).replace('"kind":', '"kind":"avatar_manifest","kind":')
  await expect(decodeReviewedMotionManifest(wire, pin)).rejects.toThrow()
  await expect(decodeReviewedMotionManifest(' '.repeat(131073), pin)).rejects.toThrow()
  await expect(decodeReviewedMotionManifest(document, { ...pin, manifest_version: 1 })).rejects.toThrow()
  await expect(decodeReviewedMotionManifest(JSON.parse(text(readPinnedProducer(MOTION_V2_CANDIDATE.path))), pin)).rejects.toThrow()
})

it('round trips all 6720 reviewed layered combinations with distinct reviewed identity and closed IDs', () => {
  let compared = 0
  for (let body = 0; body < 8; body++) for (let hair = 0; hair < 6; hair++) for (let eyes = 0; eyes < 4; eyes++)
    for (const outfit of [null, 0, 1, 2, 3, 4, 5]) for (const accessory of [null, 0, 1, 2, 3]) {
      const selection = { mode: 'wardrobe' as const, body: `body-${body}`, hair: `hair-${hair}`, eyes: `eyes-${eyes}`,
        outfit: outfit === null ? null : `outfit-${outfit}`, accessory: accessory === null ? null : `accessory-${accessory}` }
      const config = reviewedMotionConfig(selection)
      expect(reviewedMotionSelection(config)).toEqual(selection); expect(config.manifest_version).toBe(2); compared++
    }
  expect(compared).toBe(6720)
})
