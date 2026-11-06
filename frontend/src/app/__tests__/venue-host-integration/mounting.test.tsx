// @vitest-environment jsdom
// Controlled source tests of the actual mounting/providers; default-World proof is native-only.
import { StrictMode } from 'react'
import { webcrypto } from 'node:crypto'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { createMemoryRouter, Link, Outlet, RouterProvider } from 'react-router-dom'
import { VenueShellFeatures } from '../../WorldShell'
import { ParadiseCityMapPanel } from '../../venue-entry/ParadiseCityMapPanel'
import type { HttpRequest, HttpResult } from '../../../shared/api/http'
import type { LocationSnapshot } from '../../../shared/api/paradise-city-v1/location'
import type { LocationPeople } from '../../../features/paradise-city-map/model/location-adapter'
import type { ReadPort } from '../../../features/paradise-city-map/ports'
import { createSessionController } from '../../../features/auth/model/session'
import { beginCommunityBootstrap } from '../../community-adoption/bootstrap'
import { checkedBootstrap, eventId, userId } from '../../community-authority/__tests__/fixtures'
import { manifestText, imageBase64 } from 'virtual:venue-root-art'
const stops: (() => void)[] = []
const venueAt = 1_800_000_000_000
const venueIso = (offset = 0) => new Date(venueAt + offset).toISOString()
const venueIdentity = { venueId: 'paradise-city', siteplanId: 'paradise-city-siteplan', revision: 1, variantId: 'photo-5-provisional' }
const venueFriendId = '00000000-0000-4000-8000-000000000102'
const venueGrant = { event_id: eventId, selection: { landmark_id: 'real-main-gate', catalog_revision: 3, landmark_revision: 4 },
  provider_id: 'reviewed-provider', source_revision: 'source-1', venue_key: 'venue-key', map_id: 'map-1', map_revision: 2,
  policy_revision: 9, label: 'Reviewed Main Gate', review_reference: 'review-1', reviewed_at: venueIso(-120000), server_time: venueIso(-1000),
  valid_until: venueIso(60000), physical_binding: null, presentation_id: 'main-gate', can_check_in: true, can_meet: true, can_note: true }
const venueSnapshot = (): LocationSnapshot => ({
  event_id: eventId, actor_id: userId, server_time: venueIso(-1000), access_until: venueIso(60000),
  policy: { event_id: eventId, venue_key: 'venue-key', provider_id: venueGrant.provider_id, source_revision: venueGrant.source_revision, revision: 9,
    timezone: 'Asia/Seoul', valid_until: venueIso(60000), fresh_ms: 10000, last_known_ms: 30000, fix_max_age_ms: 5000, future_skew_ms: 100,
    sharing_lease_ms: 60000, request_ttl_ms: 10000, check_in_ttl_ms: 10000, coordinate_retention_ms: 60000, command_retention_ms: 60000,
    max_accuracy_m: 50, max_viewers: 5 },
  clock: { event_id: eventId, venue_key: 'venue-key', policy_revision: 9, timezone: 'Asia/Seoul', server_time: venueIso(-1000), end_of_day: venueIso(100000), valid_until: venueIso(60000) },
  sharing: { state: 'OFF', revision: 12, viewer_ids: [], expires_at: null, updated_at: venueIso(-1000) }, self_location: null,
  friends: [{ user_id: venueFriendId, source: 'PHONE', position: { latitude: 37, longitude: 126, accuracy_m: 10, captured_at: venueIso(-1000), received_at: venueIso(-500) },
    landmark: null, observed_at: venueIso(-1000), fresh_until: venueIso(10000), expires_at: venueIso(30000), access_until: venueIso(60000), visibility_revision: 8,
    physical_arrival_verified: false, can_request_location: true }],
  capabilities: { can_share_phone: false, can_pause: false, can_revoke: false, check_in_landmark_ids: ['real-main-gate'], geographic_projection: 'UNAVAILABLE' },
  incoming_requests: [], outgoing_requests: [],
})
let resources: string[], revoked: string[], calls: string[]
beforeEach(() => {
  resources = []; revoked = []; calls = []
  vi.stubGlobal('crypto', webcrypto)
  // Geometry/layout is native-only; jsdom has no ResizeObserver.
  vi.stubGlobal('ResizeObserver', class { observe() {} unobserve() {} disconnect() {} })
  vi.stubGlobal('fetch', vi.fn(async (input: string | URL | Request) => {
    const path = String(input); calls.push(path)
    return path.endsWith('.json') ? new Response(manifestText) : new Response(Uint8Array.from(atob(imageBase64), c => c.charCodeAt(0)))
  }))
  Object.defineProperty(URL, 'createObjectURL', { configurable: true, value: vi.fn(() => { const url = `blob:http://localhost/venue-${resources.length}`; resources.push(url); return url }) })
  Object.defineProperty(URL, 'revokeObjectURL', { configurable: true, value: vi.fn((url: string) => { revoked.push(url) }) })
  vi.stubGlobal('Image', function () {
    const image = document.createElement('img'), descriptor = Object.getOwnPropertyDescriptor(HTMLImageElement.prototype, 'src')!
    Object.defineProperties(image, { naturalWidth: { value: 1024 }, naturalHeight: { value: 1280 },
      src: { configurable: true, get: () => descriptor.get!.call(image), set: value => {
        descriptor.set!.call(image, value)
        if (value) queueMicrotask(() => image.onload?.call(image, new Event('load')))
      } } })
    return image
  })
})
afterEach(() => { cleanup(); stops.splice(0).reverse().forEach(stop => stop()); vi.restoreAllMocks(); vi.unstubAllGlobals() })
async function fixture() {
  const auth = createSessionController({ eventId, fetch: async () => { throw Error('No live transport') },
    cookiePolicy: { available: true, onInvalidate: () => () => {}, run: (_kind, work) => work(), dispose() {} } })
  auth.session.replace(auth.session.read().generation, { userId, eventId, accessToken: 'TEST_ONLY_MEMORY' })
  const scope = auth.privacy.capture(), lifetime = new AbortController(), bootstrap = await checkedBootstrap()
  const community = beginCommunityBootstrap({ privacy: auth.privacy, scope, signal: lifetime.signal, profile: undefined }).complete(bootstrap)
  const clock = { read: () => ({ scope, nowMs: venueAt }), subscribe: () => () => {} }
  const venueCommunity = { ...community, isCurrent: () => true, read: () => ({ clock }) } as unknown as typeof community
  const participant = (bootstrap.document as { participant?: unknown } | undefined)?.participant
  const people: ReadPort<LocationPeople | null> = { read: () => ({ scope, venue: venueIdentity, accessUntilMs: venueAt + 60000,
    self: { userId, displayName: 'Fixture fan', avatar: null, canMessage: true, canMeet: true },
    friends: [{ userId: venueFriendId, displayName: 'Fixture friend', avatar: null, canMessage: true, canMeet: true }] }), subscribe: () => () => {} }
  const http = { async request<T>(input: HttpRequest<T>): Promise<HttpResult<T>> {
    const value = input.path.endsWith('/venue/location') ? venueSnapshot()
      : input.path.endsWith('/venue/landmarks') ? { event_id: eventId, server_time: venueIso(-1000), access_until: venueIso(60000), items: [venueGrant] }
        : { user_id: input.path.split('/').at(-1), display_name: 'Fixture friend', bio: null, avatar: null }
    return { data: input.decode(value, { scope, status: 200, path: input.path }), scope }
  } }
  stops.push(() => { lifetime.abort(); community.dispose(); auth.dispose() })
  function Layout() { return <VenueShellFeatures auth={auth} scope={scope} community={venueCommunity} participant={participant} sources={{ people, http }}>
    <nav><Link to="/map">Map route</Link><Link to="/other">Sibling route</Link></nav><Outlet />
  </VenueShellFeatures> }
  const router = createMemoryRouter([{ element: <Layout />, children: [{ path: '/map', element: <ParadiseCityMapPanel /> },
    { path: '/other', element: <p>Sibling outlet</p> }] }], { initialEntries: ['/map'] })
  stops.push(() => router.dispose())
  return { auth, scope, lifetime, community, router, tree: <RouterProvider router={router} /> }
}
describe('default venue mount lifetime', () => {
  it('starts one verified image read through actual StrictMode setup cleanup setup', async () => {
    const f = await fixture(); render(<StrictMode>{f.tree}</StrictMode>)
    await screen.findByRole('tab', { name: 'MAP' })
    expect(calls.filter(path => path.endsWith('asset-interface.json'))).toHaveLength(1)
    expect(calls.filter(path => path.endsWith('siteplan-photo-5.jpg'))).toHaveLength(1)
    expect(resources).toHaveLength(1)
  })
  it('loads the verified illustrated Map through the injected venue ports and offers local tabs', async () => {
    const f = await fixture(); expect(f.community.read()).toBeNull(); render(f.tree)
    await screen.findByRole('tab', { name: 'MAP' })
    expect(calls.filter(p => p.endsWith('asset-interface.json'))).toHaveLength(1)
    expect(calls.filter(p => p.endsWith('siteplan-photo-5.jpg'))).toHaveLength(1)
    expect(document.querySelector('img')?.src).toBe(resources[0])
    expect(screen.getByRole('button', { name: 'SHARE LOCATION' }).hasAttribute('disabled')).toBe(true)
    fireEvent.click(screen.getByRole('tab', { name: 'FRIENDS' }))
    expect(screen.getByRole('tab', { name: 'FRIENDS' }).getAttribute('aria-selected')).toBe('true')
  })
  it('retains one public image and local Map tab state across real Outlet navigation and back', async () => {
    const f = await fixture(); const view = render(f.tree); await screen.findByRole('tab', { name: 'MAP' })
    fireEvent.click(screen.getByRole('tab', { name: 'FRIENDS' }))
    fireEvent.click(screen.getByRole('link', { name: 'Sibling route' })); await screen.findByText('Sibling outlet')
    await act(() => f.router.navigate(-1)); await screen.findByRole('tab', { name: 'FRIENDS' })
    expect(screen.getByRole('tab', { name: 'FRIENDS' }).getAttribute('aria-selected')).toBe('true')
    expect(resources).toHaveLength(1); expect(revoked).toHaveLength(0); expect(calls).toHaveLength(2)
    view.unmount(); expect(revoked).toEqual(resources)
  })
  it('scrubs the illustrated publication and releases its image on actual canonical privacy withdrawal', async () => {
    const f = await fixture(); render(f.tree); await screen.findByRole('tab', { name: 'MAP' })
    act(() => { f.auth.privacy.changePolicy(f.scope, 'consent-revoked') })
    await screen.findByText(/venue map is unavailable/)
    expect(document.querySelector('img')).toBeNull(); expect(revoked).toEqual(resources)
  })
  it('fences late resource fulfillment after the original shell signal aborts', async () => {
    const f = await fixture(); let release!: (response: Response) => void
    vi.stubGlobal('fetch', vi.fn(() => new Promise<Response>(resolve => { release = resolve })))
    render(f.tree); await waitFor(() => expect(release).toBeTypeOf('function'))
    act(() => f.lifetime.abort()); await act(async () => { release(new Response(manifestText)); await Promise.resolve() })
    expect(screen.queryByRole('tab', { name: 'MAP' })).toBeNull(); expect(resources).toHaveLength(0)
  })
  it('cleans up without an application crash when privacy cleanup registration throws synchronously', async () => {
    const f = await fixture(); vi.spyOn(f.auth.privacy, 'registerCleanup').mockImplementation(() => { throw Error('Controlled cleanup registration failure') })
    render(f.tree); await screen.findByText(/venue map is unavailable/)
    expect(calls).toHaveLength(0); expect(resources).toHaveLength(0)
  })
  it('rejects a foreign original full scope before any public image request', async () => {
    const f = await fixture(), foreign = { ...f.community, scope: { ...f.scope, policyGeneration: f.scope.policyGeneration + 1 } }
    const router = createMemoryRouter([{ path: '*', element: <VenueShellFeatures auth={f.auth} scope={f.scope} community={foreign}><ParadiseCityMapPanel /></VenueShellFeatures> }]); stops.push(() => router.dispose())
    render(<RouterProvider router={router} />); await screen.findByText(/venue map is unavailable/); expect(calls).toHaveLength(0)
  })
  it('leaves truthful unavailable Map state after resource rejection and can recover in a fresh shell lifetime', async () => {
    const f = await fixture(); const working = fetch; vi.stubGlobal('fetch', vi.fn(async () => { throw Error('Controlled unavailable resource') }))
    const view = render(f.tree); await screen.findByText(/venue map is unavailable/); expect(resources).toHaveLength(0)
    view.unmount(); vi.stubGlobal('fetch', working)
    const next = await fixture(); render(next.tree); await screen.findByRole('tab', { name: 'MAP' }); expect(resources).toHaveLength(1)
  })
})
