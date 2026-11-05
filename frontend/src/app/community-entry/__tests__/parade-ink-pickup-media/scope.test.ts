// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { createCommunityEntry } from '../../bridge'
import { pickupMediaScope, type PickupMediaLease } from '../../pickup-media'
import type { RootMediaBinding } from '../../../media-entry/provider'
import { fixture, giftId } from '../fixture'

const cleanup: (() => void)[] = []
afterEach(() => cleanup.splice(0).reverse().forEach(stop => stop()))
async function setup() {
  const f = fixture(), entry = createCommunityEntry({ auth: f.auth, release: f.release })
  cleanup.push(f.dispose, entry.dispose)
  const source = entry.giftPickup(giftId)!
  await source.load()
  // TEST_ONLY authorized parent projection; this test never creates or acquires media.
  const image = '00000000-0000-4000-8000-000000000088'
  const controller = { ...source, read: () => { const state = source.read(); return { ...state, gift: state.gift ? { ...state.gift, image_media_id: image } : null } } }
  const acquire = vi.fn(() => { throw Error('The bridge must not acquire media') })
  const binding: RootMediaBinding = { auth: f.auth, scope: f.release.scope,
    owner: { inspect: () => ({ disposed: false }), acquire } as unknown as RootMediaBinding['owner'] }
  const lease: PickupMediaLease = { auth: f.auth, release: f.release, entry }
  return { f, controller, binding, lease, acquire, request: { giveawayId: giftId, mediaId: image, alt: 'Test-only image' } }
}
it('returns the original matching entry scope without creating or acquiring an owner', async () => {
  const h = await setup()
  expect(pickupMediaScope(h.controller, h.lease, h.binding, h.request)).toBe(h.f.release.scope)
  expect(h.acquire).not.toHaveBeenCalled()
})
it('rejects a foreign Root auth even when its scope values match', async () => {
  const h = await setup(), other = fixture(); cleanup.push(other.dispose)
  expect(pickupMediaScope(h.controller, h.lease, { ...h.binding, auth: other.auth }, h.request)).toBeNull()
  expect(h.acquire).not.toHaveBeenCalled()
})
it.each(['generation', 'policyGeneration', 'userId', 'eventId', 'gate'] as const)('rejects Root scope replacement at %s', async field => {
  const h = await setup(), scope = { ...h.binding.scope, [field]: typeof h.binding.scope[field] === 'number' ? 999 : 'foreign' }
  expect(pickupMediaScope(h.controller, h.lease, { ...h.binding, scope: scope as RootMediaBinding['scope'] }, h.request)).toBeNull()
  expect(h.acquire).not.toHaveBeenCalled()
})
it('rejects entry withdrawal and an older actor lease rather than restamping it', async () => {
  const h = await setup(); h.f.lifetime.abort()
  expect(pickupMediaScope(h.controller, h.lease, h.binding, h.request)).toBeNull()
  expect(h.acquire).not.toHaveBeenCalled()
})
it('rejects missing owners, unavailable controllers and mismatched gift/media membership', async () => {
  const h = await setup()
  expect(pickupMediaScope(h.controller, h.lease, null, h.request)).toBeNull()
  expect(pickupMediaScope(h.controller, h.lease, h.binding, { ...h.request, mediaId: '' })).toBeNull()
  expect(pickupMediaScope(null, h.lease, h.binding, h.request)).toBeNull()
  expect(pickupMediaScope(h.controller, h.lease, h.binding, { ...h.request, giveawayId: '00000000-0000-4000-8000-000000000099' })).toBeNull()
  expect(pickupMediaScope(h.controller, h.lease, h.binding, { ...h.request, mediaId: '00000000-0000-4000-8000-000000000098' })).toBeNull()
  expect(h.acquire).not.toHaveBeenCalled()
})

it('rejects actor replacement and disposed media owners without acquiring new media', async () => {
  const h = await setup()
  expect(pickupMediaScope(h.controller, h.lease, { ...h.binding, owner: { inspect: () => ({ disposed: true }) } as RootMediaBinding['owner'] }, h.request)).toBeNull()
  h.f.auth.session.replace(h.f.auth.session.read().generation, { userId: '00000000-0000-4000-8000-000000000097', eventId: h.f.release.scope.eventId!, accessToken: 'TEST_ONLY-replacement' })
  expect(pickupMediaScope(h.controller, h.lease, h.binding, h.request)).toBeNull()
  expect(h.acquire).not.toHaveBeenCalled()
})
