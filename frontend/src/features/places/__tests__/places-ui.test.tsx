// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { afterEach, describe, expect, it } from 'vitest'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { createHttpClient, createMemorySession } from '../../../shared/api/http'
import { createPlacesController, type PlacesController } from '../model/controller'
import { PlaceDetail, PlacesOverview } from '../ui/Places'
import { WorldHelp } from '../../world-help/WorldHelp'
import { catalog, deferred, eventId, places, release, userId } from './fixtures/data'
const owned: PlacesController[] = []
afterEach(() => { cleanup(); owned.splice(0).forEach(c => c.dispose()) })
async function setup(mode = 'ok') {
  let outcome = mode
  const session = createMemorySession(); session.replace(0, { userId, eventId, accessToken: 'fixture-token' })
  let map = await release()
  const pending = deferred<Response>()
  const http = createHttpClient({ session, fetch: async input => {
    if (outcome === 'loading') return pending.promise
    if (outcome === 'error') return Response.json({ code: 'fixture', message: 'server secret', details: { reason: 'DEPENDENCY_UNAVAILABLE' } }, { status: 503 })
    const id = new URL((input as Request).url).pathname.split('/').at(-1)
    if (id !== 'places' && outcome === 'removed') return Response.json({ code: 'fixture', message: 'private', details: { reason: 'RESOURCE_UNAVAILABLE' } }, { status: 404 })
    return Response.json(id === 'places' ? catalog(outcome === 'empty' ? [] : places()) : { event_id: eventId, revision: 7, place: places().find(p => p.place_id === id) })
  } })
  if (mode === 'mismatch') map = { ...map, expected: { ...map.expected, revision: 3 } }
  const controller = createPlacesController({ session, http, map: { read: () => map, subscribe: () => () => {} } }); owned.push(controller)
  return { controller, setMode(value: string) { outcome = value }, pending }
}
describe('accessible overview and place sheet', () => {
  it('provides a DOM place list, accurate legend, search, and no fabricated physical or camera action', async () => {
    const { controller } = await setup()
    render(<MemoryRouter><PlacesOverview controller={controller} /></MemoryRouter>)
    await screen.findByRole('link', { name: 'View Synthetic virtual stage' })
    expect(screen.getByText('Virtual concept — no physical destination')).toBeInTheDocument()
    expect(screen.getByText('Physical place — verification pending')).toBeInTheDocument()
    expect(screen.getByText('Closed — do not use as a meeting destination')).toBeInTheDocument()
    expect(screen.getByRole('img', { name: /Virtual map overview/ })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Show Synthetic closed landmark on virtual map' })).toBeDisabled()
    expect(screen.queryByRole('button', { name: /arrived|meet here|check in/i })).not.toBeInTheDocument()
    fireEvent.change(screen.getByRole('searchbox', { name: 'Find a place' }), { target: { value: 'unverified' } })
    expect(screen.getByText('1 place shown')).toBeInTheDocument(); expect(screen.queryByRole('link', { name: 'View Synthetic virtual stage' })).not.toBeInTheDocument()
    fireEvent.change(screen.getByRole('searchbox'), { target: { value: 'missing' } }); expect(screen.getByText('No places match. Try a different name.')).toBeInTheDocument()
  })
  it.each(['loading', 'empty', 'error', 'mismatch'])('shows an honest %s state and appropriate recovery', async mode => {
    const { controller } = await setup(mode)
    render(<MemoryRouter><PlacesOverview controller={controller} /></MemoryRouter>)
    if (mode === 'loading') expect(screen.getByText('Loading places…')).toHaveAttribute('role', 'status')
    if (mode === 'empty') expect(await screen.findByText('No places are published for this event yet.')).toBeInTheDocument()
    if (mode === 'error') expect(await screen.findByRole('heading', { name: 'Places could not be loaded' })).toBeInTheDocument()
    if (mode === 'mismatch') {
      expect(await screen.findByRole('heading', { name: 'Your map and places do not match' })).toBeInTheDocument()
      expect(screen.getByRole('button', { name: 'Reload event' })).toBeInTheDocument()
      expect(screen.queryByRole('button', { name: 'Reload places' })).not.toBeInTheDocument()
      expect(screen.queryByRole('img')).not.toBeInTheDocument()
      expect(screen.getByRole('button', { name: 'Show Synthetic virtual stage on virtual map' })).toBeDisabled()
    }
    expect(screen.queryByText('server secret')).not.toBeInTheDocument()
  })
  it('retries only on explicit action and recovers from an unavailable service', async () => {
    const fixture = await setup('error')
    render(<MemoryRouter><PlacesOverview controller={fixture.controller} /></MemoryRouter>)
    await screen.findByRole('button', { name: 'Reload places' }); fixture.setMode('ok')
    fireEvent.click(screen.getByRole('button', { name: 'Reload places' }))
    expect(await screen.findByRole('link', { name: 'View Synthetic virtual stage' })).toBeInTheDocument()
  })
  it.each([0, 1, 2, 3, 4])('renders place %s provenance/status safely and never substitutes photos', async index => {
    const { controller } = await setup(), place = places()[index]!
    render(<MemoryRouter><PlaceDetail controller={controller} placeId={place.place_id} /></MemoryRouter>)
    expect(await screen.findByRole('heading', { name: place.label })).toBeInTheDocument()
    expect(screen.getByRole('heading', { name: 'Source and review' })).toBeInTheDocument()
    expect(screen.getByText('No authorized place photo is available in this place information.')).toBeInTheDocument()
    expect(screen.queryByRole('img')).not.toBeInTheDocument()
    const link = screen.queryByRole('link', { name: 'Open source reference (new tab)' })
    if ([1, 2, 3].includes(index)) { expect(link).toHaveAttribute('rel', 'noopener noreferrer'); expect(link).toHaveAttribute('referrerpolicy', 'no-referrer') }
    else { expect(link).not.toBeInTheDocument(); expect(screen.getByText('Source link unavailable. No external destination is offered.')).toBeInTheDocument() }
    expect(screen.getByRole('button', { name: 'Show on virtual map' })).toBeDisabled()
  })
  it('uses removed/unavailable recovery rather than retaining old place detail', async () => {
    const { controller } = await setup('removed')
    render(<MemoryRouter><PlaceDetail controller={controller} placeId={places()[0]!.place_id} /></MemoryRouter>)
    expect(await screen.findByRole('heading', { name: 'This place is unavailable' })).toBeInTheDocument()
    expect(screen.queryByText(places()[0]!.label)).not.toBeInTheDocument()
  })
  it('evicts selected detail on disposal and never displays a late response', async () => {
    const { controller, pending } = await setup('loading')
    render(<MemoryRouter><PlaceDetail controller={controller} placeId={places()[0]!.place_id} /></MemoryRouter>)
    act(() => controller.dispose()); await act(async () => { pending.resolve(Response.json(catalog())) })
    await waitFor(() => expect(controller.read().list.data).toBeNull())
    expect(screen.queryByRole('heading', { name: places()[0]!.label })).not.toBeInTheDocument()
  })
})
describe('controls help', () => {
  it('matches current keyboard/touch/camera controls and separates explicit physical decisions', () => {
    render(<MemoryRouter><WorldHelp /></MemoryRouter>)
    expect(screen.getByRole('heading', { name: 'One world, at your pace' })).toBeInTheDocument()
    expect(screen.getByText(/arrow keys or W, A, S, D/)).toBeInTheDocument()
    expect(screen.getByText(/press E/)).toBeInTheDocument()
    expect(screen.getByText(/R also recenters/)).toBeInTheDocument()
    expect(screen.getByText(/Walking an avatar does not confirm real arrival/)).toBeInTheDocument()
    expect(screen.getByRole('link', { name: 'Browse places' })).toHaveAttribute('href', '/world/map')
  })
})
