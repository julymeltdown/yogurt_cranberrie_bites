// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, expect, it } from 'vitest'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../../../../app/router'
import { createFixturePorts } from '../../../../app/__tests__/fixtures/ports'
import { PublicDocumentsProvider } from '../../model/document-provider'
import { unavailableDocuments } from '../../model/documents'
import { syntheticDocuments } from '../fixtures/documents'
const routers: ReturnType<typeof createMemoryRouter>[] = []
afterEach(() => { cleanup(); routers.splice(0).forEach(r => r.dispose()) })
function setup(path = '/', published = false) {
  const fixture = createFixturePorts('anonymous')
  fixture.ports.session.replace(fixture.ports.session.read().generation, null)
  const router = createMemoryRouter(createAppRoutes(fixture.ports), { initialEntries: [path] }); routers.push(router)
  render(<PublicDocumentsProvider provider={published ? syntheticDocuments : unavailableDocuments}><RouterProvider router={router} /></PublicDocumentsProvider>)
  return fixture
}
it('keeps anonymous landing entry actionable through the default App without mounting a world', async () => {
  const fixture = setup()
  const entry = await screen.findByRole('link', { name: 'Sign in to enter' })
  expect(entry).toHaveAttribute('href', '/login?returnTo=%2Fworld')
  expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent('One world. Every era.')
  expect(screen.getByRole('status')).toHaveTextContent('Sign in to check entry')
  expect(fixture.stats.mounts).toBe(0)
  expect(document.querySelector('.landing-scene,.landing-copy-print')).toBeNull()
  fireEvent.click(entry)
  expect(await screen.findByRole('heading', { name: 'Sign in', level: 1 })).toBeVisible()
  expect(fixture.stats.mounts).toBe(0)
})
it('navigates to unavailable legal content with heading focus and preserves disclosure text', async () => {
  setup()
  fireEvent.click(within(await screen.findByRole('navigation', { name: 'Public information' })).getByRole('link', { name: 'Terms' }))
  const heading = await screen.findByRole('heading', { level: 1, name: 'Terms' })
  expect(heading).toHaveFocus()
  expect(await screen.findByText('This document is not available yet.')).toBeVisible()
  expect(screen.getByText('An independent fan project. Not affiliated with My Chemical Romance or concert organizers.')).toBeVisible()
  expect(screen.queryByText(/TEST-2026/)).toBeNull()
})
it('renders published version metadata only from the explicitly synthetic document provider', async () => {
  setup('/legal/privacy', true)
  expect(await screen.findByText('TEST-2026.11.03')).toBeVisible()
  expect(screen.getByRole('note')).toHaveTextContent('not approved policy')
  expect(screen.getByText('Dates shown in Korea Standard Time.')).toBeVisible()
})
it('does not turn missing retained deletion state into successful cleanup', async () => {
  setup('/account/deleted')
  expect(await screen.findByRole('heading', { name: 'Account deletion status' })).toHaveFocus()
  expect(screen.getByRole('status')).toHaveTextContent('No deletion completion is confirmed')
  expect(screen.getByText(/Completion requires all eight cleanup steps/)).toBeVisible()
  expect(screen.queryByRole('button', { name: 'Check deletion progress' })).toBeNull()
})
