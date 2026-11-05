// @vitest-environment jsdom
import { afterEach, expect, it } from 'vitest'
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { MemoryRouter, useLocation } from 'react-router-dom'
import { PartyRally } from '../../ui/PartyManagement'
import { harness, ids, response } from '../harness'

const owned: ReturnType<typeof harness>[] = []
afterEach(() => { cleanup(); owned.splice(0).forEach(h => h.dispose()) })
function Location() { return <output aria-label="Current panel">{useLocation().pathname}</output> }
async function setup(beacon: string | null = ids.next, revision = 1) {
  const h = harness(request => new URL(request.url).pathname.endsWith('/rally') ? Response.json({ party_id: ids.party,
    event_id: ids.event, meeting_revision: revision, beacon_id: beacon, entries: [] }) : response(request))
  owned.push(h); await h.controller.load()
  expect(h.auth.read().admission).toBe('ADMITTED')
  expect(h.admissionRequests).toHaveLength(1)
  expect(h.admissionRequests[0]?.method).toBe('GET')
  expect(h.controller.read().party?.party_id).toBe(ids.party)
  expect(h.controller.read().phase).toBe('ready')
  await h.controller.loadRally()
  expect(h.requests.some(request => new URL(request.url).pathname.endsWith('/rally'))).toBe(true)
  if (revision === 1) expect(h.controller.read().rally?.meeting_revision).toBe(1)
  render(<MemoryRouter initialEntries={[`/meet/parties/${ids.party}/rally`]}><PartyRally controller={h.controller} /><Location /></MemoryRouter>)
  return h
}

it('links to the decoded current beacon without reporting arrival', async () => {
  const h = await setup(), link = screen.getByRole('link', { name: 'View current meeting guidance' })
  expect(link.getAttribute('href')).toBe(`/meet/beacons/${ids.next}`)
  expect((screen.getByRole('button', { name: 'HERE' }) as HTMLButtonElement).disabled).toBe(true)
  fireEvent.click(link)
  expect(screen.getByLabelText('Current panel').textContent).toBe(`/meet/beacons/${ids.next}`)
  expect(h.requests.every(request => request.method === 'GET')).toBe(true)
})

it('does not offer a beacon link for a rally without one', async () => {
  await setup(null)
  expect(screen.queryByRole('link', { name: 'View current meeting guidance' })).toBeNull()
})

it('does not offer a link from a rejected superseded meeting response', async () => {
  await setup(ids.next, 2)
  expect(screen.queryByRole('link', { name: 'View current meeting guidance' })).toBeNull()
})

it('withdraws the link immediately when current meeting authority is withdrawn', async () => {
  const h = await setup()
  expect(screen.getByRole('link', { name: 'View current meeting guidance' })).toBeTruthy()
  act(() => h.updateAuthority(null))
  expect(screen.queryByRole('link', { name: 'View current meeting guidance' })).toBeNull()
})

it('withdraws the mounted link on privacy revocation', async () => {
  const h = await setup()
  expect(screen.getByRole('link', { name: 'View current meeting guidance' })).toBeTruthy()
  act(() => h.auth.privacy.changePolicy(h.auth.privacy.capture(), 'block'))
  expect(screen.queryByRole('link', { name: 'View current meeting guidance' })).toBeNull()
})
