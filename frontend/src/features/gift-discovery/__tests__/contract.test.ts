import { describe, expect, it } from 'vitest'
import { giftActivityDecoder, giveawayDecoder, giveawayListDecoder, validateFilters } from '../model/contract'
import { actorId, eventId, giveaway, reservation, uid } from './fixtures/data'

const scope = { userId: actorId, eventId, generation: 1 }
const context = (tail: string) => ({ scope, status: 200, path: `/api/concert/events/${eventId}/${tail}` })
const decodeList = (items: unknown[], query = {}) => giveawayListDecoder(scope, query)({ items, next_cursor: null }, context('giveaways'))
const activity = () => ({ giving: { items: [giveaway({ owner: { ...giveaway().owner, user_id: actorId }, state: 'DRAFT' })], next_cursor: 'giving_next' },
  receiving: { items: [reservation()], next_cursor: 'receiving_next' } })
describe('FE24 frozen browser contract decoders', () => {
  it('accepts the closed public projection and deeply freezes independent activity pages including own drafts', () => {
    const list = decodeList([giveaway()]); expect(Object.isFrozen(list.items[0]?.owner.avatar.config)).toBe(true)
    expect(giftActivityDecoder(scope, {})(activity(), context('me/giveaways'))).toEqual(activity())
  })
  it.each([
    { state: 'DRAFT' }, { event_id: uid(9) }, { available: 4 }, { available: -1 }, { revision: 0 }, { revision: 1.5 },
    { quantity_total: 10001 }, { recipients: [actorId] }, { pickup_code: '123456' }, { pickup_ends_at: '2026-11-07T08:00:00Z' },
    { pickup_starts_at: '2026-02-30T08:00:00Z' }, { created_at: '2026-11-07T09:00:00Z' }, { title: '\ud800' },
    { title: 'x'.repeat(121) }, { image_media_id: 'https://invalid.example/img.png' },
  ])('rejects invalid or private public fields: %j', patch => expect(() => decodeList([{ ...giveaway(), ...patch }])).toThrow())
  it.each(['email', 'social_handles', 'age', 'private_meeting'])('rejects private owner field %s', key => {
    expect(() => decodeList([{ ...giveaway(), owner: { ...giveaway().owner, [key]: 'secret' } }])).toThrow()
  })
  it('accepts scalar text length in Unicode code points rather than UTF-16 units', () => {
    expect(decodeList([giveaway({ title: '🎁'.repeat(120) })]).items).toHaveLength(1)
  })
  it('binds status, actual target, actor, generation, event and requested detail ID', () => {
    const decode = giveawayDecoder(scope, giveaway().giveaway_id), valid = context(`giveaways/${giveaway().giveaway_id}`)
    expect(decode(giveaway(), valid).giveaway_id).toBe(giveaway().giveaway_id)
    for (const bad of [{ ...valid, status: 202 }, { ...valid, path: valid.path + '/wrong' },
      { ...valid, scope: { ...scope, userId: uid(77) } }, { ...valid, scope: { ...scope, generation: 2 } }]) expect(() => decode(giveaway(), bad)).toThrow()
    expect(() => decode(giveaway({ giveaway_id: uid(77) }), valid)).toThrow()
  })
  it('checks exact filter semantics, strict bounds and cursor loops', () => {
    const row = giveaway()
    expect(() => decodeList([row], { state: 'PAUSED' })).toThrow()
    expect(() => decodeList([row], { in_stock: false })).toThrow()
    expect(() => decodeList([row], { public_place_id: uid(55) })).toThrow()
    expect(() => decodeList([row], { pickup_before: row.pickup_starts_at })).toThrow()
    expect(decodeList([row], { pickup_after: row.pickup_starts_at }).items).toHaveLength(1)
    expect(() => validateFilters({ pickup_after: row.pickup_starts_at, pickup_before: row.pickup_starts_at })).toThrow()
    expect(() => validateFilters({ state: 'DRAFT' } as never)).toThrow()
    expect(() => giveawayListDecoder(scope, { cursor: 'same' })({ items: [], next_cursor: 'same' }, context('giveaways'))).toThrow()
  })
  it('rejects duplicate IDs, incorrect order and pages exceeding requested limit', () => {
    const row = giveaway()
    expect(() => decodeList([row, row])).toThrow()
    expect(() => decodeList([row, giveaway({ giveaway_id: uid(99) })])).toThrow()
    expect(() => decodeList([giveaway({ giveaway_id: uid(99) }), row], { limit: 1 })).toThrow()
  })
  it.each(['giving-owner', 'receiving-owner', 'event', 'duplicate', 'receipt', 'times', 'private', 'duplicate-active'])('rejects invalid own activity %s', mode => {
    const value = activity()
    if (mode === 'giving-owner') value.giving.items = [giveaway()]
    if (mode === 'receiving-owner') value.receiving.items = [reservation({ recipient_id: uid(8) })]
    if (mode === 'event') value.receiving.items = [reservation({ event_id: uid(8) })]
    if (mode === 'duplicate') value.receiving.items.push(reservation())
    if (mode === 'receipt') value.receiving.items = [reservation({ state: 'CLAIMED' })]
    if (mode === 'times') value.receiving.items = [reservation({ expires_at: reservation().created_at })]
    if (mode === 'private') Object.assign(value.receiving.items[0]!, { confirmation_code: '123456' })
    if (mode === 'duplicate-active') value.receiving.items.unshift(reservation({ reservation_id: uid(88) }))
    expect(() => giftActivityDecoder(scope, {})(value, context('me/giveaways'))).toThrow()
  })
  it('permits empty continuations and valid claimed terminal receipts', () => {
    const value = { giving: { items: [], next_cursor: 'empty_more' }, receiving: { items: [reservation({ state: 'CLAIMED', claim_receipt_id: uid(123) })], next_cursor: null } }
    expect(giftActivityDecoder(scope, {})(value, context('me/giveaways'))).toEqual(value)
  })
})
