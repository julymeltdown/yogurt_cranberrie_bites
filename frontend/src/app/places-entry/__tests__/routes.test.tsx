// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { StrictMode } from 'react'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../../router'
import { PlacesEntryProvider, PlaceDetailRoute, PlacesOverviewRoute, usePlacesEntry } from '../provider'
import { entryFixture } from './fixture'
import { places } from '../../../features/places/__tests__/fixtures/data'
import type { PlacesController } from '../../../features/places'
const owned: (() => void)[] = []
beforeEach(() => { vi.stubGlobal('location', { origin: 'https://mcr.example.invalid' }) })
afterEach(() => { cleanup(); owned.splice(0).reverse().forEach(stop => stop()); vi.unstubAllGlobals() })
async function setup(path = '/world/map', strict = false) {
  const h = await entryFixture(); owned.push(h.dispose)
  let current: PlacesController | null = null
  function Probe() { current = usePlacesEntry(); return null }
  const router = createMemoryRouter(createAppRoutes(h.ports, { panels: {
    '/world/map': <PlacesOverviewRoute />, '/world/places/:placeId': <PlaceDetailRoute />,
  } }), { initialEntries: [path] }); owned.push(() => router.dispose())
  const node = <PlacesEntryProvider auth={h.auth} source={h.source}><Probe /><RouterProvider router={router} /></PlacesEntryProvider>
  return { ...h, ...render(strict ? <StrictMode>{node}</StrictMode> : node), router, current: () => current }
}
it.each([false, true])('retains one Places owner and world lease through map/detail/All places/header Back (StrictMode=%s)', async strict => {
  const h = await setup('/world/map', strict)
  const label = places()[0]!.label
  await screen.findByRole('link', { name: `View ${label}` })
  const controller = h.current(), host = document.querySelector('[data-world-host]'), mounts = h.stats.mounts
  fireEvent.click(screen.getByRole('link', { name: `View ${label}` }))
  expect(await screen.findByRole('heading', { name: label })).toBeVisible()
  expect(h.current()).toBe(controller)
  fireEvent.click(screen.getByRole('link', { name: 'All places' }))
  await screen.findByRole('heading', { name: 'MAP' })
  expect(h.current()).toBe(controller)
  fireEvent.click(screen.getByRole('button', { name: 'Back to previous panel' }))
  await screen.findByRole('navigation', { name: 'World tools' })
  expect(h.router.state.location.pathname).toBe('/world')
  expect(h.current()).toBe(controller); expect(document.querySelector('[data-world-host]')).toBe(host)
  expect(h.stats.mounts).toBe(mounts)
  expect(h.stats).toMatchObject({ games: 1, connections: 1, peakGames: 1, peakConnections: 1 })
  h.unmount(); expect(h.stats).toMatchObject({ games: 0, connections: 0, disposals: mounts })
  expect(controller!.read().list.data).toBeNull()
})
it('opens direct detail, and uses the current Map parent without a foreign history entry', async () => {
  const place = places()[1]!, h = await setup(`/world/places/${place.place_id}`)
  expect(await screen.findByRole('heading', { name: place.label })).toBeVisible()
  expect(screen.getByRole('link', { name: 'Open source reference (new tab)' })).toHaveAttribute('rel', 'noopener noreferrer')
  fireEvent.click(screen.getByRole('button', { name: 'Back to previous panel' }))
  await screen.findByRole('heading', { name: 'Map', exact: true, level: 1 })
  expect(h.router.state.location.pathname).toBe('/world/map'); expect(h.stats.mounts).toBe(1)
})
it('clears the visible owner on release withdrawal and permits a new explicit qualified binding', async () => {
  const h = await setup(); await screen.findByRole('link', { name: `View ${places()[0]!.label}` })
  const previous = h.current(), binding = h.source.read()!
  act(() => { h.publish(null) })
  expect(screen.getByText(/Places are unavailable until/)).toBeVisible()
  expect(previous!.read().list.data).toBeNull()
  act(() => { h.publish({ ...binding, signal: new AbortController().signal }) })
  await screen.findByRole('link', { name: `View ${places()[0]!.label}` })
  expect(h.current()).not.toBe(previous); expect(h.stats.mounts).toBe(1)
})
it('clears the panel immediately on same-identity policy withdrawal without restamping the old binding', async () => {
  const h = await setup(); await screen.findByRole('link', { name: `View ${places()[0]!.label}` })
  act(() => { h.auth.privacy.changePolicy(h.auth.privacy.capture(), 'block') })
  expect(screen.queryByRole('link', { name: `View ${places()[0]!.label}` })).not.toBeInTheDocument()
  expect(h.current()).toBeNull()
  expect(await screen.findByText(/Places are unavailable until/)).toBeVisible()
})
it('retains authoritative removed-target and mismatch latches across route navigation and catalog retry', async () => {
  const h = await setup(), place = places()[0]!
  await screen.findByRole('link', { name: `View ${place.label}` })
  const controller = h.current()!
  h.setMode('removed')
  fireEvent.click(screen.getByRole('link', { name: `View ${place.label}` }))
  await screen.findByRole('heading', { name: 'This place is unavailable' })
  fireEvent.click(screen.getByRole('link', { name: 'All places' }))
  await screen.findByRole('heading', { name: 'MAP' })
  expect(controller.canShow(place.place_id)).toBe(false)
  await act(async () => { await controller.refresh() })
  expect(controller.canShow(place.place_id)).toBe(false)
  h.setMode('ok')
  fireEvent.click(screen.getByRole('link', { name: `View ${place.label}` }))
  await screen.findByRole('heading', { name: place.label })
  expect(controller.canShow(place.place_id)).toBe(true)
  h.setMode('mismatch')
  await act(async () => { await controller.refresh() })
  await screen.findByRole('heading', { name: 'Your map and places do not match' })
  h.setMode('ok')
  await act(async () => { await controller.refresh() })
  expect(controller.read().mapMismatch).toBe(true)
  fireEvent.click(screen.getByRole('link', { name: 'All places' }))
  await waitFor(() => expect(controller.read().selectedId).toBeNull())
  expect(h.current()).toBe(controller); expect(controller.cameraAvailable()).toBe(false)
  expect(screen.getByRole('button', { name: 'Reload event' })).toBeVisible()
})
