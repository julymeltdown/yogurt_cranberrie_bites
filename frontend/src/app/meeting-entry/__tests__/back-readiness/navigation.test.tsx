// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { StrictMode } from 'react'
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../../../router'
import type { ActiveScope, ShellPorts } from '../../../ports'
import { bootstrapFor, createFixturePorts } from '../../../__tests__/fixtures/ports'
import { fixture } from '../../../../features/gift-discovery/__tests__/fixtures/data'

// Actual routes, navigation and meeting guidance; only the existing shell/auth ports
// provide deterministic synthetic data. No route, navigate or bridge is mocked.
const resources: (() => void)[] = []
const beaconPath = '/meet/beacons/00000000-0000-4000-8000-000000000015'
const unavailable = 'Real-world meeting guidance is unavailable until current place, audience, and expiry information is verified.'
const backName = 'Back to previous panel'
beforeEach(() => { vi.stubGlobal('location', { origin: 'https://mcr.example.invalid' }) })
afterEach(() => { cleanup(); resources.splice(0).forEach(stop => stop()); vi.unstubAllGlobals() })

function setup(strict = false) {
  const auth = fixture(), world = createFixturePorts(); auth.controller.dispose()
  const ports: ShellPorts = { ...world.ports, auth: auth.auth, session: auth.auth.session,
    bootstrap: { load: async (scope: ActiveScope) => bootstrapFor(scope) } }
  const router = createMemoryRouter(createAppRoutes(ports), { initialEntries: [beaconPath] })
  resources.push(() => { router.dispose(); auth.dispose() })
  const node = <RouterProvider router={router} />
  return { ...render(strict ? <StrictMode>{node}</StrictMode> : node), router, stats: world.stats, requests: auth.requests }
}
const panel = (name: string) => screen.getByRole('main', { name })
const back = (name: string) => within(panel(name)).getByRole('button', { name: backName })

it('separates router publication from the committed panel and demonstrates the stale Back handler', async () => {
  const f = setup()
  expect(await screen.findByText(unavailable)).toBeVisible()
  const host = document.querySelector('[data-world-host]'), mounts = f.stats.mounts
  const observations: { pathname: string; heading: string | null }[] = []
  const stop = f.router.subscribe(state => {
    observations.push({ pathname: state.location.pathname, heading: document.getElementById('panel-title')?.textContent ?? null })
  })
  resources.push(stop)
  // Hold React's commit queue with its public act boundary. Both real DOM clicks
  // occur before that boundary releases; router publication itself is not held.
  await act(async () => {
    fireEvent.click(back('Find each other'))
    expect(f.router.state.location.pathname).toBe('/meet')
    expect(panel('Find each other')).toBeVisible()
    expect(screen.queryByRole('main', { name: 'Meet' })).not.toBeInTheDocument()
    fireEvent.click(back('Find each other'))
    expect(f.router.state.location.pathname).toBe('/meet')
  })
  expect(observations).toEqual([
    { pathname: '/meet', heading: 'Find each other' },
    { pathname: '/meet', heading: 'Find each other' },
  ])
  expect(await screen.findByRole('main', { name: 'Meet' })).toBeVisible()
  expect(f.router.state.location.pathname).toBe('/meet')
  expect(document.querySelector('[data-world-host]')).toBe(host)
  expect(f.stats.mounts).toBe(mounts)
  fireEvent.click(back('Meet'))
  expect(await screen.findByRole('navigation', { name: 'World tools' })).toBeVisible()
  expect(f.router.state.location.pathname).toBe('/world')
  expect(f.stats).toMatchObject({ games: 1, connections: 1, peakGames: 1, peakConnections: 1, mounts: 1 })
  expect(f.requests.filter(r => new URL(r.url).pathname.includes('/beacons'))).toHaveLength(0)
  f.unmount()
  expect(f.stats).toMatchObject({ games: 0, connections: 0, disposals: 1 })
})

it.each([false, true])('observes the committed Meet panel before its next Back click (StrictMode=%s)', async strict => {
  const f = setup(strict)
  expect(await screen.findByText(unavailable)).toBeVisible()
  const host = document.querySelector('[data-world-host]'), mounts = f.stats.mounts
  fireEvent.click(screen.getByRole('button', { name: backName }))
  await waitFor(() => expect(f.router.state.location.pathname).toBe('/meet'))
  // The proposed original-test correction: observe the committed destination.
  expect(await screen.findByRole('heading', { name: 'Meet', level: 1 })).toBeVisible()
  expect(f.router.state.location.pathname).toBe('/meet')
  expect(screen.queryByText(unavailable)).not.toBeInTheDocument()
  fireEvent.click(screen.getByRole('button', { name: backName }))
  expect(await screen.findByRole('navigation', { name: 'World tools' })).toBeVisible()
  expect(f.router.state.location.pathname).toBe('/world')
  expect(screen.queryByRole('button', { name: backName })).not.toBeInTheDocument()
  expect(document.querySelector('[data-world-host]')).toBe(host)
  expect(f.stats.mounts).toBe(mounts)
  if (!strict) expect(f.stats.mounts).toBe(1)
  expect(f.stats).toMatchObject({ games: 1, connections: 1, peakGames: 1, peakConnections: 1 })
  expect(f.requests.filter(r => new URL(r.url).pathname.includes('/beacons'))).toHaveLength(0)
  f.unmount()
  await waitFor(() => expect(f.stats).toMatchObject({ games: 0, connections: 0, disposals: mounts }))
})
