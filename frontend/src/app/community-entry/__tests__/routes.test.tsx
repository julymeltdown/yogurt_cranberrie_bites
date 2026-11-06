// @vitest-environment jsdom
import { StrictMode } from 'react'
import { afterEach, expect, it } from 'vitest'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { MemoryRouter, Route, Routes, useNavigate } from 'react-router-dom'
import { CommunityEntryProvider, communityEntryPanels } from '../routes'
import { fixture, giftId, reservationId, ids } from './fixture'

const cleanups: (() => void)[] = []
afterEach(() => { cleanup(); cleanups.splice(0).forEach(fn => fn()) })
function Navigation() {
  const navigate = useNavigate()
  return <nav><button onClick={() => navigate('/meet/parties/new')}>Party form</button>
    <button onClick={() => navigate('/gifts/new')}>Giveaway form</button>
    <button onClick={() => navigate(`/gifts/${giftId}/reservation`)}>Reserve route</button>
    <button onClick={() => navigate(`/gifts/pickups/${reservationId}`)}>Pickup route</button>
    <button onClick={() => navigate('/gifts/pickups/invalid')}>Invalid route</button></nav>
}
function mount(path = '/meet/parties/new', owner = false) {
  const f = fixture({ owner }); cleanups.push(f.dispose)
  const rendered = render(<StrictMode><MemoryRouter initialEntries={[path]}><CommunityEntryProvider auth={f.auth} release={f.release}>
    <Navigation /><Routes>{Object.entries(communityEntryPanels).map(([p, element]) => <Route key={p} path={p} element={element} />)}</Routes>
  </CommunityEntryProvider></MemoryRouter></StrictMode>)
  return { f, rendered }
}
it('keeps a session draft across route unmounts under StrictMode', async () => {
  mount(); const title = await screen.findByLabelText('Party title'); fireEvent.change(title, { target: { value: 'Retained party draft' } })
  fireEvent.click(screen.getByRole('button', { name: 'Save session draft' })); fireEvent.click(screen.getByText('Giveaway form'))
  await screen.findByText('Start a giveaway'); fireEvent.click(screen.getByText('Party form'))
  await waitFor(() => expect((screen.getByLabelText('Party title') as HTMLInputElement).value).toBe('Retained party draft'))
})
it('does not replay unknown reservation across panel navigation, and exposes original status recovery', async () => {
  const { f } = mount(`/gifts/${giftId}/reservation`); await screen.findByRole('button', { name: 'Confirm reservation' }); f.loseResponse()
  fireEvent.click(screen.getByRole('button', { name: 'Confirm reservation' })); await screen.findByText(/Outcome unknown/)
  fireEvent.click(screen.getByText('Pickup route')); await screen.findByRole('heading', { name: 'Finish the original pickup command' })
  fireEvent.click(screen.getByRole('button', { name: 'Check original command' })); await screen.findByText(/still pending/)
  fireEvent.click(screen.getByText('Reserve route')); await screen.findByText(/still pending/)
  expect(f.requests.filter(r => r.method !== 'GET')).toHaveLength(1)
})
it('invalid parameter never displays the previously selected private pickup', async () => {
  const { f } = mount(`/gifts/pickups/${reservationId}`); await screen.findByText('Reservation status')
  fireEvent.click(screen.getByText('Invalid route')); await screen.findByText('Community action unavailable')
  expect(screen.queryByText('Reservation status')).toBeNull(); expect(f.requests.filter(r => r.method !== 'GET')).toHaveLength(0)
})
it('root lease withdrawal synchronously removes forms and does not create a replacement owner', async () => {
  const { f } = mount(); await screen.findByLabelText('Party title')
  act(() => f.lifetime.abort()); expect(screen.queryByLabelText('Party title')).toBeNull()
  expect(screen.getByText('Community action unavailable')).toBeTruthy(); expect(f.subscriptions().clock).toBe(0)
})
it('party manage composes owned editing and rally route loads current membership without physical fallback', async () => {
  const { f } = mount(`/meet/parties/${ids.party}/rally`); await screen.findByText(/Current authorized physical meeting details are unavailable/)
  await waitFor(() => expect(f.requests.some(r => r.url.includes('/members'))).toBe(true))
  expect((screen.getByRole('button', { name: /^HERE$/ }) as HTMLButtonElement).disabled).toBe(true)
})
it('owner reservation opens giveaway-bound claim controller inside the manage panel', async () => {
  const { f } = mount(`/gifts/${giftId}/manage`, true); await screen.findByRole('button', { name: 'Open pickup 1' })
  fireEvent.click(screen.getByRole('button', { name: 'Open pickup 1' })); await screen.findByLabelText('Recipient’s six-digit code')
  expect(f.requests.some(r => new URL(r.url).pathname.endsWith(`/reservations/${reservationId}`))).toBe(true)
  expect(f.requests.filter(r => r.method !== 'GET')).toHaveLength(0)
})
