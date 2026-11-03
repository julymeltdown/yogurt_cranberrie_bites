import { describe, expect, it } from 'vitest'
import contract from '../../../../../contracts/generated/mcr-domain.openapi.json'
import { decodeActivity, decodeCommand, decodeList, decodeParty, decodeReceipt } from '../model/contract'
import { activity, application, ids, party, pendingReceipt } from './fixtures/data'
const scope = { generation: 1, userId: ids.actor, eventId: ids.event }, path = `/api/concert/events/${ids.event}/parties`, context = { scope, status: 200, path }
const binding = { scope, operation: 'applyConcertParty' as const, key: ids.key, partyId: ids.party }
describe('FE21 frozen C04 response admission', () => {
  it('pins operation response schema and routes without adding endpoints', () => {
    expect(contract.paths['/api/concert/events/{eventId}/parties'].get.operationId).toBe('listConcertParties')
    expect(contract.paths['/api/concert/events/{eventId}/me/parties'].get.operationId).toBe('getConcertPartyActivity')
    expect(contract.paths['/api/concert/events/{eventId}/parties/{partyId}/applications'].post.responses[200].content['application/json'].schema.$ref).toBe('#/components/schemas/Domain_party_Application')
  })
  it('admits closed C03 profile/avatar nesting and copies/freezes responses', () => {
    const v = party(), result = decodeParty(ids.party, path)(v, context)
    expect(result).toEqual(v); expect(result).not.toBe(v); expect(Object.isFrozen(result.owner)).toBe(true)
  })
  it.each([
    { ...party(), hidden_member_email: 'private' }, { ...party(), member_count: 7 }, { ...party(), event_id: ids.other },
    { ...party(), party_id: ids.next }, { ...party(), meeting_ends_at: party().meeting_starts_at }, { ...party(), meeting_starts_at: '2026-02-30T00:00:00Z' },
    { ...party(), owner: { ...party().owner, avatar: { revision: 1, config: {} } } }, { ...party(), owner: { ...party().owner, user_id: 'invalid' } },
  ])('rejects private, malformed or cross-context detail %j', value => { expect(() => decodeParty(ids.party, path)(value, context)).toThrow() })
  it('rejects wrong request context, unexpected HTTP status, overbound or duplicate list entries', () => {
    const decode = decodeList(1, path)
    expect(() => decode({ items: [party()], next_cursor: null }, { ...context, status: 202 })).toThrow()
    expect(() => decode({ items: [party()], next_cursor: null }, { ...context, path: `${path}/other` })).toThrow()
    expect(() => decodeList(2, path)({ items: [party(), party()], next_cursor: null }, context)).toThrow()
    expect(() => decode({ items: [party(), party({ party_id: ids.next })], next_cursor: null }, context)).toThrow()
  })
  it('allows empty pages with opaque continuation; never infers completion from empty items', () => {
    expect(decodeList(20, path)({ items: [], next_cursor: 'opaque_next' }, context).next_cursor).toBe('opaque_next')
  })
  it('checks event and own identity across every independently bounded activity collection', () => {
    const a = activity(), d = decodeActivity(1, path)
    expect(() => d({ ...a, owned: { items: [party()], next_cursor: null } }, context)).toThrow()
    expect(() => d({ ...a, applications: { items: [application({ applicant: party().owner })], next_cursor: null } }, context)).toThrow()
    const m = { event_id: ids.event, party_id: ids.party, user_id: ids.actor, role: 'MEMBER', state: 'ACTIVE', revision: 1, joined_at: party().created_at }
    expect(() => d({ ...a, memberships: { items: [{ ...m, event_id: ids.other }], next_cursor: null } }, context)).toThrow()
    expect(() => d({ ...a, memberships: { items: [{ ...m, user_id: ids.other }], next_cursor: null } }, context)).toThrow()
    expect(() => d({ ...a, memberships: { items: [m, m], next_cursor: null } }, context)).toThrow()
    expect(decodeActivity(2, path)({ ...a, memberships: { items: [m, { ...m, party_id: ids.next }], next_cursor: null } }, context).memberships.items).toHaveLength(2)
  })
  it('binds command final application to actor, party, application and expected transition', () => {
    const d = decodeCommand(binding, path)
    expect(d(application(), context)).toEqual(application())
    for (const a of [application({ party_id: ids.next }), application({ event_id: ids.other }), application({ state: 'APPROVED' }), application({ applicant: party().owner })]) expect(() => d(a, context)).toThrow()
    const cancel = decodeCommand({ ...binding, operation: 'cancelConcertApplication', applicationId: ids.app }, path)
    expect(cancel(application({ state: 'CANCELLED', revision: 2 }), context)).toMatchObject({ state: 'CANCELLED' })
    expect(() => cancel(application({ application_id: ids.next, state: 'CANCELLED' }), context)).toThrow()
  })
  it.each([
    { actor_id: ids.other }, { event_id: ids.other }, { operation_id: 'cancelConcertApplication' }, { idempotency_key: ids.other },
    { resource_id: ids.app }, { resource_revision: 1 }, { result_status: 200 }, { failure_reason: 'PARTY_FULL' }, { expires_at: '2026-11-03T00:00:00Z' },
  ])('rejects receipt context or state-invariant breach %j', override => { expect(() => decodeReceipt(binding, path)(pendingReceipt(ids.key, override), context)).toThrow() })
  it('202 admits only PENDING; succeeded requires final 200, registered failure reasons per command', () => {
    const d = decodeCommand(binding, path), done = pendingReceipt(ids.key, { state: 'SUCCEEDED', resource_id: ids.app, resource_revision: 1, result_status: 200 })
    expect(d(pendingReceipt(), { ...context, status: 202 })).toMatchObject({ state: 'PENDING' })
    expect(() => d(done, { ...context, status: 202 })).toThrow()
    for (const result_status of [201, 202, 204]) expect(() => decodeReceipt(binding, path)({ ...done, result_status }, context)).toThrow()
    expect(decodeReceipt(binding, path)(done, context)).toEqual(done)
    expect(() => decodeReceipt(binding, path)(pendingReceipt(ids.key, { state: 'FAILED', failure_reason: 'GIVEAWAY_SOLD_OUT' }), context)).toThrow()
    expect(() => decodeReceipt({ ...binding, operation: 'cancelConcertApplication' }, path)(pendingReceipt(ids.key, { operation_id: 'cancelConcertApplication', state: 'FAILED', failure_reason: 'PARTY_FULL' }), context)).toThrow()
    expect(() => decodeReceipt({ ...binding, operation: 'cancelConcertApplication', applicationId: ids.next }, path)({ ...done, operation_id: 'cancelConcertApplication' }, context)).toThrow()
  })
})
