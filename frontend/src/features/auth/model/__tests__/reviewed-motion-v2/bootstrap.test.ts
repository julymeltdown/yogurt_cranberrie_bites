import { createHash } from 'node:crypto'
import { expect, it } from 'vitest'
import bootstrap from '../../../../../../../contracts/fixtures/core/bootstrap-open.json'
import whole from '../../../../../../../contracts/fixtures/assets/selected-static/avatar-manifest.json'
import reviewedSchema from '../../../../../../../contracts/assets/v2/reviewed/avatar-manifest.schema.json'
import { canonicalJson, decodeBootstrap } from '../../bootstrap'
import { decodeContract, list, object, type ObjectValue } from '../../contract'

const valid = import.meta.glob('../../../../../../../contracts/fixtures/assets/motion-v2/reviewed/valid/*.json', { eager: true, import: 'default' })
const invalid = import.meta.glob('../../../../../../../contracts/fixtures/assets/motion-v2/reviewed/invalid/*.json', { eager: true, import: 'default' })
const scope = { generation: 1, userId: bootstrap.participant.user_id, eventId: bootstrap.event.event_id }
const reviewed = reviewedSchema.$defs.ReviewedMotionAvatarManifest.const
function rehash(document: ObjectValue) {
  const { content_hash: _, ...body } = document
  document.content_hash = `sha256:${createHash('sha256').update(canonicalJson(body)).digest('hex')}`
}
function bundle(manifest: unknown, config: unknown): ObjectValue {
  const b = object(structuredClone(bootstrap)), avatarManifest = object(structuredClone(manifest))
  b.avatar_manifest = avatarManifest
  b.avatar = { revision: 2, config: structuredClone(config) }
  object(object(b.participant).profile).avatar = structuredClone(b.avatar)
  for (const value of list(b.theme_manifests)) {
    const theme = object(value)
    theme.avatar_manifest_id = avatarManifest.manifest_id
    theme.avatar_manifest_version = avatarManifest.manifest_version
    theme.avatar_content_hash = avatarManifest.content_hash
    rehash(theme)
  }
  return b
}

it('retains the legacy layered bootstrap unchanged', async () => {
  expect((await decodeBootstrap(bootstrap, scope)).document).toEqual(bootstrap)
})
it('retains the legacy whole-character bootstrap unchanged', async () => {
  const b = bundle(whole, whole.presets[0]!.config)
  expect((await decodeBootstrap(b, scope)).document).toEqual(b)
})
it.each(Object.entries(valid))('validates and freezes the reviewed2 structural bundle %s without granting release authority', async (_name, config) => {
  const b = bundle(reviewed, config)
  const checked = await decodeBootstrap(b, scope)
  expect(checked.document).toEqual(b)
  expect(Object.isFrozen(checked.document)).toBe(true)
  expect(Object.isFrozen(object(checked.document).avatar_manifest)).toBe(true)
  expect(Object.isFrozen(object(object(checked.document).avatar).config)).toBe(true)
  // Auth preserves the resource document; it produces no runtime registry authority.
  expect(Object.hasOwn(checked, 'authority')).toBe(false)
  expect(Object.hasOwn(checked, 'qualification')).toBe(false)
})
it.each(Object.entries(invalid))('rejects malformed reviewed config before shell admission: %s', async (_name, config) => {
  await expect(decodeBootstrap(bundle(reviewed, config), scope)).rejects.toThrow()
})
it.each([
  ['private profile field', (b: ObjectValue) => { object(object(b.participant).profile).email = 'private@example.invalid' }],
  ['private config field', (b: ObjectValue) => { object(object(b.avatar).config).email = 'private@example.invalid'; object(object(b.participant).profile).avatar = structuredClone(b.avatar) }],
  ['profile mismatch', (b: ObjectValue) => { object(object(b.participant).profile).avatar = bootstrap.avatar }],
  ['legacy config with reviewed manifest', (b: ObjectValue) => { b.avatar = bootstrap.avatar; object(object(b.participant).profile).avatar = bootstrap.avatar }],
  ['manifest hash mismatch', (b: ObjectValue) => { object(b.avatar_manifest).content_hash = `sha256:${'0'.repeat(64)}` }],
  ['theme binding mismatch', (b: ObjectValue) => { const theme = object(list(b.theme_manifests)[0]); theme.avatar_manifest_version = 1; rehash(theme) }],
  ['unknown reviewed preset', (b: ObjectValue) => { b.avatar = { revision: 2, config: { ...reviewed.presets[0]!.config, preset_id: 'unknown' } }; object(object(b.participant).profile).avatar = structuredClone(b.avatar) }],
  ['forged reviewed manifest with recomputed hash', (b: ObjectValue) => { const m = object(b.avatar_manifest); m.production_eligible = false; rehash(m) }],
  ['changed atlas with recomputed hash', (b: ObjectValue) => { const m = object(b.avatar_manifest); object(m.atlas).path = 'https://untrusted.invalid/atlas.png'; rehash(m) }],
  ['unknown manifest field with recomputed hash', (b: ObjectValue) => { const m = object(b.avatar_manifest); m.extra = true; rehash(m) }],
])('rejects the reviewed2 bundle: %s', async (_name, change) => {
  const b = bundle(reviewed, reviewed.presets[0]!.config)
  change(b)
  await expect(decodeBootstrap(b, scope)).rejects.toThrow()
})

it('checks the full closed manifest const before parser delegation', () => {
  const b = bundle(reviewed, reviewed.presets[0]!.config)
  expect(decodeContract('Bootstrap', b)).toEqual(b)
  const manifest = object(b.avatar_manifest)
  object(manifest.source).revision = 999
  rehash(manifest)
  expect(() => decodeContract('Bootstrap', b)).toThrow()
})

it('owns caller bytes before asynchronous bundle verification', async () => {
  const b = bundle(reviewed, reviewed.presets[0]!.config), original = structuredClone(b)
  const pending = decodeBootstrap(b, scope)
  object(object(b.avatar).config).preset_id = 'mutated_while_awaiting'
  expect((await pending).document).toEqual(original)
})
