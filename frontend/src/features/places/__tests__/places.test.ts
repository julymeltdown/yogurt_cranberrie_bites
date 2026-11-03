import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createHttpClient, createMemorySession, type SessionPort } from '../../../shared/api/http'
import type { SessionController } from '../../auth/model/session'
import type { PlacesHttpPort } from '../model/controller'
import { createPlacesController, type PlaceMapRelease, type PlacesController } from '../model/controller'
import { decodePlace, decodePlaces, physicalStatus, placeSchema, sourceLink } from '../model/contract'
import fixture from '../../../../../contracts/fixtures/core/place.json'
import listFixture from '../../../../../contracts/fixtures/core/place-list.json'
import contract from '../../../../../contracts/generated/mcr-core.openapi.json'
import { catalog, deferred, eventId, otherUser, places, release, userId } from './fixtures/data'
const owned: PlacesController[] = []
beforeEach(() => vi.stubGlobal('location', { origin: 'https://mcr.example.invalid' }))
afterEach(() => { owned.splice(0).forEach(c => c.dispose()); vi.unstubAllGlobals() })
const context = { scope: { userId, eventId, generation: 1 }, status: 200, path: `/api/concert/events/${eventId}/places/${fixture.place.place_id}` }
const settle = async () => { await new Promise(resolve => setTimeout(resolve, 10)) }
async function setup(handler: (request: Request) => Response | Promise<Response> = request => {
  const id = new URL(request.url).pathname.split('/').at(-1)
  return Response.json(id === 'places' ? catalog() : { event_id: eventId, revision: 7, place: places().find(p => p.place_id === id) })
}, session: SessionPort = createMemorySession()) {
  session.replace(session.read().generation, { userId, eventId, accessToken: 'fixture-token' })
  let map: PlaceMapRelease | null = await release()
  const listeners = new Set<() => void>(), camera = { lookAt: vi.fn(), recenter: vi.fn(), setZoom: vi.fn(), read: () => ({ following: true, camera: { zoom: 2, scrollX: 0, scrollY: 0, width: 390, height: 400 } }) }
  const fetch = vi.fn<typeof globalThis.fetch>(input => Promise.resolve(handler(input as Request)))
  const controller = createPlacesController({ session, http: createHttpClient({ session, fetch }),
    map: { read: () => map, subscribe: listener => { listeners.add(listener); return () => { listeners.delete(listener) } } }, camera: { read: () => camera } })
  owned.push(controller)
  return { controller, session, fetch, camera, setMap(value: PlaceMapRelease | null) { map = value; listeners.forEach(fn => fn()) } }
}
describe('C03/C06 place consumption', () => {
  it('accepts frozen examples and pins schema fields/enums without widening the public contract', () => {
    expect(decodePlace(fixture, context)).toEqual(fixture)
    expect(decodePlaces(listFixture, context)).toEqual(listFixture)
    expect(Object.keys(placeSchema.shape).sort()).toEqual([...contract.components.schemas.Place.required].sort())
    expect(placeSchema.shape.place_kind.options).toEqual(contract.components.schemas.Place.properties.place_kind.enum)
    expect(placeSchema.shape.verification_state.options).toEqual(contract.components.schemas.Place.properties.verification_state.enum)
    expect(placeSchema.shape.closure_state.options).toEqual(contract.components.schemas.Place.properties.closure_state.enum)
  })
  it.each([
    { ...listFixture, event_id: otherUser }, { ...listFixture, revision: 0 }, { ...listFixture, revision: 1.5 },
    { ...listFixture, extra: true }, { ...listFixture, items: [...listFixture.items, ...listFixture.items] },
    { ...listFixture, items: Array.from({ length: 129 }, () => listFixture.items[0]) },
  ])('rejects cross-event, malformed, duplicate or oversized catalog', value => { expect(() => decodePlaces(value, context)).toThrow() })
  it('rejects a different detail ID/status and unknown metadata', () => {
    expect(() => decodePlace({ ...fixture, place: { ...fixture.place, place_id: otherUser } }, context)).toThrow()
    expect(() => decodePlace(fixture, { ...context, status: 201 })).toThrow()
    expect(() => decodePlace({ ...fixture, place: { ...fixture.place, gps: [1, 2] } }, context)).toThrow()
  })
  it.each([
    (p: ReturnType<typeof places>[number]) => { p.verification_state = 'VERIFIED' },
    (p: ReturnType<typeof places>[number]) => { p.place_kind = 'PHYSICAL'; p.verification_state = 'UNVERIFIED' },
    (p: ReturnType<typeof places>[number]) => { p.provenance.review_state = 'REVIEWED' },
  ])('fails closed for contradictory concept/physical/review claims', mutate => {
    const p = places()[0]!; mutate(p); expect(placeSchema.safeParse(p).success).toBe(false)
  })
  it('distinguishes concept, verified, unverified, closed and source-unavailable references', () => {
    expect(places().map(physicalStatus)).toEqual([
      'Virtual concept — no physical destination', 'Verified physical reference — check current venue guidance',
      'Physical place — verification pending', 'Closed — do not use as a meeting destination', 'Physical reference — source unavailable',
    ])
  })
  it.each([null, 'javascript:alert(1)', 'data:text/html,test', 'http://venue.example.invalid', '//venue.example.invalid',
    'https://name:secret@venue.example.invalid', 'https://venue.example.invalid/a b', 'https:\\evil.invalid', 'not a URL'])('does not offer unsafe/unavailable source %s', value => { expect(sourceLink(value)).toBeNull() })
  it('offers a safe explicit HTTPS reference with no credential transfer', () => { expect(sourceLink('https://venue.example.invalid/ref')).toBe('https://venue.example.invalid/ref') })
})
describe('scoped reads, revisions and map targets', () => {
  it('delegates HTTP to the existing auth coordinator without requesting independent refresh', async () => {
    const session = createMemorySession(); session.replace(0, { userId, eventId, accessToken: 'fixture-token' })
    const transport = createHttpClient({ session, fetch: async () => Response.json(catalog()) })
    let calls = 0
    const request: SessionController['request'] = input => {
      ++calls
      expect(Object.hasOwn(input, 'refresh')).toBe(false)
      return transport.request(input)
    }
    const http: PlacesHttpPort = { request }, map = await release()
    const c = createPlacesController({ session, http, map: { read: () => map, subscribe: () => () => {} } }); owned.push(c)
    c.start(); await vi.waitFor(() => expect(c.read().list.phase).toBe('ready')); expect(calls).toBe(1)
  })
  it('uses canonical HTTP, same-origin no-store reads and exact list/detail operations', async () => {
    const { controller: c, fetch, camera } = await setup(); c.start(); await vi.waitFor(() => expect(c.read().list.phase).toBe('ready'))
    expect(c.read().mapMismatch).toBe(false); expect(c.read().list.data!.revision).toBe(7); expect(c.read().map!.map_revision).toBe(1)
    c.select(places()[0]!.place_id); await vi.waitFor(() => expect(c.read().detail.phase).toBe('ready'))
    expect(fetch).toHaveBeenCalledTimes(2)
    for (const [request] of fetch.mock.calls) {
      const r = request as Request
      expect(r.method).toBe('GET'); expect(r.cache).toBe('no-store'); expect(r.referrerPolicy).toBe('no-referrer'); expect(r.redirect).toBe('error')
      expect(r.headers.get('Authorization')).toBe('Bearer fixture-token'); expect(r.credentials).toBe('same-origin')
    }
    expect(c.showOnMap(places()[0]!.place_id)).toBe(true); expect(camera.lookAt).toHaveBeenCalledWith({ x: 48, y: 48 })
    expect(c.read().notice).toContain('avatar has not moved'); c.recenter(); c.zoom(1)
    expect(camera.recenter).toHaveBeenCalledOnce(); expect(camera.setZoom).toHaveBeenCalledWith(3)
    expect(c.canShow(places()[3]!.place_id)).toBe(false)
  })
  it('renders an honest empty catalog and missing detail without a fabricated place', async () => {
    const { controller: c } = await setup(() => Response.json(catalog([]))); c.select(places()[0]!.place_id); c.start()
    await vi.waitFor(() => expect(c.read().list.phase).toBe('empty')); expect(c.read().detail.reason).toBe('missing')
  })
  it('invalid release pin disables targets while retaining readable place metadata', async () => {
    const { controller: c, setMap, camera } = await setup(), original = await release(), bad = { ...original, expected: { ...original.expected, revision: 99 } }
    setMap(bad); c.start(); await vi.waitFor(() => expect(c.read().list.phase).toBe('ready'))
    expect(c.read().mapMismatch).toBe(true); expect(c.showOnMap(places()[0]!.place_id)).toBe(false); expect(camera.lookAt).not.toHaveBeenCalled()
    setMap(await release()); await vi.waitFor(() => expect(c.read().list.phase).toBe('ready'))
    expect(c.read().mapMismatch).toBe(false); expect(c.canShow(places()[0]!.place_id)).toBe(true)
  })
  it('unknown map place or changed stable provenance pauses targeting', async () => {
    const items = places(); items[0]!.provenance.source_revision = 'new-source'
    const { controller: c } = await setup(() => Response.json(catalog(items))); c.start(); await vi.waitFor(() => expect(c.read().list.phase).toBe('ready'))
    expect(c.read().mapMismatch).toBe(true); expect(c.canShow(items[0]!.place_id)).toBe(false)
  })
  it('does not confuse a new catalog revision with the map revision, but rejects inconsistent detail revisions', async () => {
    const { controller: c } = await setup(request => Response.json(request.url.endsWith('/places') ? catalog()
      : { event_id: eventId, revision: 8, place: places()[0] }))
    c.start(); c.select(places()[0]!.place_id); await vi.waitFor(() => expect(c.read().detail.reason).toBe('conflict'))
    expect(c.canShow(places()[0]!.place_id)).toBe(false)
  })
  it('detail metadata disagreement at the same revision produces recovery rather than a wrong target', async () => {
    const { controller: c } = await setup(request => Response.json(request.url.endsWith('/places') ? catalog()
      : { event_id: eventId, revision: 7, place: { ...places()[0], label: 'Changed detail' } }))
    c.start(); c.select(places()[0]!.place_id); await vi.waitFor(() => expect(c.read().detail.reason).toBe('map-mismatch'))
    expect(c.read().mapMismatch).toBe(true)
  })
  it.each([404, 410])('retains authoritative detail %s target invalidation after Back and catalog retry without blocking unrelated places', async status => {
    let unavailable = true
    const data = places(), target = data[1]!.place_id, other = data[0]!.place_id
    const { controller: c, camera } = await setup(request => request.url.endsWith('/places') ? Response.json(catalog())
      : unavailable ? Response.json({ code: 'fixture', message: 'not rendered', details: { reason: 'RESOURCE_UNAVAILABLE' } }, { status })
        : Response.json({ event_id: eventId, revision: 7, place: data[1] }))
    c.start(); await vi.waitFor(() => expect(c.read().list.phase).toBe('ready')); expect(c.canShow(target)).toBe(true)
    c.select(target); await vi.waitFor(() => expect(c.read().detail.reason).toBe('missing'))
    c.select(null); expect(c.canShow(target)).toBe(false); expect(c.showOnMap(target)).toBe(false)
    expect(c.canShow(other)).toBe(true); expect(c.cameraAvailable()).toBe(true); expect(camera.lookAt).not.toHaveBeenCalled()
    await c.refresh(); expect(c.canShow(target)).toBe(false); expect(c.read().unavailablePlaceIds).toContain(target)
    unavailable = false; c.select(target); await vi.waitFor(() => expect(c.read().detail.phase).toBe('ready'))
    c.select(null); expect(c.canShow(target)).toBe(true); expect(c.read().unavailablePlaceIds).not.toContain(target)
  })
  it('retains authoritative detail map-version rejection across navigation and same-pin retries until a new matching release/catalog pair', async () => {
    let rejected = true
    const data = places(), target = data[1]!.place_id
    const { controller: c, setMap, camera } = await setup(request => request.url.endsWith('/places') ? Response.json(catalog())
      : rejected ? Response.json({ code: 'fixture', message: 'not rendered', details: { reason: 'MAP_VERSION_MISMATCH' } }, { status: 409 })
        : Response.json({ event_id: eventId, revision: 7, place: data[1] }))
    c.start(); await vi.waitFor(() => expect(c.read().list.phase).toBe('ready'))
    c.select(target); await vi.waitFor(() => expect(c.read().detail.reason).toBe('map-mismatch'))
    c.select(null); expect(c.canShow(target)).toBe(false); expect(c.canShow(data[0]!.place_id)).toBe(false)
    expect(c.cameraAvailable()).toBe(false); expect(c.recenter()).toBe(false); expect(c.zoom(1)).toBe(false)
    expect(camera.recenter).not.toHaveBeenCalled(); expect(camera.setZoom).not.toHaveBeenCalled()
    rejected = false; await c.refresh(); expect(c.read().mapMismatch).toBe(true); expect(c.canShow(target)).toBe(false)
    setMap(await release(data, 2)); await vi.waitFor(() => expect(c.read().list.phase).toBe('ready'))
    expect(c.read().mapMismatch).toBe(false); expect(c.canShow(target)).toBe(true)
    expect(c.read().list.data!.revision).toBe(7); expect(c.read().map!.map_revision).toBe(2)
  })
  it('list map-version errors also remain latched through catalog-only recovery', async () => {
    let rejected = true
    const { controller: c, setMap } = await setup(() => rejected
      ? Response.json({ code: 'fixture', message: 'not rendered', details: { reason: 'MAP_VERSION_MISMATCH' } }, { status: 409 }) : Response.json(catalog()))
    c.start(); await vi.waitFor(() => expect(c.read().list.reason).toBe('map-mismatch'))
    rejected = false; await c.refresh(); expect(c.read().list.phase).toBe('ready'); expect(c.read().mapMismatch).toBe(true)
    setMap(await release(places(), 2)); await vi.waitFor(() => expect(c.read().list.phase).toBe('ready')); expect(c.read().mapMismatch).toBe(false)
  })
  it('discarding an obsolete missing-place response cannot invalidate the newly selected target', async () => {
    const old = deferred<Response>(), data = places()
    const { controller: c } = await setup(request => request.url.endsWith('/places') ? Response.json(catalog())
      : request.url.endsWith(data[0]!.place_id) ? old.promise : Response.json({ event_id: eventId, revision: 7, place: data[1] }))
    c.start(); await vi.waitFor(() => expect(c.read().list.phase).toBe('ready')); c.select(data[0]!.place_id); c.select(data[1]!.place_id)
    await vi.waitFor(() => expect(c.read().detail.phase).toBe('ready'))
    old.resolve(Response.json({ code: 'fixture', message: 'not rendered', details: { reason: 'RESOURCE_UNAVAILABLE' } }, { status: 404 }))
    await settle(); expect(c.read().unavailablePlaceIds).toEqual([]); expect(c.canShow(data[1]!.place_id)).toBe(true)
  })
  it.each([[401, 'AUTHENTICATION_REQUIRED', 'signed-out'], [403, 'PERMISSION_DENIED', 'denied'], [404, 'RESOURCE_UNAVAILABLE', 'missing'],
    [409, 'MAP_VERSION_MISMATCH', 'map-mismatch'], [409, 'VERSION_CONFLICT', 'conflict'], [429, 'RATE_LIMITED', 'rate-limited'],
    [503, 'DEPENDENCY_UNAVAILABLE', 'unavailable']])('classifies %s/%s without leaking server text', async (status, reason, expected) => {
    let fail = true
    const { controller: c } = await setup(() => fail ? Response.json({ code: 'fixture', message: 'private server text', details: { reason } }, { status: Number(status) }) : Response.json(catalog()))
    c.start(); await vi.waitFor(() => expect(c.read().list.reason).toBe(expected)); expect(JSON.stringify(c.read())).not.toContain('private server text')
    fail = false; await c.refresh(); expect(c.read().list.phase).toBe('ready')
  })
  it('logout aborts and purges an in-flight response; an ignored abort cannot repopulate it', async () => {
    const old = deferred<Response>(), { controller: c, session } = await setup(() => old.promise)
    c.start(); expect(c.read().list.phase).toBe('loading'); session.replace(session.read().generation, null)
    expect(c.read().list.reason).toBe('signed-out'); old.resolve(Response.json(catalog())); await settle()
    expect(c.read().list.data).toBeNull(); expect(c.read().map).toBeNull()
  })
  it('account replacement installs only the new owner response', async () => {
    const old = deferred<Response>(); let n = 0
    const { controller: c, session } = await setup(() => ++n === 1 ? old.promise : Response.json(catalog([])))
    c.start(); session.replace(session.read().generation, { userId: otherUser, eventId, accessToken: 'new-token' })
    await vi.waitFor(() => expect(c.read().list.phase).toBe('empty')); old.resolve(Response.json(catalog())); await settle()
    expect(c.read().list.data?.items).toEqual([])
  })
  it('a new account/event cannot target the former scope map even when place IDs and release identity coincide', async () => {
    const { controller: c, session, setMap, camera } = await setup(request => Response.json({ ...catalog(), event_id: new URL(request.url).pathname.split('/')[4] }))
    c.start(); await vi.waitFor(() => expect(c.read().list.phase).toBe('ready'))
    const nextEvent = '00000000-0000-4000-8000-000000000209'
    session.replace(session.read().generation, { userId: otherUser, eventId: nextEvent, accessToken: 'next-scope-token' })
    await vi.waitFor(() => expect(c.read().list.phase).toBe('ready'))
    expect(c.read().mapMismatch).toBe(true); expect(c.showOnMap(places()[0]!.place_id)).toBe(false); expect(camera.lookAt).not.toHaveBeenCalled()
    const next = await release(); setMap({ ...next, scope: session.read() })
    await vi.waitFor(() => expect(c.read().list.phase).toBe('ready')); expect(c.read().mapMismatch).toBe(false)
    expect(c.canShow(places()[0]!.place_id)).toBe(true)
  })
  it('disposing from a publication observer cannot restart requests or leave a cached place', async () => {
    const { controller: c, fetch } = await setup()
    c.subscribe(() => { if (c.read().list.phase === 'loading') c.dispose(); else c.start() })
    c.start(); await settle(); expect(c.read().list.data).toBeNull(); expect(fetch).not.toHaveBeenCalled()
  })
  it('release replacement cancels old reads and disposal removes authority and subscriptions', async () => {
    const old = deferred<Response>(); let n = 0
    const { controller: c, setMap, fetch } = await setup(() => ++n === 1 ? old.promise : Response.json(catalog([])))
    c.start(); setMap(await release()); await vi.waitFor(() => expect(c.read().list.phase).toBe('empty'))
    old.resolve(Response.json(catalog())); await settle(); expect(c.read().list.phase).toBe('empty')
    c.dispose(); setMap(await release()); await c.refresh(); c.select(places()[0]!.place_id); await settle()
    expect(fetch).toHaveBeenCalledTimes(2); expect(c.read().list.data).toBeNull(); expect(c.showOnMap(places()[0]!.place_id)).toBe(false)
  })
  it('discards an older selected detail after navigation', async () => {
    const old = deferred<Response>(), data = places()
    const { controller: c } = await setup(request => request.url.endsWith('/places') ? Response.json(catalog())
      : request.url.endsWith(data[0]!.place_id) ? old.promise : Response.json({ event_id: eventId, revision: 7, place: data[1] }))
    c.start(); await vi.waitFor(() => expect(c.read().list.phase).toBe('ready')); c.select(data[0]!.place_id); c.select(data[1]!.place_id)
    await vi.waitFor(() => expect(c.read().detail.data?.place.place_id).toBe(data[1]!.place_id))
    old.resolve(Response.json({ event_id: eventId, revision: 7, place: data[0] })); await settle()
    expect(c.read().detail.data?.place.place_id).toBe(data[1]!.place_id)
  })
})
