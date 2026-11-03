import { afterEach, expect, it, vi } from 'vitest'
import core from '../../../../../../contracts/generated/mcr-core.openapi.json'
import domain from '../../../../../../contracts/generated/mcr-domain.openapi.json'
import layered from '../../../../../../contracts/fixtures/assets/avatar-config.json'
import bootstrap from '../../../../../../contracts/fixtures/core/bootstrap-open.json'
import management from '../../../../../../contracts/fixtures/domain/giveaway-management.json'
import { party } from '../../../../features/party-discovery/__tests__/fixtures/data'

const corePath = '../../../../../../contracts/generated/mcr-core.openapi.json'
const domainPath = '../../../../../../contracts/generated/mcr-domain.openapi.json'
afterEach(() => { vi.doUnmock(corePath); vi.doUnmock(domainPath); vi.resetModules() })
for (const mode of ['ambiguous', 'unknown-keyword'] as const) {
  it(`all five compilers reject ${mode} schema instead of treating oneOf as anyOf`, async () => {
    // Deliberately corrupted test-only schema copies prove rejection semantics.
    // Checked-in canonical schemas are never mutated by this test.
    const coreCopy = structuredClone(core), domainCopy = structuredClone(domain)
    for (const schemas of [coreCopy.components.schemas, domainCopy.components.schemas]) {
      if (mode === 'ambiguous') schemas.AvatarConfig.oneOf = [schemas.AvatarConfig.oneOf[0]!, schemas.AvatarConfig.oneOf[0]!]
      else Object.assign(schemas.LayeredAvatarConfig, { unsupported_future_keyword: true })
    }
    vi.resetModules()
    vi.doMock(corePath, () => ({ default: coreCopy }))
    vi.doMock(domainPath, () => ({ default: domainCopy }))
    const auth = await import('../../../../features/auth/model/contract')
    expect(() => auth.decodeContract('Bootstrap', bootstrap)).toThrow()
    const feed = await import('../../../../features/event-feed/contract')
    expect(() => feed.parse('AvatarConfig', layered)).toThrow()
    const authoring = await import('../../../../features/party-authoring/model/contract')
    expect(() => authoring.contractValue('AvatarConfig', layered)).toThrow()
    const discovery = await import('../../../../features/party-discovery/model/contract')
    const value = party(), scope = { generation: 1, userId: value.owner.user_id, eventId: value.event_id }, path = `/api/concert/events/${scope.eventId}/parties`
    expect(() => discovery.decodeList(20, path)({ items: [value], next_cursor: null }, { scope, status: 200, path })).toThrow()
    if (mode === 'unknown-keyword') await expect(import('../../../../features/gift-discovery/model/contract')).rejects.toThrow()
    else {
      const gifts = await import('../../../../features/gift-discovery/model/contract'), gift = management.giveaway
      const giftScope = { generation: 1, userId: gift.owner.user_id, eventId: gift.event_id }, giftPath = `/api/concert/events/${gift.event_id}/giveaways`
      expect(() => gifts.giveawayListDecoder(giftScope, {})({ items: [gift], next_cursor: null }, { scope: giftScope, status: 200, path: giftPath })).toThrow()
    }
  })
}
