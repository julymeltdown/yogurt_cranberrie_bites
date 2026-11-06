// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../../router'
import { createFixturePorts } from '../fixtures/ports'

const owned: (() => void)[] = []
afterEach(() => { cleanup(); owned.splice(0).forEach(dispose => dispose()); vi.unstubAllGlobals() })
async function shell() {
  vi.stubGlobal('matchMedia', () => ({ matches: true, addEventListener() {}, removeEventListener() {} }))
  const fixture = createFixturePorts()
  const router = createMemoryRouter(createAppRoutes(fixture.ports), { initialEntries: ['/world'] })
  owned.push(() => router.dispose())
  render(<RouterProvider router={router} />)
  const toggle = await screen.findByRole('button', { name: 'World tools' })
  return { fixture, router, toggle }
}

it('keeps the World main/skip target and exposes all four original destinations through the disclosure', async () => {
  const { toggle } = await shell()
  expect(document.querySelector('main#shell-content')).toHaveAttribute('tabindex', '-1')
  expect(screen.getByRole('link', { name: 'Skip to content' })).toHaveAttribute('href', '#shell-content')
  expect(toggle).toHaveAttribute('aria-expanded', 'false')
  expect(screen.queryByRole('navigation', { name: 'World tools' })).not.toBeInTheDocument()
  fireEvent.click(toggle)
  expect(toggle).toHaveAttribute('aria-expanded', 'true')
  const nav = screen.getByRole('navigation', { name: 'World tools' })
  for (const [name, destination] of [['Map', 'map'], ['Rooms', 'rooms'], ['Nearby', 'nearby'], ['Help', 'help']] as const) {
    expect(within(nav).getByRole('link', { name })).toHaveAttribute('href', `/world/${destination}`)
    expect(within(nav).getByRole('link', { name })).toHaveAttribute('id', `world-${destination}`)
  }
})

it('closes with Escape and returns focus without navigating or remounting the world', async () => {
  const { toggle, router, fixture } = await shell()
  fireEvent.click(toggle)
  const map = within(screen.getByRole('navigation', { name: 'World tools' })).getByRole('link', { name: 'Map' })
  map.focus()
  fireEvent.keyDown(map, { key: 'Escape' })
  expect(screen.getByRole('button', { name: 'World destinations' })).toHaveFocus()
  expect(toggle).toHaveAttribute('aria-expanded', 'false')
  expect(router.state.location.pathname).toBe('/world')
  expect(fixture.stats.mounts).toBe(1)
})

it.each(['Map', 'Rooms', 'Nearby', 'Help'])('returns from %s to the expanded originating tool with the same world', async name => {
  const { toggle, fixture, router } = await shell()
  const host = document.querySelector('[data-world-host]')
  fireEvent.click(toggle)
  const opener = within(screen.getByRole('navigation', { name: 'World tools' })).getByRole('link', { name })
  opener.focus()
  fireEvent.click(opener)
  await screen.findByRole('button', { name: 'Back to previous panel' })
  expect(router.state.location.pathname).toBe(`/world/${name.toLowerCase()}`)
  fireEvent.click(screen.getByRole('button', { name: 'Back to previous panel' }))
  await screen.findByRole('button', { name: 'World tools', expanded: true })
  expect(document.getElementById(opener.id)).toHaveFocus()
  expect(document.querySelector('[data-world-host]')).toBe(host)
  expect(fixture.stats.mounts).toBe(1)
})

it('retains six primary destinations and panel full/half/peek behavior', async () => {
  const { router, fixture } = await shell()
  const host = document.querySelector('[data-world-host]')
  for (const name of ['World', 'Feed', 'Meet', 'Gifts', 'Wall', 'Me']) expect(screen.getByRole('link', { name })).toBeVisible()
  await act(() => router.navigate('/feed'))
  expect(document.querySelector('.world-shell')).toHaveAttribute('data-panel-size', 'full')
  fireEvent.click(screen.getByRole('button', { name: 'Reduce panel' }))
  expect(document.querySelector('.world-shell')).toHaveAttribute('data-panel-size', 'half')
  fireEvent.click(screen.getByRole('button', { name: 'Minimize panel' }))
  expect(document.querySelector('.shell-panel-body')).not.toBeVisible()
  await act(() => router.navigate('/world'))
  expect(document.querySelector('[data-world-host]')).toBe(host)
  expect(fixture.stats.mounts).toBe(1)
})
