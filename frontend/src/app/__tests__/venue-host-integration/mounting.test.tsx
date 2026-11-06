// @vitest-environment jsdom
// Controlled source tests of the actual mounting/providers; default-World proof is native-only.
import { StrictMode } from 'react'
import { webcrypto } from 'node:crypto'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { createMemoryRouter, Link, Outlet, RouterProvider } from 'react-router-dom'
import { VenueShellFeatures } from '../../WorldShell'
import { ParadiseCityMapPanel } from '../../venue-entry/ParadiseCityMapPanel'
import { createSessionController } from '../../../features/auth/model/session'
import { beginCommunityBootstrap } from '../../community-adoption/bootstrap'
import { checkedBootstrap, eventId, userId } from '../../community-authority/__tests__/fixtures'
import { manifestText, imageBase64 } from 'virtual:venue-root-art'
const stops: (() => void)[] = []
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
  const scope = auth.privacy.capture(), lifetime = new AbortController()
  const community = beginCommunityBootstrap({ privacy: auth.privacy, scope, signal: lifetime.signal, profile: undefined }).complete(await checkedBootstrap())
  stops.push(() => { lifetime.abort(); community.dispose(); auth.dispose() })
  function Layout() { return <VenueShellFeatures auth={auth} scope={scope} community={community}>
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
  it('loads the verified illustrated Map with absent clock/social/physical producers and offers local tabs', async () => {
    const f = await fixture(); expect(f.community.read()).toBeNull(); render(f.tree)
    await screen.findByRole('tab', { name: 'MAP' })
    expect(calls.filter(p => p.endsWith('asset-interface.json'))).toHaveLength(1)
    expect(calls.filter(p => p.endsWith('siteplan-photo-5.jpg'))).toHaveLength(1)
    expect(document.querySelector('img')?.src).toBe(resources[0])
    expect(screen.getByRole('button', { name: 'Share location' }).hasAttribute('disabled')).toBe(true)
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
