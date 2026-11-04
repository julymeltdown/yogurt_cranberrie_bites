// @vitest-environment jsdom
// Synthetic public-contract responses. Default root tests require Root's actual shared seam.
import '@testing-library/jest-dom/vitest'
import { StrictMode } from 'react'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../../router'
import { PortsProvider } from '../../access'
import { ArchiveRoutePanel } from '../../archive-entry'
import { createFixturePorts } from '../fixtures/ports'
import { deferred, eventId, id, page } from '../../../features/archive/__tests__/fixtures/data'
import { createProductionPorts, type ShellPorts } from '../../ports'

const routers: ReturnType<typeof createMemoryRouter>[] = []
beforeEach(() => {
  vi.stubGlobal('location', { origin: 'https://mcr.example' })
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({
    createImageData: () => ({ data: new Uint8ClampedArray(128 * 128 * 4) }),
    putImageData: vi.fn(), clearRect: vi.fn(),
  } as unknown as CanvasRenderingContext2D)
})
afterEach(() => { cleanup(); routers.splice(0).forEach(r => r.dispose()); vi.restoreAllMocks(); vi.unstubAllGlobals() })
function publicFetch(handler: (request: Request) => Response | Promise<Response> = request => Response.json(page(new URL(request.url).searchParams.has('cursor')))) {
  const requests: Request[] = []
  const fetch = vi.fn<typeof globalThis.fetch>(input => {
    const request = input as Request
    requests.push(request)
    return Promise.resolve(handler(request))
  })
  vi.stubGlobal('fetch', fetch)
  return { requests, fetch }
}
function root(ports: ShellPorts = { ...createFixturePorts('anonymous').ports, eventId }, path = '/archive') {
  const router = createMemoryRouter(createAppRoutes(ports), { initialEntries: [path] })
  routers.push(router)
  const view = render(<StrictMode><RouterProvider router={router} /></StrictMode>)
  return { ...view, router }
}
function publicOnly(requests: Request[]) {
  for (const request of requests) {
    expect(request.method).toBe('GET'); expect(request.credentials).toBe('omit')
    expect(request.cache).toBe('no-store'); expect(request.redirect).toBe('error')
    expect(request.headers.has('Authorization')).toBe(false)
    expect(request.headers.has('Cookie')).toBe(false)
    expect(new URL(request.url).pathname).toBe(`/api/concert/events/${eventId}/archive`)
  }
}

it('default PublicBoundary renders the public final wall/feed without world, private bootstrap or credentials', async () => {
  const fixture = createFixturePorts('anonymous'), bootstrap = vi.fn(fixture.ports.bootstrap.load)
  const { requests } = publicFetch()
  root({ ...fixture.ports, eventId, bootstrap: { load: bootstrap } })
  expect(await screen.findByText(/We sang every word/)).toBeVisible()
  expect(screen.getByRole('heading', { name: 'After the show' })).toBeVisible()
  expect(screen.getByRole('img', { name: /Final frozen wall, 128 by 128 pixels/ })).toHaveAttribute('width', '128')
  expect(screen.queryByText(/still being prepared/)).not.toBeInTheDocument()
  expect(screen.queryByRole('textbox')).not.toBeInTheDocument()
  expect(screen.queryByRole('button', { name: /compose|react|comment|edit|delete|draw|pickup/i })).not.toBeInTheDocument()
  expect(document.body.textContent).not.toContain('fixture-only-memory-token')
  expect(document.querySelector('[data-world-host]')).toBeNull()
  expect(fixture.stats.mounts).toBe(0); expect(bootstrap).not.toHaveBeenCalled()
  expect(requests).toHaveLength(1); publicOnly(requests)
})

it('actual anonymous production factory supplies the explicit public event independently of Auth readiness', async () => {
  const { requests } = publicFetch()
  const ports = createProductionPorts(undefined, {}, undefined, eventId)
  expect(ports.readiness.read()).toBe('unavailable'); expect(ports.session.read().eventId).toBeNull()
  root(ports)
  expect(await screen.findByText(/We sang every word/)).toBeVisible()
  expect(requests).toHaveLength(1); publicOnly(requests)
})

it('default route paginates the public projection and truthfully leaves unqualified media unavailable', async () => {
  const { requests } = publicFetch(); root()
  await screen.findByText(/We sang every word/)
  fireEvent.click(screen.getByRole('button', { name: /View archived image 1/ }))
  expect(await screen.findByText('Archived image unavailable. No live or private image is substituted.')).toBeVisible()
  expect(requests).toHaveLength(1); expect(document.querySelector('img.archive-image')).toBeNull()
  fireEvent.click(screen.getByRole('button', { name: 'Next page' }))
  expect(await screen.findByText('The last chorus stayed with us.')).toBeVisible()
  expect(screen.queryByText(/We sang every word/)).not.toBeInTheDocument()
  expect(screen.getByRole('heading', { name: 'Posts after the show' })).toHaveFocus()
  expect(screen.queryByRole('button', { name: 'Next page' })).not.toBeInTheDocument()
  expect(new URL(requests[1]!.url).searchParams.get('cursor')).toBe('page-two')
  expect(requests).toHaveLength(2); publicOnly(requests)
})

it.each([undefined, null, 'invalid'])('missing/invalid public event %s cannot borrow session or query event', async value => {
  const { fetch } = publicFetch()
  root({ ...createFixturePorts('ready').ports, ...(value === undefined ? {} : { eventId: value }) }, `/archive?eventId=${eventId}`)
  expect(await screen.findByRole('alert')).toHaveTextContent('This archive is unavailable.')
  expect(fetch).not.toHaveBeenCalled()
})

it('exposes loading then an empty public archive without fabricating posts', async () => {
  const response = deferred<Response>(), { fetch } = publicFetch(() => response.promise)
  root(); expect(await screen.findByText('Reading the public archive…')).toBeVisible()
  const empty = page(); empty.feed.items = []; empty.feed.next_cursor = null
  await act(async () => response.resolve(Response.json(empty)))
  expect(await screen.findByText('No permitted posts on this page.')).toBeVisible()
  expect(screen.getByRole('img', { name: /Final frozen wall/ })).toBeVisible()
  expect(fetch).toHaveBeenCalledTimes(1)
})

it.each([404, 410, 429, 503])('safe public HTTP %s errors disclose no producer diagnostics', async status => {
  publicFetch(() => Response.json({ private_handle: 'DO_NOT_DISPLAY', physical_beacon: 'DO_NOT_DISPLAY' }, { status }))
  root(); expect(await screen.findByRole('alert')).toBeVisible()
  expect(document.body.textContent).not.toContain('DO_NOT_DISPLAY')
  expect(screen.queryByRole('img', { name: /Final frozen wall/ })).not.toBeInTheDocument()
})

it('rejects private projection fields and removes formerly visible content after producer withdrawal', async () => {
  let mode = 'ready'
  publicFetch(() => {
    if (mode === 'removed') return Response.json({}, { status: 410 })
    const result = page()
    return Response.json(mode === 'private' ? { ...result, private_handle: 'DO_NOT_DISPLAY' } : result)
  })
  root(); await screen.findByText(/We sang every word/)
  mode = 'private'; fireEvent.click(screen.getByRole('button', { name: 'Reload archive' }))
  await screen.findByRole('alert')
  expect(screen.queryByText(/We sang every word/)).not.toBeInTheDocument()
  expect(document.body.textContent).not.toContain('DO_NOT_DISPLAY')
  mode = 'ready'; fireEvent.click(screen.getByRole('button', { name: 'Reload archive' }))
  await screen.findByText(/We sang every word/)
  mode = 'removed'; fireEvent.click(screen.getByRole('button', { name: 'Reload archive' }))
  await screen.findByRole('alert')
  expect(screen.queryByText(/We sang every word/)).not.toBeInTheDocument()
  expect(document.querySelector('.archive-wall canvas')).toBeNull()
})

it('canonical identity generation cancels old public work and cannot repopulate from ignored-abort completion', async () => {
  const stale = deferred<Response>(), fixture = createFixturePorts('anonymous')
  let calls = 0
  const { requests } = publicFetch(() => {
    if (++calls === 1) return stale.promise
    const fresh = page(); fresh.feed.items[0]!.text = 'Fresh public generation'
    return Response.json(fresh)
  })
  root({ ...fixture.ports, eventId })
  await waitFor(() => expect(requests).toHaveLength(1))
  act(() => fixture.replace(id(81), id(82)))
  expect(requests[0]!.signal.aborted).toBe(true)
  expect(await screen.findByText('Fresh public generation')).toBeVisible()
  await act(async () => stale.resolve(Response.json(page())))
  expect(screen.queryByText(/We sang every word/)).not.toBeInTheDocument()
  expect(requests).toHaveLength(2); publicOnly(requests)
})

it('isolated adapter hides a previous event before replacement settles and releases public work on unmount', async () => {
  const fixture = createFixturePorts('anonymous'), next = deferred<Response>()
  const { requests } = publicFetch(request => new URL(request.url).pathname.includes(id(2)) ? next.promise : Response.json(page()))
  const view = render(<StrictMode><PortsProvider ports={{ ...fixture.ports, eventId }}><ArchiveRoutePanel /></PortsProvider></StrictMode>)
  await screen.findByText(/We sang every word/)
  view.rerender(<StrictMode><PortsProvider ports={{ ...fixture.ports, eventId: id(2) }}><ArchiveRoutePanel /></PortsProvider></StrictMode>)
  expect(screen.queryByText(/We sang every word/)).not.toBeInTheDocument()
  await waitFor(() => expect(requests).toHaveLength(2))
  view.unmount(); expect(requests[1]!.signal.aborted).toBe(true)
  act(() => fixture.replace())
  await act(async () => next.resolve(Response.json(page(false, id(2)))))
  expect(requests).toHaveLength(2); expect(document.querySelector('.event-archive')).toBeNull()
})
