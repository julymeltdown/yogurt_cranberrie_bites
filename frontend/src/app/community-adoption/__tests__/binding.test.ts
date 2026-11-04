import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createCommunityEntry } from '../../community-entry'
import { bootstrap, fixture, testProfile } from './fixtures'
import { createCommunityOwner } from '../owner'
const cleanups: (() => void)[] = []
beforeEach(() => { vi.useFakeTimers(); vi.stubGlobal('location', { origin: 'https://community.test' }) })
afterEach(() => { cleanups.splice(0).reverse().forEach(fn => fn()); vi.unstubAllEnvs(); vi.unstubAllGlobals(); vi.useRealTimers() })
const setup = async (caps = false, physical = false, profile: typeof testProfile | undefined = testProfile) => {
  const f = fixture(await bootstrap(caps, physical), profile); cleanups.push(f.dispose); return f
}
describe('the original Bootstrap request lifetime', () => {
  it('captures the full RTT around one canonical C03 load and retains the original signal after privacy.run finishes', async () => {
    const f = await setup(), binding = await f.load(), authority = binding.read()!
    expect(f.requests.filter(r => new URL(r.url).pathname.endsWith('/bootstrap'))).toHaveLength(1)
    expect(authority.readInterval()!.roundTripMs).toBe(500)
    expect(authority.readInterval()!.upperMs).toBe(Date.parse(binding.source.document!.server_time as string) + 1000 + 1000 + 505)
    expect(binding.signal).toBe(f.lifetime.signal)
    const withdrawn = vi.fn(); authority.clock.subscribe(withdrawn)
    f.lifetime.abort(); expect(withdrawn).toHaveBeenCalledOnce(); expect(binding.read()).toBeNull()
    expect(vi.getTimerCount()).toBe(0)
  })
  it.each(['identity', 'policy', 'dispose'] as const)('retires the original binding on %s without recapturing its scope', async kind => {
    const f = await setup(), binding = await f.load(), authority = binding.read()!, original = binding.scope
    if (kind === 'identity') f.auth.session.replace(f.scope.generation, { userId: f.scope.eventId!, eventId: f.scope.userId!, accessToken: 'SYNTHETIC_ONLY' })
    if (kind === 'policy') f.auth.privacy.changePolicy(f.scope, 'block')
    if (kind === 'dispose') binding.dispose()
    expect(binding.scope).toBe(original); expect(binding.read()).toBeNull(); expect(authority.clock.read()).toBeNull()
  })
  it('does not allocate a clock for absent policy and rejects a second decoded receipt', async () => {
    // Omit policy explicitly; a default profile must never be inferred.
    const noPolicy = fixture(await bootstrap(true, true), undefined); cleanups.push(noPolicy.dispose)
    const binding = await noPolicy.load()
    expect(noPolicy.timelineCalls()).toBe(0); expect(binding.read()).toBeNull()
    expect(() => noPolicy.timing.complete(binding.source)).toThrow(/already consumed/)
  })
  it('rejects an explicitly TEST_ONLY clock profile in production mode', async () => {
    vi.stubEnv('PROD', true)
    const f = await setup(), binding = await f.load()
    expect(f.timelineCalls()).toBe(0); expect(binding.read()).toBeNull()
  })
  it('does not dispatch after an original-signal abort before the request', async () => {
    const f = await setup()
    f.lifetime.abort()
    await expect(f.load()).rejects.toThrow()
    expect(f.requests).toHaveLength(0); expect(vi.getTimerCount()).toBe(0)
  })
  it('cleans timing on a canonical decode failure', async () => {
    const f = fixture(await bootstrap(), testProfile, 'invalid'); cleanups.push(f.dispose)
    await expect(f.load()).rejects.toThrow()
    expect(f.requests).toHaveLength(1); expect(vi.getTimerCount()).toBe(0)
  })
  it('rejects an ignored-abort late HTTP body without adopting a new clock or scope', async () => {
    const f = fixture(await bootstrap(), testProfile, 'held'); cleanups.push(f.dispose)
    const pending = f.load(), rejection = expect(pending).rejects.toThrow()
    await vi.waitFor(() => expect(f.requests).toHaveLength(1))
    f.lifetime.abort(); await rejection
    f.releaseResponse(); await Promise.resolve(); await Promise.resolve()
    expect(vi.getTimerCount()).toBe(0); expect(f.requests).toHaveLength(1)
  })
})
describe('one borrowed discovery pair and the real Community bridge', () => {
  it('keeps canonical World-only capabilities unavailable for authoring despite a qualified test clock', async () => {
    const f = await setup(), binding = await f.load(), owner = f.owner(binding); cleanups.push(owner.dispose)
    const entry = createCommunityEntry({ auth: f.auth, release: owner.release }); cleanups.push(entry.dispose)
    const author = entry.partyAuthor()
    expect(author?.read().access ?? false).toBe(false)
    expect(binding.read()!.publicPlaces.read()!.items[0]!.place_kind).toBe('VIRTUAL_CONCEPT')
    expect(binding.read()!.places.read()!.items).toEqual([])
    expect(f.requests.every(r => r.method === 'GET')).toBe(true)
  })
  it('borrows the exact pair, preserves a draft across selections, and survives owner StrictMode replay without disposing parent timing', async () => {
    const f = await setup(true, true), binding = await f.load()
    const first = f.owner(binding); first.dispose()
    expect(binding.read()!.clock.read()).not.toBeNull()
    const owner = f.owner(binding); cleanups.push(owner.dispose)
    const entry = createCommunityEntry({ auth: f.auth, release: owner.release }); cleanups.push(entry.dispose)
    expect(entry.borrowed()).toEqual({ parties: owner.parties, gifts: owner.gifts })
    const author = entry.partyAuthor()!
    expect(author.read().access).toBe(true); expect(author.places()).toHaveLength(1)
    author.edit({ ...author.read().draft, title: 'Retained draft' })
    expect(entry.partyAuthor()).toBe(author); expect(entry.partyAuthor()!.read().draft.title).toBe('Retained draft')
    f.lifetime.abort(); expect(entry.read().phase).toBe('withdrawn'); expect(owner.gifts.read().context).toBeNull()
  })
  it('disables all action bridges with no profile and never uses the device calendar as a fallback', async () => {
    const f = fixture(await bootstrap(true, true), undefined); cleanups.push(f.dispose)
    const binding = await f.load(), owner = f.owner(binding); cleanups.push(owner.dispose)
    const entry = createCommunityEntry({ auth: f.auth, release: owner.release }); cleanups.push(entry.dispose)
    expect(owner.release).toBeNull(); expect(entry.partyAuthor()).toBeNull()
    expect(owner.gifts.read().context).toBeNull()
    expect(owner.parties.applyReason()).toMatch(/unavailable/)
  })
  it('rejects a relabeled capability projection rather than upgrading the canonical decision', async () => {
    const f = await setup(), binding = await f.load(), changed = await bootstrap(true, true)
    const owner = createCommunityOwner({ auth: f.auth, bootstrap: changed, scope: f.scope, binding }); cleanups.push(owner.dispose)
    expect(owner.release).toBeNull(); expect(owner.parties.applyReason()).toMatch(/unavailable/)
  })
  it('withdraws the bridge and discovery projections at clock discontinuity and retains no physical arrival permission', async () => {
    const f = await setup(true, true), binding = await f.load(), owner = f.owner(binding); cleanups.push(owner.dispose)
    const entry = createCommunityEntry({ auth: f.auth, release: owner.release }); cleanups.push(entry.dispose)
    const clock = binding.read()!.clock
    entry.partyAuthor(); f.setTime(599)
    expect(clock.read()).toBeNull(); expect(binding.read()).toBeNull(); expect(entry.read().phase).toBe('withdrawn')
    expect(owner.gifts.read().context).toBeNull(); expect(owner.release).not.toHaveProperty('pickupPlace')
    expect(owner.release).not.toHaveProperty('rally'); expect(owner.release).not.toHaveProperty('pickupMeeting')
  })
})
