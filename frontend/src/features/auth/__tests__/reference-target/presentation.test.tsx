// @vitest-environment jsdom
import { cleanup, render, screen, waitFor } from '@testing-library/react'
import { afterEach, expect, it } from 'vitest'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { PortsProvider } from '../../../../app/access'
import { SignIn } from '../../ui/SignIn'
import { createSessionController } from '../../model/session'
import { capabilities, eventId, isolatedCookiePolicy } from '../fixtures/data'
afterEach(cleanup)
it('uses actual fields, disabled provider marks, cemetery scenery and no invented remember-session control', async () => {
  const auth = createSessionController({ eventId, cookiePolicy: isolatedCookiePolicy(), fetch: async () => Response.json(capabilities()) }); auth.start()
  const router = createMemoryRouter([{ path: '/login', element: <SignIn /> }], { initialEntries: ['/login'] })
  const view = render(<PortsProvider ports={{ auth, session: auth.session, readiness: auth.readiness, bootstrap: { load: auth.loadBootstrap }, admin: { authorize: async () => false } }}><RouterProvider router={router} /></PortsProvider>)
  await waitFor(() => expect((screen.getByLabelText('Email') as HTMLInputElement).disabled).toBe(false))
  expect(screen.getByRole('heading', { name: 'Sign in', level: 1 })).toBeTruthy()
  expect(screen.queryByRole('checkbox')).toBeNull()
  expect(screen.getByRole('button', { name: 'Sign in with email' }).textContent).toBe('SIGN IN')
  for (const name of ['X', 'Discord', 'Google']) {
    const button = screen.getByRole('button', { name: `Continue with ${name}` }) as HTMLButtonElement
    expect(button.disabled).toBe(true); expect(button.querySelector('svg[aria-hidden=true]')).toBeTruthy()
  }
  expect(view.container.querySelectorAll('.auth-cemetery')).toHaveLength(2)
  expect(screen.getByRole('link', { name: 'Forgot password?' }).getAttribute('href')).toBe('/reset-password')
  expect(screen.getByRole('link', { name: 'Create an account' }).getAttribute('href')).toBe('/signup')
  router.dispose(); auth.dispose()
})
