// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { StrictMode } from 'react'
import { act, cleanup, render, screen, waitFor } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { PortsProvider } from '../../../../app/access'
import { createProductionPorts } from '../../../../app/ports'
import { Callback } from '../../ui/Callback'
import { AdmissionBoundary } from '../../ui/AdmissionBoundary'
import { createSessionController, type SessionController } from '../../model/session'
import { authResponse, deferred, eventId, isolatedCookiePolicy, userId } from '../fixtures/data'

const controllers: SessionController[] = [], routers: ReturnType<typeof createMemoryRouter>[] = []
afterEach(() => { cleanup(); routers.splice(0).forEach(router => router.dispose()); controllers.splice(0).forEach(auth => auth.dispose()) })
function setup(path: string, options: { refresh?: () => Response | Promise<Response>; admission?: string; event?: string | null; boundary?: boolean } = {}) {
  const requests: Request[] = []
  const fetch = vi.fn<typeof globalThis.fetch>(async input => {
    const request = input as Request; requests.push(request)
    // The rendered callback must have scrubbed every URL field before dispatch.
    expect(router.state.location).toMatchObject({ pathname: '/auth/callback', search: '', hash: '', state: null })
    if (request.url.endsWith('/refresh')) return options.refresh?.() ?? Response.json(authResponse())
    if (request.url.endsWith('/admission')) return Response.json({ decision: options.admission ?? 'ADMITTED',
      policyRevision: '00000000-0000-4000-8000-000000000123', acceptedRevision: options.admission ? null : '00000000-0000-4000-8000-000000000123', serverTime: '2026-11-05T00:00:00Z' })
    throw new Error('Unexpected callback request')
  })
  const auth = createSessionController({ eventId: options.event === undefined ? eventId : options.event, fetch, cookiePolicy: isolatedCookiePolicy(), timeoutMs: 1000 }); controllers.push(auth)
  const ports = createProductionPorts(auth)
  const router = createMemoryRouter([
    { path: '/auth/callback', element: <PortsProvider ports={ports}>{options.boundary ? <AdmissionBoundary><Callback /></AdmissionBoundary> : <Callback />}</PortsProvider> },
    ...['/world', '/feed', '/me', '/meet/people'].map(target => ({ path: target, element: <h1>Authorized destination</h1> })),
    { path: '/legal/privacy', element: <h1>Privacy</h1> },
  ], { initialEntries: [path] }); routers.push(router)
  render(<StrictMode><RouterProvider router={router} /></StrictMode>)
  return { auth, router, requests, fetch }
}
it.each(['/world', '/feed', '/me', '/meet/people'])('scrubs before dispatch and completes StrictMode callback to %s once', async target => {
  const { auth, router, requests } = setup(`/auth/callback?status=success&returnTo=${encodeURIComponent(target)}`)
  await screen.findByRole('heading', { name: 'Authorized destination' })
  expect(router.state.location).toMatchObject({ pathname: target, search: '', hash: '', state: null })
  expect(router.state.historyAction).toBe('REPLACE'); expect(auth.read().admission).toBe('ADMITTED')
  expect(requests.filter(request => request.url.endsWith('/refresh'))).toHaveLength(1)
  expect(document.body.innerHTML).not.toContain(authResponse().accessToken)
})
it.each(['?status=denied', '?status=error', '?status=unknown', '?status=success&status=success', '?status=success&code=PRIVATE', '?status=success#access_token=PRIVATE'])('does not request credentials for unusable callback %s', async suffix => {
  const { auth, router, fetch } = setup('/auth/callback'+suffix)
  await waitFor(() => expect(router.state.location.search+router.state.location.hash).toBe(''))
  expect(fetch).not.toHaveBeenCalled(); expect(auth.session.read().userId).toBeNull()
  expect(document.body.innerHTML).not.toContain('PRIVATE')
  await waitFor(() => expect(screen.getByRole('heading')).toHaveFocus())
})
it('keeps admission-required accounts out of the success destination with usable focused recovery', async () => {
  const { auth, router } = setup('/auth/callback?status=success&returnTo=/feed', { admission: 'CONSENT_REQUIRED' })
  expect(await screen.findByRole('heading', { name: 'Account access needs attention' })).toHaveFocus()
  expect(router.state.location.pathname).toBe('/auth/callback'); expect(auth.read().admission).toBe('CONSENT_REQUIRED')
  expect(screen.getByRole('link', { name: 'Back to your destination' })).toHaveAttribute('href', '/feed')
})
it('completes callback navigation through the actual admission boundary', async () => {
  const { auth, router, requests } = setup('/auth/callback?status=success&returnTo=/feed', { boundary: true })
  await screen.findByRole('heading', { name: 'Authorized destination' })
  expect(router.state.location.pathname).toBe('/feed')
  expect(auth.read()).toMatchObject({ admission: 'ADMITTED', pending: null })
  expect(requests.map(request => new URL(request.url).pathname)).toEqual(['/api/auth/refresh', '/api/auth/enrollment/admission'])
})
it('preserves verified inactive identity when admission replaces the callback', async () => {
  const { auth, router, requests } = setup('/auth/callback?status=success&returnTo=/feed', { boundary: true, admission: 'ACCOUNT_INACTIVE' })
  await screen.findByRole('heading', { name: 'Account entry unavailable' })
  expect(router.state.location.pathname).toBe('/auth/callback')
  expect(auth.session.read().userId).toBe(userId)
  await waitFor(() => expect(auth.read()).toMatchObject({ admission: 'ACCOUNT_INACTIVE', pending: null }))
  expect(requests).toHaveLength(2)
})
it('shows safe failure and does not retry an expired cookie on a repeated callback navigation', async () => {
  const { auth, router, requests } = setup('/auth/callback?status=success', { refresh: () => Response.json({ code: 'EXPIRED', message: 'PRIVATE_COOKIE' }, { status: 401 }) })
  expect(await screen.findByRole('heading', { name: 'Provider sign-in is unavailable' })).toHaveFocus()
  await act(async () => { await router.navigate('/auth/callback?status=success') })
  await screen.findByRole('heading', { name: 'Provider sign-in is unavailable' })
  expect(requests).toHaveLength(1); expect(auth.session.read().userId).toBeNull(); expect(document.body.innerHTML).not.toContain('PRIVATE_COOKIE')
})
it('aborts pending handoff on route exit and ignores a late credential response', async () => {
  const delayed = deferred<Response>(), { auth, router, requests } = setup('/auth/callback?status=success', { refresh: () => delayed.promise })
  await waitFor(() => expect(requests).toHaveLength(1))
  await act(async () => { await router.navigate('/legal/privacy') })
  expect(requests[0]!.signal.aborted).toBe(true)
  await act(async () => { delayed.resolve(Response.json(authResponse())) })
  expect(auth.session.read().userId).toBeNull(); expect(router.state.location.pathname).toBe('/legal/privacy')
})
it('does not dispatch without the existing event context', async () => {
  const { fetch } = setup('/auth/callback?status=success', { event: null })
  await screen.findByRole('heading', { name: 'Provider sign-in is unavailable' }); expect(fetch).not.toHaveBeenCalled()
})
it('preserves callback identity across a fresh clean navigation without reusing denial state', async () => {
  const { auth, router, fetch } = setup('/auth/callback?error=access_denied&returnTo=%2Ffeed')
  await screen.findByRole('heading', { name: 'Sign-in cancelled' })
  let identity: ReturnType<typeof auth.session.replace> = null
  await act(async () => {
    identity = auth.session.replace(auth.session.read().generation, { userId, eventId, accessToken: 'existing-canonical-token' })
    await router.navigate('/auth/callback')
  })
  expect(await screen.findByRole('heading', { name: 'Provider sign-in is unavailable' })).toHaveFocus()
  expect(screen.getByRole('link', { name: 'Back to your destination' })).toHaveAttribute('href', '/world')
  expect(auth.session.isCurrent(identity!)).toBe(true); expect(auth.session.read().accessToken).toBe('existing-canonical-token')
  expect(fetch).not.toHaveBeenCalled()
})
