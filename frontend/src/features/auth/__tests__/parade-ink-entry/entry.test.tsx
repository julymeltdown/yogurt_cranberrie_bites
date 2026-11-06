// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../../../../app/router'
import { createProductionPorts } from '../../../../app/ports'
import { createSessionController, type SessionController } from '../../model/session'
import { capabilities, eventId, isolatedCookiePolicy } from '../fixtures/data'
import { withAdmission } from '../fixtures/admission'
const routers: ReturnType<typeof createMemoryRouter>[] = [], controllers: SessionController[] = []
afterEach(() => { cleanup(); routers.splice(0).forEach(r => r.dispose()); controllers.splice(0).forEach(c => c.dispose()) })
async function setup(path: string) {
  const requests: string[] = []
  const auth = createSessionController({ eventId, cookiePolicy: isolatedCookiePolicy(), fetch: withAdmission(async input => {
    const url = (input as Request).url; requests.push(url)
    return url.endsWith('/capabilities') ? Response.json(capabilities()) : Response.json({ code: 'UNAVAILABLE', message: 'Synthetic unavailable' }, { status: 503 })
  }) }); controllers.push(auth); auth.start()
  await vi.waitFor(() => expect(auth.read().capabilities).toBe('ready'))
  const router = createMemoryRouter(createAppRoutes(createProductionPorts(auth)), { initialEntries: [path] }); routers.push(router)
  render(<RouterProvider router={router} />)
  return { auth, router, requests }
}
it('retains native labels, unavailable providers, secret clearing and truthful sign-in error', async () => {
  const { auth } = await setup('/login')
  fireEvent.change(await screen.findByLabelText('Email'), { target: { value: 'synthetic@example.invalid' } })
  fireEvent.change(screen.getByLabelText('Password'), { target: { value: 'synthetic-password' } })
  fireEvent.submit(screen.getByRole('form', { name: 'Email sign-in' }))
  expect(screen.getByLabelText('Password')).toHaveValue('')
  expect(await screen.findByText('Sign-in could not be completed. Try again when the service is available.')).toBeVisible()
  expect(auth.session.read().userId).toBeNull()
  for (const provider of ['X', 'Discord', 'Google']) expect(screen.getByRole('button', { name: `Continue with ${provider}` })).toBeDisabled()
})
it.each([['/signup', 'Create account'], ['/verify-email', 'Verify email']])('keeps account forms gated and reachable at %s', async (path, title) => {
  await setup(path)
  expect(await screen.findByRole('heading', { name: title, level: 1 })).toHaveFocus()
  expect(screen.getByRole('button', { name: title })).toBeDisabled()
  expect(screen.getByRole('link', { name: 'Back to home' })).toHaveAttribute('href', '/')
})
it('preserves password recovery mode controls and focus after an uncertain request', async () => {
  await setup('/reset-password')
  fireEvent.change(await screen.findByLabelText('Email'), { target: { value: 'synthetic@example.invalid' } })
  fireEvent.submit(screen.getByRole('form'))
  const message = await screen.findByText('Your request may have been processed, but the result and email delivery are not confirmed. You can try again later.')
  expect(message.closest('[role=status]')).toHaveFocus()
  fireEvent.click(screen.getByRole('button', { name: 'I have a reset token' }))
  expect(screen.getByLabelText('Reset token')).toBeVisible()
  expect(screen.getByLabelText('New password')).toHaveAttribute('type', 'password')
})
it('scrubs provider callback secrets and exposes a safe sign-in continuation', async () => {
  const { router, auth } = await setup('/auth/callback?error=access_denied&returnTo=https%3A%2F%2Fevil.invalid#secret')
  await act(async () => { await vi.waitFor(() => expect(router.state.location.search + router.state.location.hash).toBe('')) })
  expect(await screen.findByRole('heading', { name: 'Sign-in could not be verified' })).toHaveFocus()
  expect(screen.getByRole('link', { name: 'Back to sign in' })).toHaveAttribute('href', '/login?returnTo=%2Fworld')
  expect(auth.session.read().userId).toBeNull()
  expect(document.body.innerHTML).not.toContain('evil.invalid')
})
