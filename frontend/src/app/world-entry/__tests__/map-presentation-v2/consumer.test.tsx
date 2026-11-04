// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import type { CompositionModel } from '../../../../world/reference-scene/map4-service-v2/model'
import { MapServiceLinks } from '../../../../features/places/ui/reference-scene/map-service-links'
import { IllustratedMap } from '../../../../features/places/ui/IllustratedMap'
import type { PlacesController, PlacesState } from '../../../../features/places/model/controller'
import { placeSchema } from '../../../../features/places/model/contract'
import { map4Parade } from '../../../../world/reference-scene/map4-service-v1/identity'
import { presentationHash, presentationVersion } from '../../../../world/reference-scene/map4-service-v1/annotations'
import { compositionFixture } from './fixture'
import { loadedMap4 } from '../map-presentation-v1/fixture'

const binding = vi.hoisted(() => ({ model: null as CompositionModel | null }))
vi.mock('../../../../world/reference-scene/map4-service-v2/current', () => ({ currentComposition: () => binding.model }))
beforeEach(() => { vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} }) })
afterEach(() => { cleanup(); binding.model = null; vi.unstubAllGlobals() })

async function setup() {
  const fixture = await compositionFixture(), loaded = await loadedMap4()
  binding.model = fixture.model
  const identity = { ...map4Parade, presentationVersion: fixture.document.version, presentationHash: fixture.registration.presentationHash }
  const state: PlacesState = { overview: { ...identity, imageUrl: 'blob:unit-composition' }, map: loaded.map,
    list: { phase: 'ready', reason: null, data: { event_id: '00000000-0000-4000-8000-000000000001', revision: 1, items: loaded.map.places.map(place => placeSchema.parse(place)) } },
    detail: { phase: 'idle', reason: null, data: null }, selectedId: null, mapMismatch: false, unavailablePlaceIds: [], notice: null }
  // IllustratedMap uses controller identity only; HTTP/camera ownership stays outside this DOM unit.
  const controller = {} as PlacesController
  return { ...fixture, loaded, identity, state, controller }
}

it('uses four v2 virtual routes once, with packed 44px targets and no canonical IDs', async () => {
  const f = await setup(), onOpen = vi.fn()
  render(<MemoryRouter><MapServiceLinks identity={f.identity} width={265.5} height={448.03} onOpen={onOpen} /></MemoryRouter>)
  const links = screen.getAllByRole('link')
  expect(links.map(link => link.getAttribute('href'))).toEqual(['/wall', '/feed', '/meet', '/gifts'])
  for (const link of links) {
    expect(Number.parseFloat(link.style.width)).toBeGreaterThanOrEqual(44)
    expect(Number.parseFloat(link.style.height)).toBeGreaterThanOrEqual(44)
    expect(Number.parseFloat(link.style.top) + Number.parseFloat(link.style.height) / 2).toBeLessThanOrEqual(448.03)
  }
  fireEvent.click(links[0]!)
  expect(onOpen).toHaveBeenCalledWith('wall')
  expect(screen.queryByRole('link', { name: /food|merch|stage|plaza|physical/i })).not.toBeInTheDocument()
})

it('preserves qualified v1 links while a v2 model is registered and withdraws forged v2 identities', async () => {
  const f = await setup(), v1 = { ...map4Parade, presentationHash, presentationVersion }
  const { rerender } = render(<MemoryRouter><MapServiceLinks identity={v1} width={360} height={608} /></MemoryRouter>)
  expect(screen.getAllByRole('link')).toHaveLength(4)
  rerender(<MemoryRouter><MapServiceLinks identity={{ ...f.identity, presentationHash: presentationHash }} width={360} height={608} /></MemoryRouter>)
  expect(screen.queryByRole('link')).not.toBeInTheDocument()
})

it('projects canonical markers, leaders and selected pins together without rewriting server coordinates', async () => {
  const f = await setup(), originalGeometry = structuredClone(f.loaded.map.geometry)
  render(<MemoryRouter><IllustratedMap controller={f.controller} state={f.state} fallback={null} legend={null} /></MemoryRouter>)
  fireEvent.load(screen.getByAltText('PARADE illustrated virtual map'))
  expect(screen.getAllByRole('link', { name: /^View / })).toHaveLength(4)
  expect(screen.getAllByRole('link', { name: /^Open / })).toHaveLength(4)
  const plane = document.querySelector<HTMLElement>('.places-map-plane')!
  for (const anchor of f.model.parsed.canonicalDisplayAnchors) {
    const interaction = f.loaded.map.geometry.interactions.find(i => i.interaction_id === anchor.interactionId)!
    const place = f.loaded.map.places.find(p => p.place_id === interaction.place_id)!
    const link = screen.getByRole('link', { name: `View ${place.label}` })
    expect(link).toHaveAttribute('href', `/world/places/${place.place_id}`)
    expect(link).toHaveAttribute('data-map-compact-marker', anchor.interactionId)
    fireEvent.focus(link)
    const pin = screen.getByRole('img', { name: `Selected virtual location: ${place.label}` })
    expect(Number.parseFloat(pin.style.left)).toBeCloseTo(anchor.x / 768 * 100)
    expect(Number.parseFloat(pin.style.top)).toBeCloseTo(anchor.y / 1296 * 100)
    const expected = f.model.displayPoint(f.identity, anchor.interactionId, Number.parseFloat(plane.style.width), Number.parseFloat(plane.style.height))!
    const leader = Array.from(document.querySelectorAll('line')).find(line => Math.abs(Number(line.getAttribute('x1')) - expected.x) < .001
      && Math.abs(Number(line.getAttribute('y1')) - expected.y) < .001)
    expect(leader).toBeDefined()
    expect(Number(leader!.getAttribute('x2'))).toBe(Number.parseFloat(link.style.left))
    expect(Number(leader!.getAttribute('y2'))).toBe(Number.parseFloat(link.style.top))
  }
  expect(f.loaded.map.geometry).toEqual(originalGeometry)
  expect(screen.getByText('No physical arrival is implied.').parentElement).toHaveStyle({ display: 'block' })
  fireEvent.click(screen.getByRole('button', { name: 'Reset view' }))
  expect(screen.queryByRole('img', { name: /^Selected virtual location:/ })).not.toBeInTheDocument()
})

it('retains closed and unavailable canonical status without a supported selected pin', async () => {
  const f = await setup(), items = structuredClone(f.state.list.data!.items)
  items[0]!.closure_state = 'CLOSED'
  const unavailable = items[1]!.place_id
  const state = { ...f.state, unavailablePlaceIds: [unavailable], list: { ...f.state.list, data: { ...f.state.list.data!, items } } }
  render(<MemoryRouter><IllustratedMap controller={f.controller} state={state} fallback={null} legend={null} /></MemoryRouter>)
  fireEvent.load(screen.getByAltText('PARADE illustrated virtual map'))
  for (const [place, status] of [[items[0]!, 'closed'], [items[1]!, 'unavailable']] as const) {
    const link = screen.getByRole('link', { name: `View ${place.label} — ${status}` })
    expect(link).toHaveAttribute('data-status', status)
    fireEvent.focus(link)
    expect(screen.queryByRole('img', { name: /^Selected virtual location:/ })).not.toBeInTheDocument()
  }
})
