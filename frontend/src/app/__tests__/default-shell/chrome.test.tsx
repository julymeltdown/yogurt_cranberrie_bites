// @vitest-environment jsdom
import { afterEach, expect, it } from 'vitest'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../../router'
import { createFixturePorts } from '../fixtures/ports'
const routers: ReturnType<typeof createMemoryRouter>[] = []
afterEach(() => { cleanup(); routers.splice(0).forEach(router => router.dispose()) })
async function setup(path = '/world') {
  const fixture = createFixturePorts(), router = createMemoryRouter(createAppRoutes(fixture.ports), { initialEntries: [path] })
  routers.push(router); render(<RouterProvider router={router} />)
  await screen.findByRole('navigation', { name: 'Main navigation' }); return { ...fixture, router }
}
it('actual default shell adopts one compact chrome set and truthful unavailable era', async () => {
  const h = await setup()
  expect(document.querySelectorAll('.mcr-chrome-header')).toHaveLength(1)
  expect(document.querySelectorAll('.mcr-chrome-bottom')).toHaveLength(1)
  expect(document.querySelectorAll('.shell-header,.shell-tabs')).toHaveLength(0)
  expect(screen.getByText('Eras unavailable.')).toBeTruthy(); expect(document.querySelector('[aria-pressed="true"]')).toBeNull()
  expect(h.stats.mounts).toBe(1); expect(h.stats.connections).toBe(1)
})
it('default six routes, Settings and Inbox preserve sole world and Back focus', async () => {
  const h = await setup()
  for (const label of ['Feed', 'Meet', 'Gifts', 'Wall', 'Me']) {
    fireEvent.click(screen.getByRole('link', { name: label }))
    await screen.findByRole('heading', { name: label, level: 1 })
    expect(screen.getByRole('link', { name: label }).getAttribute('aria-current')).toBe('page')
    expect(h.stats.mounts).toBe(1); expect(h.stats.connections).toBe(1)
  }
  fireEvent.click(screen.getByRole('link', { name: 'World' })); await screen.findByRole('navigation', { name: 'World tools' })
  fireEvent.click(screen.getByRole('link', { name: 'Inbox' })); await screen.findByRole('heading', { name: 'Inbox', level: 1 })
  fireEvent.click(screen.getByRole('button', { name: 'Back to previous panel' })); await screen.findByRole('navigation', { name: 'World tools' })
  await waitFor(() => expect(document.activeElement?.id).toBe('shell-inbox'))
  fireEvent.click(screen.getByRole('link', { name: 'Settings' })); await screen.findByRole('heading', { name: 'Settings', level: 1 })
  expect(h.router.state.location.pathname).toBe('/me/settings'); expect(h.stats.mounts).toBe(1)
})
it('actual panel header preserves full/peek/half behavior and coverage input', async () => {
  const h = await setup('/world/map')
  fireEvent.click(screen.getByRole('button', { name: 'Expand panel' })); expect(h.stats.visible).toBe(false)
  fireEvent.click(screen.getByRole('button', { name: 'Minimize panel' })); expect(screen.getByRole('button', { name: 'Show panel content' })).toBeTruthy()
  fireEvent.click(screen.getByRole('button', { name: 'Show panel content' })); expect(screen.getByRole('button', { name: 'Expand panel' })).toBeTruthy()
  expect(h.stats.visible).toBe(true); expect(h.stats.inputBlocked).toBe(true); expect(h.stats.mounts).toBe(1)
})
