import { expect, it } from 'vitest'
import bootstrap from '../../../../../../contracts/fixtures/core/bootstrap-open.json'
import layered from '../../../../../../contracts/fixtures/assets/avatar-config.json'
import selected from '../../../../../../contracts/fixtures/assets/selected-static/avatar-manifest.json'
import { contentHash } from '../../../../world/map/json'
import { decodeBootstrap } from '../../../../features/auth/model/bootstrap'
import { decodeParticipant } from '../../../../features/me/model/contract'
import { decodeProfile } from '../../../../features/people/contract'
import { decodeAvatarState } from '..'

const scope = { generation: 1, userId: bootstrap.participant.user_id, eventId: bootstrap.event.event_id }
async function wholeBootstrap() {
  const avatar = { revision: 7, config: structuredClone(selected.presets[4]!.config) }
  const themes = await Promise.all(bootstrap.theme_manifests.map(async source => {
    const theme = { ...structuredClone(source), avatar_manifest_id: selected.manifest_id,
      avatar_manifest_version: selected.manifest_version, avatar_content_hash: selected.content_hash }
    theme.content_hash = await contentHash(theme)
    return theme
  }))
  return { ...structuredClone(bootstrap), avatar, avatar_manifest: structuredClone(selected), theme_manifests: themes,
    participant: { ...structuredClone(bootstrap.participant), profile: { ...structuredClone(bootstrap.participant.profile), avatar } } }
}

it('admits a real-shaped whole-character Bootstrap with identity, theme/hash and exact canonical avatar retained', async () => {
  const input = await wholeBootstrap(), result = await decodeBootstrap(input, scope), document = result.document
  expect(document).toEqual(input)
  if (!document) throw new Error('Expected the admitted resource document')
  expect(document.avatar).toEqual({ revision: 7, config: selected.presets[4]!.config })
  expect(Object.isFrozen(document.avatar_manifest)).toBe(true)
  input.avatar.config.preset_id = 'changed_by_caller'
  expect(document.avatar).toEqual({ revision: 7, config: selected.presets[4]!.config })
})

it.each(['identity', 'profile-identity', 'profile-avatar', 'unknown-preset', 'theme-pin', 'manifest-hash', 'frame-mapping', 'branch-mix', 'read-only'])(
  'whole Bootstrap fails closed on %s', async kind => {
    const input = await wholeBootstrap()
    if (kind === 'identity') input.participant.user_id = '00000000-0000-4000-8000-000000000099'
    if (kind === 'profile-identity') input.participant.profile.user_id = '00000000-0000-4000-8000-000000000099'
    if (kind === 'profile-avatar') input.participant.profile.avatar = { ...input.avatar, revision: 8 }
    if (kind === 'unknown-preset') input.avatar.config.preset_id = 'photo01_front_07'
    if (kind === 'theme-pin') {
      input.theme_manifests[0]!.avatar_content_hash = `sha256:${'0'.repeat(64)}`
      input.theme_manifests[0]!.content_hash = await contentHash(input.theme_manifests[0]!)
    }
    if (kind === 'manifest-hash') input.avatar_manifest.content_hash = `sha256:${'0'.repeat(64)}`
    if (kind === 'frame-mapping') {
      input.avatar_manifest.presets[0]!.frame.x = 64
      input.avatar_manifest.content_hash = await contentHash(input.avatar_manifest)
    }
    if (kind === 'branch-mix') Object.assign(input.avatar.config, { item_ids: layered.item_ids })
    if (kind === 'read-only') { input.event.phase = 'READ_ONLY'; Object.assign(input.capabilities.mutations, { available: true, reason: null }) }
    await expect(decodeBootstrap(input, scope)).rejects.toThrow()
  },
)

it.each([layered, ...selected.presets.map(p => p.config)])('Me and People retain exact canonical branch $contract_version/$preset_id', config => {
  const avatar = { revision: 9, config }, profile = { ...bootstrap.participant.profile, avatar }
  const me = decodeParticipant({ ...bootstrap.participant, profile }, scope)
  const person = decodeProfile(profile, scope.userId)
  expect(me.profile.avatar).toEqual(avatar)
  expect(person.avatar).toEqual(avatar)
  expect(Object.isFrozen(person.avatar.config)).toBe(true)
  expect(person.avatar.config).not.toBe(config)
  expect(decodeAvatarState(avatar)).toEqual(avatar)
})

it.each([
  { revision: 0, config: layered }, { revision: 1.5, config: layered }, { revision: Number.MAX_SAFE_INTEGER + 1, config: layered },
  { revision: 1, config: selected.presets[0]!.config, extra: true },
  { revision: 1, config: { ...selected.presets[0]!.config, item_ids: layered.item_ids } },
  { revision: 1, config: { ...selected.presets[0]!.config, preset_id: 'unknown' } },
  { revision: 1, config: { ...layered, contract_version: '1.1.0' } },
])('Me and People reject invalid revision/union %# without stripping fields', avatar => {
  const profile = { ...bootstrap.participant.profile, avatar }
  expect(() => decodeAvatarState(avatar)).toThrow()
  expect(() => decodeParticipant({ ...bootstrap.participant, profile }, scope)).toThrow()
  expect(() => decodeProfile(profile, scope.userId)).toThrow()
})
