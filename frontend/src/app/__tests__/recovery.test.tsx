// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { StrictMode } from 'react'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../router'
import { createProductionPorts } from '../ports'
import { createSessionController, type SessionController } from '../../features/auth/model/session'
import { deferred, eventId, isolatedCookiePolicy, userId } from '../../features/auth/__tests__/fixtures/data'
const token = 'B'.repeat(43), secret = '  new-password-secret  '
const routers: ReturnType<typeof createMemoryRouter>[] = [], controllers: SessionController[] = []
afterEach(() => { cleanup(); routers.splice(0).forEach(r => r.dispose()); controllers.splice(0).forEach(c => c.dispose()) })
function setup(path = '/reset-password', handler: (r: Request) => Response | Promise<Response> = () => Response.json({ status: 'SENT' })) {
  const requests: Request[] = []
  const fetch = vi.fn<typeof globalThis.fetch>(async input => {
    const r = input as Request
    requests.push(r.clone())
    return r.url.endsWith('/logout') ? new Response(null, { status: 204 }) : handler(r)
  })
  const auth = createSessionController({ eventId: null, fetch, cookiePolicy: isolatedCookiePolicy(), timeoutMs: 1000 }); controllers.push(auth)
  const router = createMemoryRouter(createAppRoutes(createProductionPorts(auth)), { initialEntries: ['/legal/privacy', path] }); routers.push(router)
  const view = render(<StrictMode><RouterProvider router={router} /></StrictMode>)
  return { router, auth, fetch, requests, ...view }
}
async function confirmForm() {
  await screen.findByRole('heading', { name: 'Reset password' })
  fireEvent.click(screen.getByRole('button', { name: 'I have a reset token' }))
  fireEvent.change(screen.getByLabelText('Reset token'), { target: { value: token } })
  fireEvent.change(screen.getByLabelText('New password'), { target: { value: secret } })
}
it('renders usable request/explicit-token forms without a production event or automatic network calls', async () => {
  const { fetch } = setup()
  expect(await screen.findByRole('heading', { name: 'Reset password' })).toHaveFocus()
  expect(screen.getByLabelText('Email')).toBeEnabled(); expect(fetch).not.toHaveBeenCalled()
  fireEvent.click(screen.getByRole('button', { name: 'I have a reset token' }))
  expect(screen.getByLabelText('Reset token')).toHaveAttribute('type', 'password')
  expect(screen.getByLabelText('New password')).toHaveAttribute('autocomplete', 'new-password')
  expect(document.querySelector('canvas,[data-world-host]')).toBeNull()
})
it('keeps unknown-email request feedback neutral about existence and delivery and does not reflect server status', async () => {
  const { fetch } = setup('/reset-password', () => Response.json({ status: 'SOURCE_PRIVATE_SENT' }))
  fireEvent.change(await screen.findByLabelText('Email'), { target: { value: 'unknown@example.com' } })
  fireEvent.submit(screen.getByRole('form', { name: 'Request password reset' }))
  const feedback = await screen.findByText(/Account details and email delivery are not confirmed here/)
  expect(feedback.closest('[role="status"]')).toHaveFocus()
  expect(document.body.textContent).not.toContain('SOURCE_PRIVATE_SENT'); expect(fetch).toHaveBeenCalledOnce()
})
it('imports the exact source fragment only into transient memory, replacing history before rendering the form', async () => {
  const { router, fetch, requests } = setup(`/reset-password?returnTo=%2Ffeed#token=${token}`, () => Response.json({ status: 'RESET' }))
  expect(await screen.findByText('Reset link ready. Enter your new password below.')).toBeVisible()
  expect(router.state.location).toMatchObject({ pathname: '/reset-password', search: '', hash: '', state: null })
  expect(router.state.historyAction).toBe('REPLACE'); expect(screen.queryByLabelText('Reset token')).toBeNull()
  expect(document.body.innerHTML).not.toContain(token); expect(fetch).not.toHaveBeenCalled()
  fireEvent.change(screen.getByLabelText('New password'), { target: { value: secret } })
  fireEvent.submit(screen.getByRole('form', { name: 'Confirm password reset' }))
  await screen.findByText('Password reset completed. Sign in again with your new password.')
  expect(await requests[0]!.json()).toEqual({ token, newPassword: secret })
  expect(document.body.innerHTML).not.toContain(secret); expect(screen.getByRole('link', { name: 'Back to sign in' })).toHaveAttribute('href', '/login?returnTo=%2Ffeed')
  await act(async () => { await router.navigate(-1) }); expect(router.state.location.pathname).toBe('/legal/privacy')
})
it.each(['?token=private-query-token', '#token=short', '?returnTo=https%3A%2F%2Fevil.example#token=short'])('scrubs unusable reset URL %s and offers an explicit recovery path', async suffix => {
  const { router, fetch } = setup(`/reset-password${suffix}`)
  expect(await screen.findByText(/This reset link could not be used/)).toBeVisible()
  expect(router.state.location.search + router.state.location.hash).toBe('')
  expect(document.body.innerHTML).not.toContain('private-query-token'); expect(document.body.innerHTML).not.toContain('evil.example')
  expect(fetch).not.toHaveBeenCalled(); expect(screen.getByRole('button', { name: 'I have a reset token' })).toBeEnabled()
})
it('guards duplicate form submissions and clears token/password values immediately', async () => {
  const delayed = deferred<Response>(), { fetch } = setup('/reset-password', () => delayed.promise)
  await confirmForm(); const form = screen.getByRole('form', { name: 'Confirm password reset' })
  fireEvent.submit(form); fireEvent.submit(form)
  expect(screen.getByLabelText('Reset token')).toHaveValue(''); expect(screen.getByLabelText('New password')).toHaveValue('')
  expect(screen.getByRole('button', { name: 'Reset password' })).toBeDisabled()
  await waitFor(() => expect(fetch).toHaveBeenCalledOnce())
  await act(async () => delayed.resolve(Response.json({ status: 'RESET' })))
  expect(await screen.findByText('Password reset completed. Sign in again with your new password.')).toBeVisible()
})
it('shows safe expired-token feedback without raw details or automatic retry', async () => {
  const { fetch } = setup('/reset-password', () => Response.json({ code: 'RAW', message: `private ${token} ${secret}` }, { status: 400 }))
  await confirmForm(); fireEvent.submit(screen.getByRole('form', { name: 'Confirm password reset' }))
  await screen.findByText(/It may be invalid, expired or already used/)
  expect(document.body.textContent).not.toContain('private'); expect(fetch).toHaveBeenCalledOnce()
  expect(screen.getByRole('button', { name: 'Request another reset' })).toBeEnabled()
})
it('does not claim success or retain secrets when the dispatched mutation has an uncertain outcome', async () => {
  const { fetch } = setup('/reset-password', () => { throw new TypeError(`private ${secret}`) })
  await confirmForm(); fireEvent.submit(screen.getByRole('form', { name: 'Confirm password reset' }))
  await screen.findByText(/we could not confirm the result/)
  expect(screen.queryByText(/Password reset completed/)).toBeNull(); expect(document.body.innerHTML).not.toContain(secret)
  expect(screen.getByLabelText('Reset token')).toHaveValue(''); expect(fetch).toHaveBeenCalledOnce()
})
it('keeps uncertain request outcomes neutral about account existence and delivery', async () => {
  setup('/reset-password', () => { throw new TypeError('raw private request error') })
  fireEvent.change(await screen.findByLabelText('Email'), { target: { value: 'unknown@example.com' } })
  fireEvent.submit(screen.getByRole('form', { name: 'Request password reset' }))
  expect(await screen.findByText('Your request may have been processed, but the result and email delivery are not confirmed. You can try again later.')).toBeVisible()
  expect(screen.queryByText(/The reset may have completed/)).toBeNull()
})
it.each(['logout', 'account-change'] as const)('clears draft and fences pending feedback after %s', async change => {
  const delayed = deferred<Response>(), { auth, fetch } = setup('/reset-password', () => delayed.promise)
  await confirmForm(); fireEvent.submit(screen.getByRole('form', { name: 'Confirm password reset' }))
  await waitFor(() => expect(fetch).toHaveBeenCalledOnce())
  await act(async () => {
    if (change === 'logout') await auth.logout()
    else auth.session.replace(auth.session.read().generation, { userId, eventId, accessToken: 'replacement-memory-token' })
  })
  expect(screen.getByText('Your session changed. Reset details have been cleared.')).toBeVisible()
  expect(screen.getByLabelText('New password')).toHaveValue(''); expect(screen.getByRole('button', { name: 'Reset password' })).toBeEnabled()
  await act(async () => delayed.resolve(Response.json({ status: 'RESET' })))
  expect(screen.queryByText(/Password reset completed/)).toBeNull()
})
it('clears unsent recovery drafts on an identity invalidation', async () => {
  const { auth, fetch } = setup(); await confirmForm()
  await act(async () => { auth.session.replace(auth.session.read().generation, null) })
  expect(screen.getByLabelText('New password')).toHaveValue(''); expect(screen.getByLabelText('Reset token')).toHaveValue('')
  expect(fetch).not.toHaveBeenCalled()
})
it('does not carry a fragment token into a fresh clean navigation to the same reset route', async () => {
  const { router, fetch } = setup(`/reset-password#token=${token}`)
  await screen.findByText('Reset link ready. Enter your new password below.')
  await act(async () => { await router.navigate('/reset-password') })
  expect(screen.getByLabelText('Email')).toBeVisible(); expect(screen.queryByText(/Reset link ready/)).toBeNull()
  fireEvent.click(screen.getByRole('button', { name: 'I have a reset token' }))
  expect(screen.getByLabelText('Reset token')).toHaveValue(''); expect(fetch).not.toHaveBeenCalled()
})
it('cancels pending work on a fresh clean navigation within the same reset route', async () => {
  const delayed = deferred<Response>(), { fetch, router } = setup('/reset-password', () => delayed.promise)
  await confirmForm(); fireEvent.submit(screen.getByRole('form', { name: 'Confirm password reset' }))
  await waitFor(() => expect(fetch).toHaveBeenCalledOnce())
  await act(async () => { await router.navigate('/reset-password') })
  expect((fetch.mock.calls[0]![0] as Request).signal.aborted).toBe(true)
  await act(async () => delayed.resolve(Response.json({ status: 'RESET' })))
  expect(screen.queryByText(/Password reset completed/)).toBeNull(); expect(screen.getByLabelText('Email')).toHaveValue('')
})
it('aborts a pending recovery on unmount and ignores its late completion', async () => {
  const delayed = deferred<Response>(), { fetch, router } = setup('/reset-password', () => delayed.promise)
  await confirmForm(); fireEvent.submit(screen.getByRole('form', { name: 'Confirm password reset' }))
  await waitFor(() => expect(fetch).toHaveBeenCalledOnce())
  await act(async () => { await router.navigate('/legal/privacy') })
  expect((fetch.mock.calls[0]![0] as Request).signal.aborted).toBe(true)
  await act(async () => delayed.resolve(Response.json({ status: 'RESET' })))
  expect(await screen.findByRole('heading', { name: 'Privacy' })).toBeVisible(); expect(screen.queryByText(/Password reset completed/)).toBeNull()
})
it.each([
  ['', 'Provider sign-in is unavailable'],
  ['?error=access_denied&state=PRIVATE_STATE&error_description=PRIVATE_DESCRIPTION&returnTo=%2Ffeed', 'Sign-in cancelled'],
  ['?code=PRIVATE_CODE&state=PRIVATE_STATE&returnTo=https%3A%2F%2Fevil.example', 'Sign-in could not be verified'],
  ['#access_token=PRIVATE_TOKEN', 'Sign-in could not be verified'],
] as const)('presents a safe callback state for %s without requests, credential import or URL reflection', async (suffix, title) => {
  const { auth, router, fetch } = setup(`/auth/callback${suffix}`)
  expect(await screen.findByRole('heading', { name: title })).toHaveFocus()
  expect(router.state.location).toMatchObject({ pathname: '/auth/callback', search: '', hash: '', state: null })
  expect(document.body.innerHTML).not.toMatch(/PRIVATE_|evil\.example/)
  expect(auth.session.read().accessToken).toBeNull(); expect(fetch).not.toHaveBeenCalled()
  expect(screen.getByRole('link', { name: 'Back to your destination' })).toHaveAttribute('href', title === 'Sign-in cancelled' ? '/feed' : '/world')
  expect(document.querySelector('canvas,[data-world-host]')).toBeNull()
})
it('keeps the existing sign-in composition and all OAuth providers disabled after callback return', async () => {
  const { router } = setup('/auth/callback?error=access_denied')
  fireEvent.click(await screen.findByRole('link', { name: 'Back to sign in' }))
  expect(await screen.findByRole('heading', { name: 'Sign in' })).toBeVisible()
  expect(router.state.location.pathname).toBe('/login')
  expect(screen.getByText('Event entry is not open yet. Please check back later.')).toBeVisible()
})
it('does not revoke an existing canonical account or reuse old denial projection on a fresh clean callback navigation', async () => {
  const { auth, router, fetch } = setup('/auth/callback?error=access_denied&returnTo=%2Ffeed')
  await screen.findByRole('heading', { name: 'Sign-in cancelled' })
  const identity = auth.session.replace(auth.session.read().generation, { userId, eventId, accessToken: 'existing-canonical-token' })
  await act(async () => { await router.navigate('/auth/callback') })
  expect(screen.getByRole('heading', { name: 'Provider sign-in is unavailable' })).toBeVisible()
  expect(screen.getByRole('link', { name: 'Back to your destination' })).toHaveAttribute('href', '/world')
  expect(auth.session.isCurrent(identity!)).toBe(true); expect(auth.session.read().accessToken).toBe('existing-canonical-token')
  expect(fetch).not.toHaveBeenCalled()
})
