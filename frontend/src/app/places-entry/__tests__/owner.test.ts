import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { createPlacesEntry, type PlacesEntry } from '../owner'
import { entryFixture } from './fixture'
import { catalog, deferred, eventId, otherUser, places } from '../../../features/places/__tests__/fixtures/data'
const owned: (() => void)[] = []
beforeEach(() => { vi.stubGlobal('location', { origin: 'https://mcr.example.invalid' }) })
afterEach(() => { owned.splice(0).reverse().forEach(stop => stop()); vi.unstubAllGlobals() })
async function setup(handler?: Parameters<typeof entryFixture>[0]) {
  const h = await entryFixture(handler); owned.push(h.dispose)
  const entry = createPlacesEntry({ auth: h.auth, source: h.source, release: h.source.read()! }); owned.push(entry.dispose)
  return { ...h, entry, c: entry.controller }
}
const ready = (entry: PlacesEntry) => vi.waitFor(() => expect(entry.controller.read().list.phase).toBe('ready'))
it('delegates only to canonical Auth and retains the same owner across bearer refresh', async () => {
  const h = await setup(); await ready(h.entry)
  expect(h.requests).toHaveLength(1)
  expect(h.requests[0]!.cache).toBe('no-store')
  expect(h.requests[0]!.headers.get('Authorization')).toBe('Bearer synthetic-memory-token')
  expect(new URL(h.requests[0]!.url).pathname).toBe(`/api/concert/events/${eventId}/places`)
  h.auth.session.commitRefresh(h.auth.session.read(), { userId: h.auth.session.read().userId!, accessToken: 'new-memory-token' })
  expect(h.entry.read()).toBe(h.c)
  h.c.select(places()[0]!.place_id)
  await vi.waitFor(() => expect(h.c.read().detail.phase).toBe('ready'))
  expect(h.requests[1]!.headers.get('Authorization')).toBe('Bearer new-memory-token')
  expect(h.c.showOnMap(places()[0]!.place_id)).toBe(true)
  expect(h.camera.lookAt).toHaveBeenCalledTimes(1)
})
it.each(['identity', 'event', 'logout', 'policy', 'suspended', 'signal', 'withdraw', 'replace', 'map-withdraw', 'map-replace'] as const)('synchronously withdraws and clears feature state on %s', async kind => {
  const h = await setup(); await ready(h.entry)
  if (kind === 'identity') h.auth.session.replace(h.auth.session.read().generation, { userId: otherUser, eventId, accessToken: 'other-token' })
  if (kind === 'event') h.auth.session.replace(h.auth.session.read().generation, { userId: otherUser, eventId: otherUser, accessToken: 'other-token' })
  if (kind === 'logout') h.auth.session.replace(h.auth.session.read().generation, null)
  if (kind === 'policy' || kind === 'suspended') h.auth.privacy.changePolicy(h.auth.privacy.capture(), kind === 'policy' ? 'block' : 'suspended')
  if (kind === 'signal') h.lease.abort()
  if (kind === 'withdraw') h.publish(null)
  if (kind === 'replace') h.publish({ ...h.source.read()! })
  if (kind === 'map-withdraw') h.setMap(null)
  if (kind === 'map-replace') h.setMap({ ...h.mapPort.read()! })
  expect(h.entry.isDisposed()).toBe(true); expect(h.entry.read()).toBeNull()
  expect(h.c.read()).toMatchObject({ map: null, list: { data: null }, detail: { data: null } })
  expect(h.c.showOnMap(places()[0]!.place_id)).toBe(false)
  expect(h.subscriptions()).toEqual({ maps: 0, sources: 0 })
})
it('does not redirect old commands to a replaced camera or reenable a restored attachment', async () => {
  const h = await setup(); await ready(h.entry)
  const replacement = { ...h.camera, lookAt: vi.fn(), recenter: vi.fn() }
  h.setCamera(replacement)
  expect(h.c.showOnMap(places()[0]!.place_id)).toBe(false)
  expect(h.c.recenter()).toBe(false)
  h.setCamera(h.camera)
  expect(h.c.cameraAvailable()).toBe(false)
  expect(h.camera.lookAt).not.toHaveBeenCalled(); expect(replacement.lookAt).not.toHaveBeenCalled()
  expect(h.c.read().list.phase).toBe('ready')
})
it.each(['list', 'detail'] as const)('aborts a late %s read on policy change and never restores its result', async stage => {
  const pending = deferred<Response>()
  const h = await setup(request => {
    const id = new URL(request.url).pathname.split('/').at(-1)
    return stage === 'list' || id !== 'places' ? pending.promise : Response.json(catalog())
  })
  if (stage === 'detail') { await ready(h.entry); h.c.select(places()[0]!.place_id) }
  await vi.waitFor(() => expect(h.requests).toHaveLength(stage === 'list' ? 1 : 2))
  const request = h.requests.at(-1)!
  h.auth.privacy.changePolicy(h.auth.privacy.capture(), 'consent-revoked')
  expect(request.signal.aborted).toBe(true); expect(h.entry.read()).toBeNull()
  pending.resolve(Response.json(stage === 'list' ? catalog() : { event_id: eventId, revision: 7, place: places()[0] }))
  await Promise.resolve(); await Promise.resolve()
  expect(h.c.read().list.data).toBeNull(); expect(h.c.read().detail.data).toBeNull()
})
it('rejects an absent map, wrong map scope, stale privacy binding or aborted lease before HTTP dispatch', async () => {
  const h = await entryFixture(); owned.push(h.dispose)
  const binding = h.source.read()!, original = h.mapPort.read()!
  for (const mutate of [() => h.setMap(null), () => h.setMap({ ...original, scope: { ...original.scope, userId: otherUser } }), () => { h.setMap(original); h.auth.privacy.changePolicy(h.auth.privacy.capture(), 'block') }]) {
    mutate(); expect(() => createPlacesEntry({ auth: h.auth, source: h.source, release: binding })).toThrow()
  }
  h.lease.abort(); expect(() => createPlacesEntry({ auth: h.auth, source: h.source, release: binding })).toThrow()
  expect(h.requests).toHaveLength(0)
})
it('keeps an initially unavailable camera unavailable when a different attachment appears', async () => {
  const h = await entryFixture(); owned.push(h.dispose)
  h.setCamera(null)
  const entry = createPlacesEntry({ auth: h.auth, source: h.source, release: h.source.read()! }); owned.push(entry.dispose)
  await ready(entry)
  expect(entry.controller.cameraAvailable()).toBe(false)
  h.setCamera(h.camera)
  expect(entry.controller.cameraAvailable()).toBe(false)
  expect(entry.controller.showOnMap(places()[0]!.place_id)).toBe(false)
  expect(entry.controller.read().list.phase).toBe('ready')
})
it('cleans up a reentrant source withdrawal during subscription before any HTTP dispatch', async () => {
  const h = await entryFixture(); owned.push(h.dispose)
  let value = h.source.read()
  const release = value!, stop = vi.fn()
  const source = { read: () => value, subscribe(listener: () => void) { value = null; listener(); return stop } }
  const entry = createPlacesEntry({ auth: h.auth, source, release }); owned.push(entry.dispose)
  expect(entry.isDisposed()).toBe(true); expect(entry.read()).toBeNull()
  expect(stop).toHaveBeenCalledTimes(1)
  expect(h.subscriptions().maps).toBe(0); expect(h.requests).toHaveLength(0)
})
it('treats a missing or throwing camera snapshot as unavailable and never silently reenables it', async () => {
  const h = await setup(); await ready(h.entry)
  const read = h.camera.read
  h.camera.read = () => { throw new Error('Synthetic lost attachment') }
  expect(h.c.recenter()).toBe(false)
  expect(h.c.cameraAvailable()).toBe(false)
  h.camera.read = read
  expect(h.c.recenter()).toBe(false)
  expect(h.camera.recenter).not.toHaveBeenCalled()
  expect(h.c.read().list.phase).toBe('ready')
})
