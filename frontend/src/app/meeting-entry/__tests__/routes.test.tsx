// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { StrictMode, type ReactNode } from 'react'
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { MemoryRouter, Route, Routes, useNavigate } from 'react-router-dom'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { MeetingBeaconPanel, MeetingCheckInPanel, MeetingEntryFeatures, useMeetingEntry } from '../routes'
import { entryFixture, beaconId } from './fixtures'
import { deferred } from '../../../features/meet-beacon/__tests__/fixtures/harness'

const owners: Awaited<ReturnType<typeof entryFixture>>[] = []
async function setup() { const f = await entryFixture(); owners.push(f); return f }
beforeEach(() => vi.stubGlobal('location', { origin: 'https://mcr.example.invalid' }))
afterEach(() => { cleanup(); owners.splice(0).forEach(f => f.dispose()); vi.unstubAllGlobals() })
function Wrap({ f, children }: { f: Awaited<ReturnType<typeof entryFixture>>; children: ReactNode }) {
  return <MemoryRouter><MeetingEntryFeatures auth={f.h.auth} release={f.release}>{children}</MeetingEntryFeatures></MemoryRouter>
}
function CountOwner() { const view = useMeetingEntry(); return <output>{view.phase}</output> }
it('provides safe unavailable panels when the shell has no qualified runtime', async () => {
  const f = await setup(), count = f.h.requests.length
  render(<MemoryRouter><MeetingEntryFeatures auth={f.h.auth} release={null}><MeetingCheckInPanel /><MeetingBeaconPanel beaconId={beaconId} /></MeetingEntryFeatures></MemoryRouter>)
  expect(screen.getAllByText(/Real-world meeting guidance is unavailable/)).toHaveLength(2)
  expect(screen.queryByRole('button', { name: 'Share check-in' })).not.toBeInTheDocument()
  expect(f.h.requests).toHaveLength(count)
})
it('renders a deep-linked beacon through route params using the current qualified feature owner', async () => {
  const f = await setup()
  render(<MemoryRouter initialEntries={[`/meet/beacons/${beaconId}`]}><MeetingEntryFeatures auth={f.h.auth} release={f.release}>
    <Routes><Route path="/meet/beacons/:beaconId" element={<MeetingBeaconPanel />} /></Routes>
  </MeetingEntryFeatures></MemoryRouter>)
  expect(await screen.findByRole('heading', { name: 'Current meeting' })).toBeVisible()
  expect(screen.getByText('SYNTHETIC red scarf')).toBeVisible()
  expect(screen.getByText('Beacon 1 · Place 3 · Map 1')).toBeVisible()
})
it('rejects a malformed deep link without dispatching a meeting request', async () => {
  const f = await setup(), count = f.h.requests.length
  render(<Wrap f={f}><MeetingBeaconPanel beaconId="not-a-beacon" /></Wrap>)
  expect(screen.getByRole('status')).toHaveTextContent('unavailable'); expect(f.h.requests).toHaveLength(count)
})
it('survives StrictMode setup/cleanup with one live feature subscription set and releases it on unmount', async () => {
  const f = await setup()
  const view = render(<StrictMode><Wrap f={f}><MeetingCheckInPanel /><CountOwner /></Wrap></StrictMode>)
  expect(await screen.findByRole('button', { name: 'Share check-in' })).toBeEnabled()
  expect(f.subscriptions()).toEqual({ clock: 1, authority: 1, places: 1, invalidations: 1 })
  expect(screen.getByRole('status')).toHaveTextContent('available')
  view.unmount(); expect(f.subscriptions()).toEqual({ clock: 0, authority: 0, places: 0, invalidations: 0 })
})
it('scrubs a linked beacon on clock loss and canonical identity change without creating new auth or world owners', async () => {
  const f = await setup()
  render(<Wrap f={f}><MeetingBeaconPanel beaconId={beaconId} /></Wrap>)
  expect(await screen.findByText('SYNTHETIC red scarf')).toBeVisible()
  act(() => f.setClock(false)); expect(screen.queryByText('SYNTHETIC red scarf')).not.toBeInTheDocument()
  expect(screen.getByRole('status')).toHaveTextContent('unavailable')
  act(() => f.setClock(true)); expect(await screen.findByText('SYNTHETIC red scarf')).toBeVisible()
  act(() => { f.h.auth.session.replace(f.scope.generation, null) })
  expect(screen.queryByText('SYNTHETIC red scarf')).not.toBeInTheDocument()
  expect(f.subscriptions()).toEqual({ clock: 0, authority: 0, places: 0, invalidations: 0 })
})
it('never displays the previous target while the next route read is pending', async () => {
  const f = await setup(), other = '00000000-0000-4000-8000-000000000777', response = deferred<Response>()
  const view = render(<Wrap f={f}><MeetingBeaconPanel beaconId={beaconId} /></Wrap>)
  expect(await screen.findByText('SYNTHETIC red scarf')).toBeVisible()
  f.h.setOverride(r => r.url.endsWith(`/beacons/${other}`) ? response.promise : null)
  view.rerender(<Wrap f={f}><MeetingBeaconPanel beaconId={other} /></Wrap>)
  expect(screen.queryByText('SYNTHETIC red scarf')).not.toBeInTheDocument()
  await act(async () => { response.resolve(Response.json({ ...f.h.beacon, beacon_id: other, meeting: { ...f.h.beacon.meeting, identification: 'SYNTHETIC new target' } })) })
  expect(await screen.findByText('SYNTHETIC new target')).toBeVisible()
})
it('keeps an unresolved command locked while navigating within the same shell provider', async () => {
  const f = await setup()
  function Panels() { const navigate = useNavigate(); return <><button onClick={() => navigate('/world/check-in')}>Open check-in fixture</button><button onClick={() => navigate(`/meet/beacons/${beaconId}`)}>Open beacon fixture</button>
    <Routes><Route path="/world/check-in" element={<MeetingCheckInPanel />} /><Route path="/meet/beacons/:beaconId" element={<MeetingBeaconPanel />} /></Routes></> }
  render(<MemoryRouter initialEntries={['/world/check-in']}><MeetingEntryFeatures auth={f.h.auth} release={f.release}><Panels /></MeetingEntryFeatures></MemoryRouter>)
  await screen.findByLabelText('Identification')
  fireEvent.change(screen.getByLabelText('Reviewed place'), { target: { value: f.h.items[1]!.place_id } })
  fireEvent.change(screen.getByLabelText('Who can see this?'), { target: { value: 'synthetic-party' } })
  fireEvent.change(screen.getByLabelText('How to find me'), { target: { value: 'SYNTHETIC marker' } })
  fireEvent.change(screen.getByLabelText('Identification'), { target: { value: 'SYNTHETIC scarf' } })
  fireEvent.click(screen.getByRole('checkbox')); f.h.setMode('lost')
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Share check-in' })) })
  expect(screen.getByRole('button', { name: 'Share check-in' })).toBeDisabled()
  fireEvent.click(screen.getByRole('button', { name: 'Open beacon fixture' }))
  await screen.findByRole('heading', { name: 'Current meeting' })
  fireEvent.click(screen.getByRole('button', { name: 'Open check-in fixture' }))
  expect(await screen.findByRole('button', { name: 'Share check-in' })).toBeDisabled()
  expect(f.h.requests.filter(r => r.method === 'POST')).toHaveLength(1)
})
