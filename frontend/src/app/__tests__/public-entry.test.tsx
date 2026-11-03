// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { StrictMode, useLayoutEffect } from 'react'
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../router'
import { PortsProvider } from '../access'
import { FoundationApp } from '../FoundationApp'
import { createProductionPorts, type ShellPorts } from '../ports'
import { createFixturePorts } from './fixtures/ports'
import { createSessionController, type SessionController } from '../../features/auth/model/session'
import { authResponse, capabilities, eventId, isolatedCookiePolicy } from '../../features/auth/__tests__/fixtures/data'
import { PublicDocumentsProvider } from '../../features/public/model/document-provider'
import type { DocumentProvider } from '../../features/public/model/documents'
import { deferred, syntheticDocument, syntheticDocuments } from '../../features/public/__tests__/fixtures/documents'
const routers: ReturnType<typeof createMemoryRouter>[] = [], controllers: SessionController[] = []
afterEach(() => { cleanup(); routers.splice(0).forEach(r => r.dispose()); controllers.splice(0).forEach(c => c.dispose()); vi.useRealTimers() })
function setup(path = '/', provider?: DocumentProvider, ports: ShellPorts = createProductionPorts(), entries = [path]) {
  const router = createMemoryRouter(createAppRoutes(ports), { initialEntries: entries }); routers.push(router)
  const node = <RouterProvider router={router} />
  const rendered = render(<StrictMode>{provider ? <PublicDocumentsProvider provider={provider}>{node}</PublicDocumentsProvider> : node}</StrictMode>)
  return { router, ...rendered }
}
it.each(['anonymous', 'unknown', 'checking', 'unavailable', 'error'] as const)('keeps %s home outside world allocation and imports no fixture identity into product text', async readiness => {
  const fixture = createFixturePorts(readiness), load = vi.fn(fixture.ports.bootstrap.load)
  setup('/', undefined, { ...fixture.ports, bootstrap: { load } })
  await screen.findByRole('heading', { name: 'One world. Every era.' })
  expect(load).not.toHaveBeenCalled(); expect(fixture.stats.mounts).toBe(0)
  expect(document.querySelector('[data-world-host]')).toBeNull()
  expect(document.body.textContent).not.toContain('fixture-only-memory-token')
  if (readiness === 'anonymous') expect(screen.getByRole('link', { name: 'Sign in to enter' })).toHaveAttribute('href', '/login?returnTo=%2Fworld')
  else expect(screen.getByRole('button', { name: /Enter the world|Checking entry/ })).toBeDisabled()
})
it('shows entry unavailable without production event/identity and links to actual public routes', async () => {
  const { router } = setup(); await screen.findByText('World entry is not open yet.')
  const nav = screen.getByRole('navigation', { name: 'Public information' })
  for (const [label, path] of [['Terms', '/legal/terms'], ['Privacy', '/legal/privacy'], ['Safety', '/legal/safety'], ['Contact', '/legal/contact']]) {
    expect(within(nav).getByRole('link', { name: label! })).toHaveAttribute('href', path)
  }
  fireEvent.click(screen.getByRole('link', { name: 'Sign in' }))
  await screen.findByRole('heading', { name: 'Sign in' }); expect(router.state.location.pathname).toBe('/login')
  expect(screen.getByText('Event entry is not open yet. Please check back later.')).toBeVisible()
})
it('uses canonical FE09 ready identity for a replace to /world, preserving the prior history entry', async () => {
  const fetch = vi.fn<typeof globalThis.fetch>(async input => (input as Request).url.endsWith('/capabilities') ? Response.json(capabilities())
    : (input as Request).url.endsWith('/login') ? Response.json(authResponse()) : new Response(null, { status: 503 }))
  const auth = createSessionController({ eventId, fetch, cookiePolicy: isolatedCookiePolicy() }); controllers.push(auth); auth.start()
  await vi.waitFor(() => expect(auth.read().emailEnabled).toBe(true)); await auth.login('fan@example.com', 'test-password')
  const { router } = setup('/', undefined, createProductionPorts(auth), ['/legal/privacy', '/'])
  await waitFor(() => expect(router.state.location.pathname).toBe('/world'))
  expect(router.state.historyAction).toBe('REPLACE')
  await act(async () => { await router.navigate(-1) })
  expect(router.state.location.pathname).toBe('/legal/privacy')
  expect(await screen.findByRole('heading', { name: 'Privacy' })).toBeVisible()
  expect(document.body.textContent).not.toContain(authResponse().accessToken)
})
it('does not redirect a rendered identity revoked before layout dispatch or project a prior account', async () => {
  const fixture = createFixturePorts(), load = vi.fn(fixture.ports.bootstrap.load)
  let revoked = false
  function RevokeBeforeSiblingLayout() {
    useLayoutEffect(() => { if (!revoked) { revoked = true; fixture.ports.session.replace(fixture.ports.session.read().generation, null); fixture.setReadiness('anonymous') } }, [])
    return null
  }
  const ports = { ...fixture.ports, bootstrap: { load } }
  const router = createMemoryRouter([{ path: '/', element: <PortsProvider ports={ports}><RevokeBeforeSiblingLayout /><FoundationApp /></PortsProvider> },
    { path: '/world', element: <p>Unexpected stale navigation</p> }]); routers.push(router)
  render(<StrictMode><RouterProvider router={router} /></StrictMode>)
  await screen.findByRole('link', { name: 'Sign in to enter' })
  expect(router.state.location.pathname).toBe('/'); expect(load).not.toHaveBeenCalled()
  expect(document.body.textContent).not.toContain('Unexpected stale navigation')
  expect(document.body.textContent).not.toContain(fixture.ports.session.read().userId ?? 'fixture-only-memory-token')
})
it('never treats ready without a valid canonical identity/event as permission to redirect', async () => {
  const fixture = createFixturePorts('ready'); fixture.ports.session.replace(fixture.ports.session.read().generation, null)
  const { router } = setup('/', undefined, fixture.ports)
  await screen.findByText('We could not confirm entry. You can check sign-in again.')
  expect(router.state.location.pathname).toBe('/'); expect(fixture.stats.mounts).toBe(0)
})
it.each(['terms', 'privacy', 'safety', 'contact'])('defaults %s to honestly unavailable and supports home/back navigation', async key => {
  const fixture = createFixturePorts('ready'), load = vi.fn(fixture.ports.bootstrap.load)
  const { router } = setup(`/legal/${key}`, undefined, { ...fixture.ports, bootstrap: { load } })
  await screen.findByText(key === 'contact' ? 'Operator contact details are not available yet.' : 'This document is not available yet.')
  expect(document.querySelector('time')).toBeNull(); expect(load).not.toHaveBeenCalled(); expect(fixture.stats.mounts).toBe(0)
  expect(screen.getByRole('navigation', { name: 'Public information' }).querySelector('[aria-current="page"]')).toHaveTextContent(new RegExp(key, 'i'))
  expect(screen.getByRole('link', { name: 'Back to home' })).toHaveAttribute('href', '/')
  expect(router.state.location.pathname).toBe(`/legal/${key}`)
})
it('does not reflect or load an unknown document name', async () => {
  const read = vi.fn(syntheticDocuments.read)
  setup('/legal/private-email%40example.com', { syntheticPreview: true, read })
  await screen.findByRole('heading', { name: 'Document unavailable' })
  expect(read).not.toHaveBeenCalled(); expect(document.body.textContent).not.toContain('private-email@example.com')
})
it('qualifies loading/version/date/links with an explicit synthetic disclosure and focused route headings', async () => {
  const delayed = deferred<unknown>(), read = vi.fn(async () => delayed.promise)
  setup('/legal/terms', { syntheticPreview: true, read })
  expect(await screen.findByText('Opening this document…')).toBeVisible()
  await act(async () => delayed.resolve(syntheticDocument()))
  expect(await screen.findByText('TEST-2026.11.03')).toBeVisible()
  expect(screen.getByText('Synthetic document preview for UI testing. This is not approved policy or operator contact information.')).toBeVisible()
  expect(document.querySelector('time')).toHaveAttribute('datetime', '2026-11-03T00:00:00Z')
  expect(screen.getByText('03 Nov 2026')).toBeVisible(); expect(screen.getByText('04 Nov 2026')).toBeVisible()
  expect(screen.getByRole('heading', { name: 'Terms' })).toHaveFocus()
})
it('does not display stale terms while the next document waits, and cancels ignored-abort completions', async () => {
  const privacy = deferred<unknown>(), terms = deferred<unknown>(), signals: AbortSignal[] = []
  const provider: DocumentProvider = { syntheticPreview: true, read: async (key, signal) => { signals.push(signal); return key === 'terms' ? terms.promise : privacy.promise } }
  const { router } = setup('/legal/terms', provider)
  await screen.findByText('Opening this document…')
  await act(async () => { await router.navigate('/legal/privacy') })
  expect(screen.queryByText('TEST-2026.11.03')).not.toBeInTheDocument()
  expect(signals.some(s => s.aborted)).toBe(true)
  await act(async () => terms.resolve(syntheticDocument('terms')))
  expect(screen.queryByText('TEST-2026.11.03')).not.toBeInTheDocument()
  await act(async () => privacy.resolve(syntheticDocument('privacy')))
  expect(await screen.findByText('TEST-2026.11.03')).toBeVisible()
  expect(screen.getByRole('heading', { name: 'Privacy' })).toHaveFocus()
})
it('shows a safe provider error, retries explicitly, and does not reveal rejection details', async () => {
  const read = vi.fn().mockRejectedValueOnce(new Error('private raw source detail')).mockResolvedValue(syntheticDocument())
  setup('/legal/terms', { syntheticPreview: true, read })
  expect(await screen.findByRole('alert')).toHaveTextContent('We could not open this document.')
  expect(document.body.textContent).not.toContain('private raw source detail')
  const count = read.mock.calls.length; fireEvent.click(screen.getByRole('button', { name: 'Try again' }))
  expect(await screen.findByText('TEST-2026.11.03')).toBeVisible(); expect(read).toHaveBeenCalledTimes(count + 1)
})
it('renders markup as inert plain text without an HTML, Markdown or script interpreter', async () => {
  const source = syntheticDocument(); source.document.sections[0]!.paragraphs = ['<script>window.__PUBLIC_ATTACK__=true</script><img src=x onerror=alert(1)>', '[unsafe](javascript:alert(1))']
  setup('/legal/terms', { syntheticPreview: true, read: async () => source })
  expect(await screen.findByText('<script>window.__PUBLIC_ATTACK__=true</script><img src=x onerror=alert(1)>')).toBeVisible()
  expect(document.querySelector('.public-document script, .public-document img')).toBeNull()
  expect(document.querySelector('a[href^="javascript:"]')).toBeNull()
})
it('rejects hostile external links as an unavailable display without partial document text', async () => {
  const source = syntheticDocument(); source.document.sections[0]!.links[0]!.to = 'https://evil.example/collect'
  setup('/legal/terms', { syntheticPreview: true, read: async () => source })
  expect(await screen.findByRole('alert')).toBeVisible()
  expect(screen.queryByText('TEST-2026.11.03')).not.toBeInTheDocument()
  expect(document.querySelector('a[href*="evil.example"]')).toBeNull()
})

it('bounds a hung document read and suppresses its ignored-abort completion', async () => {
  const pending = deferred<unknown>(), signals: AbortSignal[] = []
  vi.useFakeTimers()
  const { unmount } = setup('/legal/terms', { syntheticPreview: true, read: async (_key, signal) => { signals.push(signal); return pending.promise } })
  await act(async () => undefined)
  expect(signals.length).toBeGreaterThan(0)
  await act(async () => { await vi.advanceTimersByTimeAsync(10_001) })
  expect(screen.getByRole('alert')).toHaveTextContent('We could not open this document.')
  expect(signals.every(s => s.aborted)).toBe(true)
  await act(async () => pending.resolve(syntheticDocument()))
  expect(screen.queryByText('TEST-2026.11.03')).not.toBeInTheDocument()
  unmount()
})
it('reads fresh public document state on link navigation and browser back without a world', async () => {
  const fixture = createFixturePorts('anonymous'), { router } = setup('/legal/terms', syntheticDocuments, fixture.ports)
  await screen.findByText('TEST-2026.11.03')
  fireEvent.click(screen.getByRole('link', { name: 'Open the synthetic privacy page' }))
  expect(await screen.findByRole('heading', { name: 'Privacy' })).toHaveFocus()
  expect(router.state.location.pathname).toBe('/legal/privacy')
  await act(async () => { await router.navigate(-1) })
  expect(await screen.findByRole('heading', { name: 'Terms' })).toHaveFocus()
  expect(router.state.location.pathname).toBe('/legal/terms'); expect(fixture.stats.mounts).toBe(0)
})
