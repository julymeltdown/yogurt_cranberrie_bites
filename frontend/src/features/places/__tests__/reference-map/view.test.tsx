// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { createHttpClient, createMemorySession } from '../../../../shared/api/http'
import { createPlacesController } from '../../model/controller'
import { PlacesOverview } from '../../ui/Places'
import { catalog, eventId, release, userId } from '../fixtures/data'
import type { SceneryOverview } from '../../../../world/illustrated/overview'
beforeEach(() => {
  vi.stubGlobal('location', { origin: 'https://mcr.example.invalid' })
  vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} })
  if (!HTMLDialogElement.prototype.showModal) HTMLDialogElement.prototype.showModal = function() { this.open = true }
  if (!HTMLDialogElement.prototype.close) HTMLDialogElement.prototype.close = function() { this.open = false }
  vi.spyOn(HTMLDialogElement.prototype, 'showModal').mockImplementation(function(this: HTMLDialogElement) { this.open = true })
  vi.spyOn(HTMLDialogElement.prototype, 'close').mockImplementation(function(this: HTMLDialogElement) { this.open = false })
})
afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.unstubAllGlobals() })
async function setup() {
  const session = createMemorySession(); session.replace(0, { userId, eventId, accessToken: 'memory-test' })
  const map = await release(), g = map.manifest.geometry, listeners = new Set<() => void>()
  let snapshot: SceneryOverview | null = { width: g.width_cells * g.tile_width_px, height: g.height_cells * g.tile_height_px,
    mapId: map.manifest.map_id, mapRevision: map.manifest.map_revision, mapHash: map.manifest.content_hash, geometryHash: map.manifest.geometry_hash,
    themeId: 'REVENGE', themeRevision: 1, themeHash: 'sha256:TEST', imageUrl: 'blob:test-map-1' }
  const fetch = vi.fn(async () => Response.json(catalog())), camera = { setZoom: vi.fn(() => true), lookAt: vi.fn(), recenter: vi.fn(), read: () => ({ following: true, camera: null }) }
  const controller = createPlacesController({ session, http: createHttpClient({ session, fetch }), map: { read: () => map, subscribe: () => () => {} }, camera: { read: () => camera }, overview: { read: () => snapshot, subscribe(fn) { listeners.add(fn); return () => { listeners.delete(fn) } } } })
  render(<MemoryRouter><PlacesOverview controller={controller} /></MemoryRouter>)
  await waitFor(() => expect(document.querySelector('img[alt="REVENGE illustrated virtual map"]')).not.toBeNull())
  const img = document.querySelector('img[alt="REVENGE illustrated virtual map"]')!
  fireEvent.load(img)
  return { controller, fetch, camera, img, replace(value: SceneryOverview | null) { snapshot = value; listeners.forEach(f => f()) }, snapshot: () => snapshot }
}
it('uses authorized labels/icon plaques, no fabricated current-position pin, and local zoom without network/camera writes', async () => {
  const f = await setup()
  expect(screen.getAllByRole('link', { name: /^View / })).toHaveLength(5)
  expect(screen.queryByRole('img', { name: /^Selected virtual location/ })).not.toBeInTheDocument()
  fireEvent.focus(screen.getByRole('link', { name: 'View Synthetic virtual stage' }))
  expect(screen.getByRole('img', { name: 'Selected virtual location: Synthetic virtual stage' })).toBeInTheDocument()
  const plus = screen.getByRole('button', { name: 'Zoom in map view' })
  for (let i = 0; i < 12; i++) fireEvent.click(plus)
  expect(plus).toBeDisabled(); fireEvent.click(screen.getByRole('button', { name: 'Fit map' }))
  expect(screen.getByRole('button', { name: 'Zoom out map view' })).toBeDisabled()
  fireEvent.click(screen.getByRole('button', { name: 'Reset view' }))
  expect(screen.queryByRole('img', { name: /^Selected virtual location/ })).not.toBeInTheDocument()
  expect(f.fetch).toHaveBeenCalledTimes(1); expect(f.camera.setZoom).not.toHaveBeenCalled(); expect(f.camera.lookAt).not.toHaveBeenCalled(); f.controller.dispose()
})
it('marks closed places honestly and never shows them as a supported selected pin', async () => {
  const f = await setup(); const closed = screen.getByRole('link', { name: 'View Synthetic closed landmark — closed' })
  expect(closed).toHaveAttribute('data-status', 'closed'); fireEvent.focus(closed)
  expect(screen.queryByRole('img', { name: /^Selected virtual location/ })).not.toBeInTheDocument()
  fireEvent.click(screen.getByRole('button', { name: 'Legend' })); expect(screen.getByRole('dialog')).toBeVisible()
  expect(screen.getByText('Moving your avatar or camera never confirms physical arrival. No GPS, travel time or safe route is inferred.')).toBeInTheDocument()
  fireEvent.click(screen.getByRole('button', { name: 'Close map information' })); expect(screen.queryByRole('dialog')).not.toBeInTheDocument(); f.controller.dispose()
})
it('falls back after image failure/replacement and withdraws old plaques and pins', async () => {
  const f = await setup(); fireEvent.error(f.img)
  expect(screen.getByText('Illustration unavailable. Schematic and place list remain available.')).toBeInTheDocument()
  expect(screen.queryByRole('link', { name: /^View / })).not.toBeInTheDocument()
  const next = { ...f.snapshot()!, imageUrl: 'blob:test-map-2', themeId: 'PARADE' }
  f.replace(next); await waitFor(() => expect(document.querySelector('img[alt="PARADE illustrated virtual map"]')).not.toBeNull()); fireEvent.load(document.querySelector('img[alt="PARADE illustrated virtual map"]')!)
  expect(await screen.findByRole('link', { name: 'View Synthetic virtual stage' })).toBeInTheDocument()
  f.replace(null); await waitFor(() => expect(screen.queryByRole('img', { name: 'PARADE illustrated virtual map' })).not.toBeInTheDocument()); f.controller.dispose()
})
