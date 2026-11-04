// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { StrictMode } from 'react'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createSessionController } from '../../../features/auth/model/session'
import { PortsProvider } from '../../access'
import { AdmissionBoundary } from '../../../features/auth/ui/AdmissionBoundary'
import { SignIn } from '../../../features/auth/ui/SignIn'
import { authResponse, capabilities, deferred, eventId, isolatedCookiePolicy, userId } from '../../../features/auth/__tests__/fixtures/data'
import { admission, policy } from '../../../features/auth/__tests__/enrollment-consent/fixtures'
const owners: { dispose(): void }[] = []
afterEach(() => { cleanup(); owners.splice(0).forEach(c => c.dispose()) })
async function setup(decision = 'CONSENT_REQUIRED', path = '/world', delayedAdmission = false) {
  let current = decision
  const late = deferred<Response>(), commands: Request[] = []
  const fetch = vi.fn<typeof globalThis.fetch>(async input => {
    const r = input as Request; commands.push(r.clone())
    if (r.url.endsWith('/capabilities')) return Response.json(capabilities())
    if (r.url.endsWith('/login')) return Response.json(authResponse())
    if (r.url.endsWith('/logout')) return new Response(null, { status: 204 })
    if (r.url.endsWith('/policy')) return Response.json(policy())
    if (r.url.endsWith('/consent')) { current = 'ADMITTED'; return Response.json(admission()) }
    if (r.url.endsWith('/admission')) return delayedAdmission ? late.promise : Response.json(admission(current))
    throw new Error('Unexpected fixture request')
  })
  const auth = createSessionController({ eventId, fetch, cookiePolicy: isolatedCookiePolicy() }); owners.push(auth)
  auth.start(); await vi.waitFor(() => expect(auth.read().emailEnabled).toBe(true))
  if (path !== '/login') { auth.session.replace(0, { userId, eventId, accessToken: 'existing-canonical' }); if (!delayedAdmission) await auth.checkAdmission() }
  const router = createMemoryRouter([{ path: '/world', element: <AdmissionBoundary><h1>Protected destination</h1></AdmissionBoundary> },
    { path: '/login', element: <AdmissionBoundary><SignIn /></AdmissionBoundary> }, { path: '/', element: <h1>Home</h1> }], { initialEntries: [path] }); owners.push(router)
  render(<StrictMode><PortsProvider ports={{ auth, session: auth.session, readiness: auth.readiness,
    bootstrap: { load: auth.loadBootstrap }, admin: { authorize: async () => false } }}><RouterProvider router={router} /></PortsProvider></StrictMode>)
  return { auth, commands, router, late }
}
async function agree() {
  fireEvent.click(screen.getByRole('button', { name: 'Load current agreements' })); await screen.findByRole('checkbox')
  fireEvent.click(screen.getByRole('checkbox')); fireEvent.click(screen.getByRole('button', { name: 'Agree and continue' }))
}
it('does not mount destination during checking and admits only the current server decision', async () => {
  const { late } = await setup('ADMITTED', '/world', true)
  expect(screen.getByRole('heading', { name: 'Checking account entry' })).toBeVisible(); expect(screen.queryByText('Protected destination')).toBeNull()
  await act(async () => late.resolve(Response.json(admission())))
  expect(await screen.findByRole('heading', { name: 'Protected destination' })).toBeVisible()
})
it('uses protected consent and fresh admission before remounting the saved destination', async () => {
  const { auth, commands, router } = await setup(); const before = auth.session.read()
  expect(screen.getByRole('heading', { name: 'Review current agreements' })).toBeVisible(); expect(screen.queryByText('Protected destination')).toBeNull()
  await agree(); expect(await screen.findByRole('heading', { name: 'Protected destination' })).toBeVisible()
  expect(auth.session.read()).toBe(before); expect(router.state.location.pathname).toBe('/world')
  const consent = commands.find(r => r.url.endsWith('/consent'))!
  expect(consent.headers.get('Authorization')).toBe('Bearer existing-canonical'); expect(consent.headers.get('X-MCR-Intent')).toBe('browser')
  expect(commands.filter(r => r.url.endsWith('/consent'))).toHaveLength(1)
  expect(commands.slice(commands.indexOf(consent) + 1).some(r => r.url.endsWith('/admission'))).toBe(true)
})
it('continues canonical sign-in through consent without submitting a second login', async () => {
  const { auth, commands } = await setup('CONSENT_REQUIRED', '/login')
  fireEvent.change(screen.getByLabelText('Email'), { target: { value: 'test@example.invalid' } })
  fireEvent.change(screen.getByLabelText('Password'), { target: { value: 'real-test-password' } })
  fireEvent.submit(screen.getByRole('form', { name: 'Email sign-in' }))
  await screen.findByRole('heading', { name: 'Review current agreements' }); await waitFor(() => expect(auth.read().pending).toBeNull())
  await agree(); expect(await screen.findByRole('link', { name: 'Continue' })).toHaveAttribute('href', '/world')
  expect(commands.filter(r => r.url.endsWith('/login'))).toHaveLength(1); expect(auth.session.read().userId).toBe(userId)
})
it.each(['POLICY_UNAVAILABLE', 'ACCOUNT_INACTIVE'])('shows truthful %s and leaves destination unmounted', async decision => {
  const { commands } = await setup(decision)
  expect(screen.queryByText('Protected destination')).toBeNull(); expect(screen.queryByRole('checkbox')).toBeNull()
  expect(screen.getByRole('heading', { name: decision === 'POLICY_UNAVAILABLE' ? 'World entry paused' : 'Account entry unavailable' })).toBeVisible()
  expect(commands.some(r => r.method === 'POST')).toBe(false)
})
