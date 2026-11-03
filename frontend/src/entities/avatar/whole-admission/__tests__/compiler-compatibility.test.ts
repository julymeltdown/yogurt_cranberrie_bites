import { expect, it } from 'vitest'
import bootstrap from '../../../../../../contracts/fixtures/core/bootstrap-open.json'
import layered from '../../../../../../contracts/fixtures/assets/avatar-config.json'
import selected from '../../../../../../contracts/fixtures/assets/selected-static/avatar-manifest.json'
import management from '../../../../../../contracts/fixtures/domain/giveaway-management.json'
import { decodeBootstrap } from '../../../../features/auth/model/bootstrap'
import { decodeContract } from '../../../../features/auth/model/contract'
import { giveawayListDecoder } from '../../../../features/gift-discovery/model/contract'
import { parse } from '../../../../features/event-feed/contract'
import { contractValue } from '../../../../features/party-authoring/model/contract'

it('admits the retained complete bootstrap through real Auth admission after the C06 union update', async () => {
  const scope = { generation: 1, userId: bootstrap.participant.user_id, eventId: bootstrap.event.event_id }
  const checked = await decodeBootstrap(bootstrap, scope)
  expect(checked.document).toEqual(bootstrap)
  expect(Object.isFrozen(checked.document)).toBe(true)
})

it.each([layered, selected.presets[0]!.config])('public contract compilers admit exact config branch $contract_version', config => {
  expect(parse('AvatarConfig', config)).toEqual(config)
  expect(contractValue('AvatarConfig', config)).toEqual(config)
  const value = structuredClone(bootstrap)
  Object.assign(value.avatar.config, config)
  // Replace rather than merge branches: strict schemas must never strip mixed fields.
  const avatar = { revision: value.avatar.revision, config }
  expect(decodeContract('Bootstrap', { ...value, avatar, participant: { ...value.participant, profile: { ...value.participant.profile, avatar } } }).avatar).toEqual(avatar)
  const gift = { ...management.giveaway, owner: { ...management.giveaway.owner, avatar } }
  const scope = { generation: 1, userId: gift.owner.user_id, eventId: gift.event_id }
  const path = `/api/concert/events/${scope.eventId}/giveaways`
  const result = giveawayListDecoder(scope, {})({ items: [gift], next_cursor: null }, { scope, path, status: 200 })
  expect(result.items[0]!.owner.avatar).toEqual(avatar)
})

it.each([
  { ...selected.presets[0]!.config, item_ids: layered.item_ids },
  { ...selected.presets[0]!.config, preset_id: 'photo01_front_07' },
  { ...selected.presets[0]!.config, contract_version: '1.0.0' },
])('public compilers reject zero-match, mixed or unknown union branch %#', config => {
  expect(() => parse('AvatarConfig', config)).toThrow()
  expect(() => contractValue('AvatarConfig', config)).toThrow()
})
