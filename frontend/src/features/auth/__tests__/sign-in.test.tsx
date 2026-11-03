// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { StrictMode } from 'react'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../../../app/router'
import { createProductionPorts } from '../../../app/ports'
import { createSessionController, type SessionController } from '../model/session'
import { authResponse, capabilities, deferred, eventId, isolatedCookiePolicy } from './fixtures/data'
const instances: SessionController[] = [], routers: ReturnType<typeof createMemoryRouter>[] = []
afterEach(() => { cleanup(); routers.splice(0).forEach(r => r.dispose()); instances.splice(0).forEach(c => c.dispose()); vi.unstubAllGlobals() })
function setup(response: () => Promise<Response> | Response, target = '/feed', providers = capabilities()) {
  const cookiePolicy = isolatedCookiePolicy()
  const fetch = vi.fn<typeof globalThis.fetch>(async input => (input as Request).url.endsWith('capabilities') ? Response.json(providers) : response())
  const auth = createSessionController({ eventId, fetch, cookiePolicy }); instances.push(auth); auth.start()
  const router = createMemoryRouter(createAppRoutes(createProductionPorts(auth)), { initialEntries: [`/login?returnTo=${encodeURIComponent(target)}`] }); routers.push(router)
  render(<StrictMode><RouterProvider router={router} /></StrictMode>)
  return { auth, router, fetch, cookiePolicy }
}
async function fill() {
  await waitFor(() => expect(screen.getByRole('button', { name: 'Sign in with email' })).toBeEnabled())
  fireEvent.change(screen.getByLabelText('Email', { exact: true }), { target: { value: 'fan@example.com' } })
  fireEvent.change(screen.getByLabelText('Password', { exact: true }), { target: { value: 'test-password' } })
}
it('keeps frozen OAuth disabled and provides labelled autofill fields', async () => {
  const advertised = capabilities(); advertised.providers = advertised.providers.map(p => ({ ...p, enabled: true, disabled_reason: null }))
  setup(() => new Response(null, { status: 401 }), '/feed', advertised); await fill()
  expect(screen.getByLabelText('Email', { exact: true })).toHaveAttribute('autocomplete', 'username')
  expect(screen.getByLabelText('Password', { exact: true })).toHaveAttribute('autocomplete', 'current-password')
  for (const provider of ['Google', 'Apple', 'X']) expect(screen.getByRole('button', { name: `Continue with ${provider}` })).toBeDisabled()
  expect(screen.getByText('Provider sign-in is not available yet.')).toBeVisible()
})
it('disables double submit, clears password, and shows a neutral credential failure', async () => {
  const pending = deferred<Response>(), { fetch } = setup(() => pending.promise); await fill()
  const form = screen.getByRole('form', { name: 'Email sign-in' })
  fireEvent.submit(form); fireEvent.submit(form)
  expect(screen.getByLabelText('Password', { exact: true })).toHaveValue('')
  expect(screen.getByRole('button', { name: 'Signing in…' })).toBeDisabled()
  await waitFor(() => expect(fetch).toHaveBeenCalledTimes(2))
  await act(async () => pending.resolve(Response.json({ code: 'INTERNAL_DEBUG', message: 'Account found: private@example.com' }, { status: 401 })))
  expect(await screen.findByText('We could not sign you in. Check your details or try again later.')).toBeVisible()
  expect(document.body.textContent).not.toContain('private@example.com')
  expect(screen.getByRole('button', { name: 'Sign in with email' })).toBeEnabled()
})
it('bounds UTF-8 passwords before dispatch and keeps a safe destination', async () => {
  const { fetch, router } = setup(() => Response.json(authResponse()), 'https://evil.example/steal'); await fill()
  fireEvent.change(screen.getByLabelText('Password', { exact: true }), { target: { value: 'é'.repeat(65) } })
  fireEvent.submit(screen.getByRole('form', { name: 'Email sign-in' }))
  expect(await screen.findByText('Enter a valid email and a password of 1–128 UTF-8 bytes.')).toBeVisible()
  expect(fetch).toHaveBeenCalledTimes(1); expect(router.state.location.pathname).toBe('/login')
})
it('navigates only to the safe target after successful login', async () => {
  const { router, fetch } = setup(() => Response.json(authResponse()), 'https://evil.example/steal'); await fill()
  fireEvent.submit(screen.getByRole('form', { name: 'Email sign-in' }))
  await waitFor(() => expect(router.state.location.pathname).toBe('/world'))
  expect(fetch.mock.calls.filter(([r]) => (r as Request).url.endsWith('/login'))).toHaveLength(1)
})
it('clears form drafts on a peer identity change', async () => {
  const { cookiePolicy } = setup(() => Response.json(authResponse())); await fill()
  act(() => cookiePolicy.invalidate())
  expect(screen.getByLabelText('Email', { exact: true })).toHaveValue('')
  expect(screen.getByLabelText('Password', { exact: true })).toHaveValue('')
  expect(screen.getByText('Your session changed in another tab. Sign in again to continue.')).toBeVisible()
})
it('ignores successful login completion after the sign-in view unmounts', async () => {
  const delayed = deferred<Response>(), { router, auth } = setup(() => delayed.promise); await fill()
  fireEvent.submit(screen.getByRole('form', { name: 'Email sign-in' }))
  await act(async () => { await router.navigate('/legal/privacy') })
  await act(async () => { delayed.resolve(Response.json(authResponse())) })
  await waitFor(() => expect(auth.read().readiness).toBe('ready'))
  expect(router.state.location.pathname).toBe('/legal/privacy')
})
