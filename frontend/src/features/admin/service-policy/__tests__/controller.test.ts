// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { policyDecision, policyInput, policyReceipt } from '../contract'
import { actor, epoch, eventId, fixture, time, uid, validDraft } from './fixture'
const owners: ReturnType<typeof fixture>[] = []
afterEach(() => { owners.splice(0).forEach(owner => owner.dispose()); vi.useRealTimers() })
async function setup() {
  const owner = fixture(); owners.push(owner); owner.installUnitIdentity(); await owner.initialize(); await owner.controller.refresh(); return owner
}
const puts = (owner: ReturnType<typeof fixture>) => owner.requests.filter(r => r.method === 'PUT')
function prepare(owner: ReturnType<typeof fixture>) { owner.controller.edit(validDraft); expect(owner.controller.review()).toBe(true) }
describe('FE38 policy through canonical Auth, actual OperationsEntry and borrowed guard', () => {
  it('reads the six fields through the registered operation; cancel preserves the draft without a write', async () => {
    const owner = await setup(), c = owner.controller
    expect(c.read().access).toBe('ready'); expect(c.read().decision?.policy_revision).toBe(4)
    prepare(owner); expect(c.read().command?.input.expected_policy_revision).toBe(4)
    c.cancel(); expect(c.read().draft).toEqual(validDraft); expect(puts(owner)).toHaveLength(0)
  })
  it('publishes once and displays event revision distinct from policy revision; never treats receipt as decision', async () => {
    const owner = await setup(), c = owner.controller; prepare(owner); await c.confirm(); await c.confirm()
    expect(c.read().command?.stage).toBe('succeeded'); expect(c.read().command?.receipt?.resource_revision).toBe(22)
    expect(c.read().decision).toBeNull(); expect(puts(owner)).toHaveLength(1)
    await c.refresh(); expect(c.read().decision?.policy_revision).toBe(5); c.dismiss(); expect(c.read().command).toBeNull()
  })
  it('withdraws using only WITHDRAW fields and known revision; unavailable never revives older normal', async () => {
    const owner = await setup(), c = owner.controller
    c.edit({ action: 'WITHDRAW', reason: 'Withdraw unsafe publication' }); expect(c.review()).toBe(true); await c.confirm(); await c.refresh()
    expect((puts(owner)[0]!.body as { change: unknown }).change).toEqual({ action: 'WITHDRAW' })
    expect(c.read().decision).toMatchObject({ kind: 'UNAVAILABLE', policy_revision: 5, minimum_client_version: null })
  })
  it('supports explicit first publication CAS zero only when history is absent', async () => {
    const owner = await setup(); owner.current({ policyRevision: null, kind: 'UNAVAILABLE', minimum: null })
    // A new scope with no prior history, rather than accepting a regression of an observed policy.
    owner.dispose(); const first = fixture(); owners.push(first); first.current({ policyRevision: null, kind: 'UNAVAILABLE', minimum: null })
    first.installUnitIdentity(); await first.initialize(); await first.controller.refresh(); prepare(first)
    expect(first.controller.read().command?.input.expected_policy_revision).toBe(0)
  })
  it('retains a draft on a terminal FAILED conflict receipt; requires current read and explicit review', async () => {
    const owner = await setup(), c = owner.controller; prepare(owner); owner.mode('conflict'); await c.confirm()
    expect(c.read().command?.stage).toBe('conflict'); expect(c.read().draft).toEqual(validDraft)
    c.dismiss(); expect(c.read().command).not.toBeNull(); owner.mode('success'); await c.refresh(); c.dismiss(); expect(c.review()).toBe(true)
    expect(c.read().command?.input.expected_policy_revision).toBe(5); expect(puts(owner)).toHaveLength(1)
  })
  it('keeps 202 pending locked across refresh/remount-style subscription; status resolves without replay', async () => {
    const owner = await setup(), c = owner.controller; prepare(owner); owner.mode('pending'); await c.confirm()
    expect(c.read().command?.stage).toBe('pending'); const stop = c.subscribe(() => {}); stop()
    await c.refresh(); c.dismiss(); c.edit({ reason: 'must not replace unresolved input' }); expect(c.review()).toBe(false)
    await c.checkStatus(); expect(c.read().command?.stage).toBe('pending')
    owner.settle(); await c.checkStatus(); expect(c.read().command?.stage).toBe('succeeded'); expect(puts(owner)).toHaveLength(1)
  })
  it('recovers a lost-after-commit response by command status, with no retry or replacement key', async () => {
    const owner = await setup(), c = owner.controller; prepare(owner); owner.mode('lost'); await c.confirm()
    expect(c.read().command?.stage).toBe('unknown'); await c.refresh(); expect(c.review()).toBe(false)
    await c.checkStatus(); expect(c.read().command?.stage).toBe('succeeded'); expect(puts(owner)).toHaveLength(1)
  })
  it('leaves missing/expired status unavailable without leaking a receipt or replaying', async () => {
    const owner = await setup(), c = owner.controller; prepare(owner); owner.mode('lost'); await c.confirm(); owner.setReceipt(null)
    await c.checkStatus(); expect(c.read().command?.stage).toBe('unknown'); expect(c.read().message).toContain('unresolved'); expect(puts(owner)).toHaveLength(1)
  })
  it('rejects foreign write receipt and later uses correctly bound status', async () => {
    const owner = await setup(), c = owner.controller; prepare(owner); owner.mode('malformed'); await c.confirm()
    expect(c.read().command?.stage).toBe('unknown'); await c.checkStatus(); expect(c.read().command?.stage).toBe('succeeded')
  })
  it('rejects swapped receipt ID and retained-time changes while pending', async () => {
    const owner = await setup(), c = owner.controller; prepare(owner); owner.mode('pending'); await c.confirm()
    owner.setReceipt({ ...c.read().command!.receipt!, receipt_id: uid(999) }); await c.checkStatus()
    expect(c.read().command?.stage).toBe('pending'); expect(c.read().message).toContain('unresolved')
  })
  it('does not turn service errors or read-only rejection into policy decisions', async () => {
    const owner = await setup(), c = owner.controller; prepare(owner); owner.mode('read-only'); await c.confirm()
    expect(c.read().command?.stage).toBe('failed'); expect(c.read().decision).toBeNull()
    owner.mode('unavailable'); await c.refresh(); expect(c.read().decision).toBeNull(); expect(c.read().draft).toEqual(validDraft)
  })
  it('withdraws all drafts and command state synchronously when real access entry withdraws', async () => {
    const owner = await setup(), c = owner.controller; prepare(owner); owner.withdraw()
    expect(c.read()).toMatchObject({ access: 'withdrawn', decision: null, command: null, draft: { reason: '' } }); await c.confirm(); expect(puts(owner)).toHaveLength(0)
  })
  it.each(['identity', 'event', 'privacy'] as const)('scrubs %s changes and prevents late arrivals', async change => {
    const owner = await setup(), c = owner.controller; prepare(owner)
    let release!: (response: Response) => void
    owner.override(request => request.method === 'PUT' ? new Promise<Response>(resolve => { release = resolve }) : null)
    const sending = c.confirm(); await vi.waitFor(() => expect(release).toBeTypeOf('function'))
    if (change === 'privacy') owner.auth.privacy.changePolicy(owner.auth.privacy.capture(), 'consent-revoked')
    else owner.auth.session.replace(owner.auth.session.read().generation, { userId: change === 'identity' ? uid(33) : actor, eventId: change === 'event' ? uid(34) : eventId, accessToken: 'other-test-token' })
    release(Response.json(owner.receipt(uid(100), 'SUCCEEDED'))); await sending
    expect(c.read()).toMatchObject({ access: 'withdrawn', command: null, draft: { reason: '' } })
  })
  it('does not grant operator controls from moderator or absent role responses', async () => {
    for (const role of ['MODERATOR', null] as const) {
      const owner = fixture(); owners.push(owner); owner.role(role); owner.installUnitIdentity(); await owner.initialize(); await owner.controller.refresh()
      expect(owner.controller.read().access).toBe('withdrawn'); expect(owner.requests.filter(r => r.path.endsWith('/service-policy'))).toHaveLength(0)
    }
  })
  it('rejects stale decisions, backward revisions and same-revision changed immutable minimum', async () => {
    const owner = await setup(), c = owner.controller
    owner.current({ policyRevision: 3 }); await c.refresh(); expect(c.read().decision).toBeNull()
    owner.current({ policyRevision: 4, minimum: '3.0.0' }); await c.refresh(); expect(c.read().decision).toBeNull()
    owner.current({ policyRevision: 5, minimum: '3.0.0' }); await c.refresh(); expect(c.read().decision?.policy_revision).toBe(5)
  })
  it('expires decision before confirmation, preserving draft and issuing no write', async () => {
    const owner = await setup(), c = owner.controller; prepare(owner); owner.advance(30_001); await c.confirm()
    expect(c.read().command).toBeNull(); expect(c.read().draft).toEqual(validDraft); expect(puts(owner)).toHaveLength(0)
  })
  it('revalidates publication expiry before sending', async () => {
    const owner = await setup(), c = owner.controller; c.edit({ ...validDraft, expiresAt: time(1000) }); expect(c.review()).toBe(true)
    owner.advance(1001); await c.confirm(); expect(c.read().command).toBeNull(); expect(puts(owner)).toHaveLength(0)
  })
  it('forbidden write withdraws access without rendering server diagnostics', async () => {
    const owner = await setup(), c = owner.controller; prepare(owner); owner.mode('forbidden'); await c.confirm()
    expect(c.read().access).toBe('withdrawn'); expect(JSON.stringify(c.read())).not.toContain('Private diagnostic')
  })
})
describe('strict frozen policy validation and additional bindings', () => {
  it.each(['01.0.0', '1.2.3\n', '65536.0.0', '1.2', '1.2.3 '])('rejects noncanonical version %j', version => {
    expect(() => policyInput({ expected_policy_revision: 4, reason: 'Reason', change: { action: 'PUBLISH', mode: 'NORMAL', minimum_client_version: version, expires_at: time(1000) } }, epoch)).toThrow()
  })
  it.each([' Reason', 'Reason ', '\nReason', 'x'.repeat(501), '界'.repeat(167)])('rejects invalid or byte-overlong reason', reason => {
    expect(() => policyInput({ expected_policy_revision: 4, reason, change: { action: 'WITHDRAW' } }, epoch)).toThrow()
  })
  it('rejects an overlong decision horizon and unknown source metadata', () => {
    const owner = fixture(); owners.push(owner)
    expect(() => policyDecision({ ...owner.decision(), valid_until: time(30_001) }, epoch)).toThrow()
    expect(() => policyDecision({ ...owner.decision(), reviewed: true }, epoch)).toThrow()
  })
  it('binds successful resource revision to event history, independently from policy CAS', () => {
    const owner = fixture(); owners.push(owner)
    const binding = { actor, event: eventId, key: uid(100), eventRevision: 18 }
    expect(policyReceipt(owner.receipt(uid(100), 'SUCCEEDED'), binding, epoch).resource_revision).toBe(19)
    expect(() => policyReceipt(owner.receipt(uid(100), 'SUCCEEDED'), { ...binding, eventRevision: 19 }, epoch)).toThrow()
  })
})
