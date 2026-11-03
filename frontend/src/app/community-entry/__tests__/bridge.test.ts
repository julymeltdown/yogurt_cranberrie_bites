// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { createCommunityEntry } from '../bridge'
import { fixture, giftId, reservationId, ids, place } from './fixture'

const cleanups: (() => void)[] = []
afterEach(() => { cleanups.splice(0).reverse().forEach(fn => fn()) })
function setup() { const f = fixture(), entry = createCommunityEntry({ auth: f.auth, release: f.release }); cleanups.push(() => { entry.dispose(); f.dispose() }); return { f, entry } }
describe('community entry lease and command ownership', () => {
  it('borrows exact discovery controllers without starting requests or disposing them', () => {
    const { f, entry } = setup(), parties = vi.spyOn(f.parties, 'dispose'), gifts = vi.spyOn(f.gifts, 'dispose')
    expect(entry.borrowed()).toEqual({ parties: f.parties, gifts: f.gifts }); expect(f.requests).toHaveLength(0)
    entry.dispose(); entry.dispose(); expect(parties).not.toHaveBeenCalled(); expect(gifts).not.toHaveBeenCalled()
  })
  it('retains authoring/management owners across panel changes and rejects malformed IDs', () => {
    const { entry } = setup(), a = entry.partyAuthor(), m = entry.partyManager(ids.party), g = entry.giveawayAuthor()
    expect(a).not.toBeNull(); expect(m).not.toBeNull(); expect(g).not.toBeNull()
    expect(entry.partyAuthor()).toBe(a); expect(entry.partyManager(ids.party)).toBe(m); expect(entry.giveawayAuthor()).toBe(g)
    expect(entry.partyAuthor('../bad')).toBeNull(); expect(entry.partyManager('')).toBeNull(); expect(entry.giftPickup('bad')).toBeNull()
  })
  it('requires qualified scoped time and never defaults to local wall-clock', () => {
    const { f } = setup(), { clock: _clock, ...release } = f.release
    const entry = createCommunityEntry({ auth: f.auth, release }); cleanups.push(() => entry.dispose())
    expect(entry.read().phase).toBe('unavailable'); expect(entry.partyAuthor()).toBeNull(); expect(entry.giftPickup(giftId)).toBeNull()
    expect(f.requests).toHaveLength(0)
  })
  it('withdraws all owned controllers synchronously on lease abort', () => {
    const { f, entry } = setup(), author = entry.partyAuthor()!, manager = entry.partyManager(ids.party)!, gift = entry.giveawayAuthor()!
    const spies = [author, manager, gift].map(c => vi.spyOn(c, 'dispose'))
    f.lifetime.abort(); expect(entry.read().phase).toBe('withdrawn'); expect(entry.borrowed()).toBeNull()
    spies.forEach(spy => expect(spy).toHaveBeenCalledOnce()); expect(entry.partyAuthor()).toBeNull()
    expect(f.subscriptions().clock).toBe(0)
  })
  it('rejects a stale scope without allocating resources', () => {
    const { f } = setup(), entry = createCommunityEntry({ auth: f.auth, release: { ...f.release, scope: { ...f.scope, policyGeneration: 99 } } })
    cleanups.push(() => entry.dispose()); expect(entry.partyAuthor()).toBeNull(); expect(entry.borrowed()).toBeNull()
  })
  it('does not adopt replacement Auth identity', () => {
    const { f, entry } = setup(); entry.partyAuthor()
    f.auth.session.replace(f.auth.session.read().generation, { userId: ids.other, eventId: f.scope.eventId!, accessToken: 'new-fixture' })
    expect(entry.read().phase).toBe('withdrawn'); expect(entry.partyManager(ids.party)).toBeNull()
  })
  it('hides wrong-scope places and never synthesizes a fallback', () => {
    const { f, entry } = setup(), author = entry.partyAuthor()!
    expect(author.places().map(p => p.id)).toContain(place)
    f.places({ ...f.scope, policyGeneration: 99 }); expect(author.places()).toEqual([])
  })
  it('physical rally remains unavailable without qualified authority', async () => {
    const { f, entry } = setup(), manager = entry.partyManager(ids.party)!
    await manager.load(); expect(manager.meeting()).toBeNull(); await manager.report('HERE', true)
    expect(f.requests.every(r => r.method === 'GET')).toBe(true)
  })
  it('keeps one pickup writer and blocks replacing its unfinished journal', async () => {
    const { f, entry } = setup(), pickup = entry.giftPickup(giftId)!
    await pickup.load(); f.loseResponse(); await pickup.reserve()
    expect(pickup.read().phase).toBe('uncertain'); expect(f.journal.read(f.scope.userId!, f.scope.eventId!)).not.toBeNull()
    expect(entry.giftPickup(giftId)).toBe(pickup); expect(entry.giftPickup(undefined, reservationId)).toBeNull()
    await pickup.checkStatus(); expect(pickup.read().phase).toBe('pending')
    expect(f.requests.filter(r => r.method !== 'GET')).toHaveLength(1)
  })
  it('clock withdrawal retains unresolved command metadata for a new qualified release', async () => {
    const { f, entry } = setup(), pickup = entry.giftPickup(giftId)!
    await pickup.load(); f.loseResponse(); await pickup.reserve(); const saved = f.journal.read(f.scope.userId!, f.scope.eventId!)
    f.clock(false); expect(entry.read().phase).toBe('withdrawn'); expect(f.journal.read(f.scope.userId!, f.scope.eventId!)).toEqual(saved)
    f.clock(true); const next = createCommunityEntry({ auth: f.auth, release: f.release }); cleanups.push(() => next.dispose())
    const restored = next.giftPickup(giftId)!; expect(restored.read().phase).toBe('uncertain'); await restored.checkStatus()
    expect(restored.read().phase).toBe('pending'); expect(f.requests.filter(r => r.method !== 'GET')).toHaveLength(1)
  })
  it('privacy revocation scrubs old-scope journal as well as UI', async () => {
    const { f, entry } = setup(), pickup = entry.giftPickup(giftId)!
    await pickup.load(); f.loseResponse(); await pickup.reserve(); f.auth.privacy.changePolicy(f.scope, 'block')
    expect(entry.read().phase).toBe('withdrawn'); expect(f.journal.read(f.scope.userId!, f.scope.eventId!)).toBeNull()
    expect(pickup.read().gift).toBeNull()
  })
  it('does not allocate when the two context projections disagree', () => {
    const { f, entry } = setup(), context = f.release.context.read()!
    f.context({ ...context, eventPhase: 'ARCHIVED' }); expect(entry.read().phase).toBe('unavailable'); expect(entry.giveawayAuthor()).toBeNull()
  })
  it('allows a different pickup target only after the previous outcome is known', async () => {
    const { entry } = setup(), first = entry.giftPickup(giftId)!
    await first.load(); const dispose = vi.spyOn(first, 'dispose')
    const second = entry.giftPickup(undefined, reservationId); expect(second).not.toBe(first); expect(dispose).toHaveBeenCalledOnce()
  })
  it('keeps private physical pickup guidance hidden without authority', async () => {
    const { entry } = setup(), pickup = entry.giftPickup(undefined, reservationId)!
    await pickup.load(); await pickup.loadPickup(); expect(pickup.read().pickup).toBeNull(); expect(pickup.physicalReady()).toBe(false)
  })
})
