import { webcrypto } from 'node:crypto'
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { Link, MemoryRouter, Outlet, Route, Routes } from 'react-router-dom'
import { createVenueRootBinding, createVenueRootInput, VenueRootFeatures } from '../../src/app/venue-root'
import { createMeetingLeafOwner, meetingConsumer, VenueCommunityOutlet } from '../../src/app/venue-community-entry'
import { useMeetingLeafOwner } from '../../src/app/venue-community-entry/provider'
import { ParadiseCityMapPanel } from '../../src/app/venue-entry/ParadiseCityMapPanel'
import { createVenueConsumer } from '../../src/app/venue-entry/consumer'
import type { MeetingConsumerPort } from '../../src/features/paradise-city-map/ports'
import { fixture } from './fixture'

beforeAll(() => {
  vi.stubGlobal('crypto', webcrypto)
  vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} })
  HTMLElement.prototype.scrollTo = () => {}
})
afterEach(() => { cleanup() })
const decode = async () => { const image = new Image(); image.src = 'blob:controlled-verified-bytes'; return {
  source: image, naturalWidth: 1024, naturalHeight: 1280, dispose() { image.src = '' },
} }
const owned: (() => void)[] = []
afterEach(() => { for (const stop of owned.splice(0).reverse()) stop() })
async function setup() { const f = await fixture(decode); owned.push(f.dispose); return f }

describe('actual venue root input and lifetime bridge', () => {
  it('projects the registered 22 ART IDs once and snapshots explicit real-provider associations', async () => {
    const f = await setup(), original = f.input.venue
    expect(original.landmarks).toHaveLength(22)
    f.borrowed.meetingPlaces.clear()
    expect(original.landmarks.find(p => p.id === f.gate.id)?.meetingContext?.landmarkId).toBe('reviewed-place-fixture')
    expect(original.image.src).toBe('blob:controlled-verified-bytes')
    expect(f.counts()).toMatchObject({ requests: 0, keys: 0, phoneStarts: 0 })
  })
  it('rejects a different asset registration instead of constructing another loader or map', async () => {
    const f = await setup()
    expect(() => createVenueRootInput({ ...f.borrowed, siteplan: { ...f.siteplan, producer: { kind: 'SYNTHETIC_TEST', sourceRevision: 'other' } } })).toThrow()
  })
  it.each(['userId', 'eventId', 'generation', 'policyGeneration', 'gate'] as const)('rejects a focus intent with stale %s', async field => {
    const f = await setup(), b = createVenueRootBinding(f.input), target = vi.fn(() => true); owned.push(b.dispose)
    b.bindFocus({ focus: target })
    const scope = { ...f.input.scope, [field]: field === 'generation' || field === 'policyGeneration' ? 999 : 'stale' }
    expect(b.focus.focus({ scope: scope as typeof f.input.scope, venue: f.input.venue.identity, landmarkId: f.gate.id })).toBe(false)
    b.onMapFocused(); expect(target).not.toHaveBeenCalled(); expect(f.counts().navigations).toBe(0)
  })
  it('requires the same venue and a known ART ID; successful focus authorizes one navigation', async () => {
    const f = await setup(), b = createVenueRootBinding(f.input), target = vi.fn(() => true); owned.push(b.dispose)
    b.bindFocus({ focus: target }); b.onMapFocused(); expect(f.counts().navigations).toBe(0)
    expect(b.focus.focus({ scope: f.input.scope, venue: { ...f.input.venue.identity, revision: 999 }, landmarkId: f.gate.id })).toBe(false)
    expect(b.focus.focus({ scope: f.input.scope, venue: f.input.venue.identity, landmarkId: 'unknown' as typeof f.gate.id })).toBe(false)
    expect(b.focus.focus({ scope: f.input.scope, venue: f.input.venue.identity, landmarkId: f.gate.id })).toBe(true)
    b.onMapFocused(); b.onMapFocused(); expect(f.counts().navigations).toBe(1)
  })
  it('scrubs both proxies before hostile unsubscribe and subscriber callbacks on privacy withdrawal', async () => {
    const f = await setup(), b = createVenueRootBinding(f.input); owned.push(b.dispose)
    const observed: unknown[] = [], target = vi.fn(() => true)
    b.bindFocus({ focus: target })
    b.bindConsumer({ read: () => ({ active: true, locked: false, content: 'PRIVATE' }), subscribe: () => () => {
      observed.push(b.meeting.read()); observed.push(b.focus.focus({ scope: f.input.scope, venue: f.input.venue.identity, landmarkId: f.gate.id }))
    }, open: async () => ({ kind: 'opened' }), close: () => true })
    b.subscribe(() => observed.push(b.meeting.read()))
    f.revoke()
    expect(observed).toEqual([null, false, null]); expect(b.read()).toBe(false)
    expect(target).not.toHaveBeenCalled(); expect(f.counts().phoneDisposes).toBe(0)
  })
  it('fences a late meeting open after abort and cannot resurrect an old delegate', async () => {
    const f = await setup(), b = createVenueRootBinding(f.input); owned.push(b.dispose)
    let finish!: (result: { kind: 'opened' }) => void
    const port: MeetingConsumerPort = { read: () => null, subscribe: () => () => {}, close: () => true,
      open: () => new Promise(resolve => { finish = resolve }) }
    b.bindConsumer(port); const pending = b.meeting.open(f.intent()); f.lifetime.abort(); finish({ kind: 'opened' })
    expect(await pending).toEqual({ kind: 'stale' }); b.bindConsumer(port); expect(b.meeting.read()).toBeNull()
  })
  it('keeps the image/local map input when community clock is unavailable and creates no meeting authority', async () => {
    const f = await setup(), b = createVenueRootBinding(createVenueRootInput({ ...f.borrowed, community: null })); owned.push(b.dispose)
    expect(b.read()).toBe(true); expect(b.release).toBeNull(); expect(b.mapInput.venue).toHaveProperty('classification', 'ILLUSTRATED_UNCALIBRATED')
    expect(await b.meeting.open(f.intent())).toEqual({ kind: 'unavailable' })
  })
  it('rejects a substitute clock or shell signal even if its current sample looks the same', async () => {
    const f = await setup()
    for (const meeting of [{ ...f.meeting, clock: { ...f.clock } }, { ...f.meeting, signal: new AbortController().signal }]) {
      const b = createVenueRootBinding(createVenueRootInput({ ...f.borrowed, meeting })); owned.push(b.dispose); expect(b.release).toBeNull()
    }
  })
  it('borrows actual map and meeting owners without a Map→release→meeting rebind cycle', async () => {
    const f = await setup(), b = createVenueRootBinding(f.input), input = b.mapInput; owned.push(b.dispose)
    const map = createVenueConsumer(input), leaf = createMeetingLeafOwner({ auth: f.auth, release: b.release, newCommandKey: f.input.newCommandKey })
    owned.push(map.dispose, leaf.dispose); b.bindFocus({ focus: map.controller.focus }); b.bindConsumer(meetingConsumer(leaf))
    expect(await b.meeting.open(f.intent())).toEqual({ kind: 'opened' }) // MESSAGE: no place, invite, phone or position.
    expect(leaf.read().surface).toBe('conversation'); expect(leaf.read().context?.landmarkLabel).toBeNull()
    expect(await b.meeting.open(f.intent('INVITE'))).toEqual({ kind: 'opened' })
    expect(leaf.viewOnMap()).toBe(true); expect(f.counts().navigations).toBe(1)
    expect(map.controller.read().focusRequest?.id).toBe(f.gate.id); expect(b.mapInput).toBe(input)
    expect(await b.meeting.open(f.intent('INVITE'))).toEqual({ kind: 'opened' }); expect(f.counts().reads).toBe(2)
    expect(f.counts()).toMatchObject({ requests: 0, keys: 0, phoneStarts: 0 })
  })
})

it('retains actual providers across Outlet remounts, then clears the rendered private leaf on withdrawal', async () => {
  const f = await setup(); let leaf: ReturnType<typeof useMeetingLeafOwner> = null
  function Probe() { leaf = useMeetingLeafOwner(); return null }
  function Layout() { return <VenueRootFeatures borrowed={f.input}><Probe /><Link to="/map">Map</Link><Link to="/other">Other</Link><Outlet /></VenueRootFeatures> }
  render(<MemoryRouter initialEntries={['/map']}><Routes><Route element={<Layout />}>
    <Route path="/map" element={<ParadiseCityMapPanel />} /><Route path="/other" element={<VenueCommunityOutlet />} />
  </Route></Routes></MemoryRouter>)
  await screen.findByRole('tab', { name: 'FRIENDS' })
  fireEvent.click(screen.getByRole('tab', { name: 'FRIENDS' }))
  await act(async () => { await leaf!.open(f.intent()); leaf!.setDraft('Retained private draft') })
  const original = leaf
  fireEvent.click(screen.getByRole('link', { name: 'Other' }))
  expect(await screen.findByDisplayValue('Retained private draft')).toBeTruthy(); expect(leaf).toBe(original)
  fireEvent.click(screen.getByRole('link', { name: 'Map' }))
  await waitFor(() => expect(screen.getByRole('tab', { name: 'FRIENDS' }).getAttribute('aria-selected')).toBe('true'))
  expect(leaf).toBe(original); expect(f.counts().phoneStarts).toBe(0)
  act(() => f.revoke())
  expect(screen.queryByDisplayValue('Retained private draft')).toBeNull()
  expect(screen.getByText(/venue map is unavailable/)).toBeTruthy()
})

it('renders the approved illustration and local tabs without capability or clock authority', async () => {
  const f = await setup(), input = createVenueRootInput({ auth: f.auth, scope: f.input.scope,
    signal: f.input.signal, community: null, siteplan: f.siteplan, meeting: null, phone: f.input.phone!,
    newCommandKey: f.input.newCommandKey, navigateToMap: f.input.navigateToMap })
  render(<VenueRootFeatures borrowed={input}><ParadiseCityMapPanel /></VenueRootFeatures>)
  await screen.findByRole('tab', { name: 'NOTES' })
  fireEvent.click(screen.getByRole('tab', { name: 'NOTES' }))
  expect(screen.getByRole('tab', { name: 'NOTES' }).getAttribute('aria-selected')).toBe('true')
  expect(document.querySelector('img')?.getAttribute('src')).toBe(input.venue.image.src)
  expect(f.counts()).toMatchObject({ requests: 0, keys: 0, phoneStarts: 0 })
})
