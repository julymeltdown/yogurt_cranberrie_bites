// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../../router'
import { createProductionPorts } from '../../ports'
import { authHarness } from '../../world-entry/__tests__/admitted-auth'

afterEach(cleanup)
it('uses the default catchall with safe navigation and never renders the rejected URL', async () => {
  const { auth } = authHarness(), ports = createProductionPorts(auth)
  const router = createMemoryRouter(createAppRoutes(ports), { initialEntries: ['/unknown-private-identifier?token=private-token'] })
  try {
    render(<RouterProvider router={router} />)
    expect(await screen.findByRole('heading', { name: 'Page not found' })).toBeVisible()
    expect(document.body).not.toHaveTextContent('unknown-private-identifier'); expect(document.body).not.toHaveTextContent('private-token')
    expect(screen.getByRole('link', { name: 'Back to world' })).toHaveAttribute('href', '/world')
    expect(screen.getByRole('link', { name: 'Back to home' })).toHaveAttribute('href', '/')
  } finally { cleanup(); router.dispose(); ports.dispose?.(); auth.dispose() }
})
it('uses the default route error element without printing the thrown diagnostic', async () => {
  const { auth } = authHarness(), ports = createProductionPorts(auth), routes = createAppRoutes(ports)
  routes[0]!.children!.unshift({ path: '/failed-fixture', loader: () => { throw new Error('private fixture diagnostic and bearer') }, element: <p>Not reachable</p> })
  const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {})
  const router = createMemoryRouter(routes, { initialEntries: ['/failed-fixture'] })
  try {
    render(<RouterProvider router={router} />)
    expect(await screen.findByRole('heading', { name: 'This page is unavailable' })).toBeVisible()
    expect(document.body).not.toHaveTextContent('private fixture diagnostic'); expect(document.body).not.toHaveTextContent('Not reachable')
    expect(screen.getByRole('button', { name: 'Reload page' })).toBeEnabled()
  } finally { cleanup(); router.dispose(); ports.dispose?.(); auth.dispose(); consoleError.mockRestore() }
})
