import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { createMeetingEntry } from '../bridge'
import { entryFixture, beaconId } from './fixtures'
import { deferred } from '../../../features/meet-beacon/__tests__/fixtures/harness'

const owners: Awaited<ReturnType<typeof entryFixture>>[] = []
async function setup() { const f = await entryFixture(); owners.push(f); return f }
beforeEach(() => vi.stubGlobal('location', { origin: 'https://mcr.example.invalid' }))
afterEach(() => { owners.splice(0).forEach(f => f.dispose()); vi.unstubAllGlobals() })

it('does not fabricate authority or make requests for an absent runtime', async () => {
  const f = await setup(), count = f.h.requests.length, missing = f.create(null), noAuth = createMeetingEntry({ auth: null, release: f.release })
  expect(missing.read()).toMatchObject({ phase: 'unavailable', controller: null })
  expect(noAuth.read()).toMatchObject({ phase: 'unavailable', controller: null })
  expect(f.h.requests).toHaveLength(count); expect(f.subscriptions()).toEqual({ clock: 0, authority: 0, places: 0, invalidations: 0 })
  noAuth.dispose()
})
it('composes the committed feature with actual FE19 and canonical HTTP without eagerly reading a beacon', async () => {
  const f = await setup(), count = f.h.requests.length, entry = f.create()
  expect(entry.read().phase).toBe('available'); expect(f.h.requests).toHaveLength(count)
  const c = entry.read().controller!; await c.select(beaconId)
  expect(c.read().beacon?.meeting.place_revision).toBe(3); expect(c.read().phase).toBe('current')
  expect(f.subscriptions()).toEqual({ clock: 1, authority: 1, places: 1, invalidations: 1 })
  const r = f.h.requests.at(-1)!; expect(r.cache).toBe('no-store'); expect(r.headers.get('Authorization')).toBe('Bearer synthetic-memory-token')
})
it.each(['clock', 'clock-scope', 'authority', 'authority-scope'] as const)('fails closed for missing/mismatched %s and can reopen only under the captured lease', async kind => {
  const f = await setup()
  if (kind === 'clock') f.setClock(false)
  if (kind === 'clock-scope') f.setClockScope({ ...f.scope, policyGeneration: 999 })
  if (kind === 'authority') f.h.setAuthority(null)
  if (kind === 'authority-scope') f.h.setAuthority({ ...f.h.authority!, scope: { ...f.scope, eventId: '00000000-0000-4000-8000-000000000999' } })
  const entry = f.create(); expect(entry.read()).toMatchObject({ phase: 'unavailable', controller: null })
})
it('scrubs private details when the server clock is withdrawn and reuses the same feature owner when it returns', async () => {
  const f = await setup(), entry = f.create(), c = entry.read().controller!; await c.select(beaconId)
  f.setClock(false); expect(entry.read().controller).toBeNull(); expect(c.read().beacon).toBeNull()
  f.setClock(true); expect(entry.read().controller).toBe(c); expect(c.read().beacon).toBeNull()
  await c.select(beaconId); expect(c.read().phase).toBe('current')
})
it('withdraws permanently on privacy/identity changes without restamping old grants', async () => {
  const f = await setup(), entry = f.create(), c = entry.read().controller!; await c.select(beaconId)
  f.h.auth.privacy.changePolicy(f.scope, 'block')
  expect(entry.read()).toMatchObject({ phase: 'withdrawn', controller: null }); expect(c.read().beacon).toBeNull()
  expect(f.subscriptions()).toEqual({ clock: 0, authority: 0, places: 0, invalidations: 0 })
  f.h.setAuthority({ ...f.h.authority!, scope: f.h.auth.privacy.capture() }); f.notifyAuthority()
  expect(entry.read().phase).toBe('withdrawn')
})
it('aborts pending reads and disposes listeners on external shell withdrawal, ignoring noncooperative completion', async () => {
  const f = await setup(), entry = f.create(), c = entry.read().controller!, response = deferred<Response>()
  f.h.setOverride(r => r.url.includes('/beacons/') ? response.promise : null)
  const pending = c.select(beaconId)
  await vi.waitFor(() => expect(f.h.requests.some(r => r.url.includes('/beacons/'))).toBe(true))
  f.lifetime.abort(); expect(entry.read().phase).toBe('withdrawn')
  response.resolve(Response.json(f.h.beacon)); await pending
  expect(c.read().beacon).toBeNull(); expect(f.subscriptions()).toEqual({ clock: 0, authority: 0, places: 0, invalidations: 0 })
})
it('uses scoped invalidation metadata only and clears moved/revoked guidance', async () => {
  const f = await setup(), entry = f.create(), c = entry.read().controller!; await c.select(beaconId)
  f.invalidate({ scope: { ...f.scope, policyGeneration: 999 }, beaconId, revision: 2, state: 'REVOKED' })
  expect(c.read().phase).toBe('current')
  f.invalidate({ scope: f.scope, beaconId, revision: 2 }); expect(c.read()).toMatchObject({ phase: 'moved', beacon: null })
  f.invalidate({ scope: f.scope, beaconId, revision: 3, state: 'REVOKED' }); expect(c.read().phase).toBe('revoked')
})
it('does not derive a place revision from the catalog when the public per-place projection is missing', async () => {
  const f = await setup(); f.h.setAuthority({ ...f.h.authority!, placeRevisions: [] })
  const count = f.h.requests.length, entry = f.create()
  expect(entry.read()).toMatchObject({ phase: 'unavailable', controller: null }); expect(f.h.requests).toHaveLength(count)
  f.h.setAuthority({ ...f.h.authority!, placeRevisions: [{ placeId: f.h.items[1]!.place_id, revision: 3 }] }); f.notifyAuthority()
  expect(entry.read().phase).toBe('available')
})
it('retains unknown-command locks in the same owner through clock suspension instead of replaying', async () => {
  const f = await setup(), entry = f.create(), c = entry.read().controller!
  c.setDraft({ placeId: f.h.items[1]!.place_id, audienceId: 'synthetic-party', instructions: 'SYNTHETIC marker', identification: 'SYNTHETIC scarf', ttlSeconds: 60, acknowledged: true })
  f.h.setMode('lost'); await c.create(); expect(c.read().command.phase).toBe('uncertain')
  f.setClock(false); f.setClock(true); expect(entry.read().controller).toBe(c)
  expect(await c.create()).toBe(false); expect(f.h.requests.filter(r => r.method === 'POST')).toHaveLength(1)
})
it('cleans up partial registration failures without retaining feature subscriptions', async () => {
  const f = await setup(), release = { ...f.release, places: { ...f.release.places, subscribe() { throw new Error('Synthetic registration failure') } } }
  const entry = f.create(release); expect(entry.read().phase).toBe('withdrawn')
  expect(f.subscriptions()).toEqual({ clock: 0, authority: 0, places: 0, invalidations: 0 })
})
it('does not close the existing FE19 owner or canonical Auth when the adapter disposes', async () => {
  const f = await setup(), entry = f.create(); entry.dispose(); entry.dispose()
  expect(f.h.placesController.read().list.phase).toBe('ready'); expect(f.h.auth.session.read().userId).toBe(f.scope.userId)
  expect(f.subscriptions()).toEqual({ clock: 0, authority: 0, places: 0, invalidations: 0 })
})
it('still clears presentation and other sources if one unsubscribe throws', async () => {
  const f = await setup(), release = { ...f.release, authority: { ...f.release.authority,
    subscribe(listener: () => void) { const stop = f.release.authority.subscribe(listener); return () => { stop(); throw new Error('Synthetic cleanup failure') } } } }
  const entry = f.create(release), c = entry.read().controller!; await c.select(beaconId)
  expect(() => entry.dispose()).not.toThrow(); expect(entry.read()).toMatchObject({ phase: 'withdrawn', controller: null })
  expect(c.read().beacon).toBeNull(); expect(f.subscriptions()).toEqual({ clock: 0, authority: 0, places: 0, invalidations: 0 })
})
