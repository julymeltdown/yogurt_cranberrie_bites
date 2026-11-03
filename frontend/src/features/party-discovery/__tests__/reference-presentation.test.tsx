// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { PartyDiscovery, type PartyDiscoveryPresentation } from '../ui/PartyDiscovery'
import { activity, ids, party } from './fixtures/data'
import { harness } from './fixtures/harness'
const owned: ReturnType<typeof harness>[] = []
afterEach(() => { cleanup(); owned.splice(0).forEach(h => h.dispose()) })
function setup(presentation?: PartyDiscoveryPresentation, full = false) {
  const h = harness(r => Response.json(new URL(r.url).pathname.endsWith('/me/parties') ? activity()
    : { items: [party(full ? { member_count: 5 } : {})], next_cursor: null })); owned.push(h)
  render(<MemoryRouter initialEntries={['/meet']}><Routes>
    <Route path="/meet" element={<PartyDiscovery controller={h.controller} {...presentation ? { presentation } : {}} />} />
    <Route path="/meet/people" element={<h1>People route</h1>} />
    <Route path="/meet/parties/new" element={<h1>Create route</h1>} />
    <Route path="/meet/parties/:partyId" element={<h1>Party detail route</h1>} />
  </Routes></MemoryRouter>)
  return h
}
describe('FE-REF-MEET production presentation', () => {
  it('uses the reference navigation order, real PEOPLE route, no nested window, and unavailable creation by default', async () => {
    setup(); await screen.findByRole('link', { name: party().title })
    const nav = screen.getByRole('navigation', { name: 'Meet views' })
    expect([...nav.children].map(el => el.textContent)).toEqual(['Parties', 'People', 'MY PLAN'])
    expect(screen.getByRole('button', { name: 'Parties' }).getAttribute('aria-pressed')).toBe('true')
    expect(screen.getByRole('button', { name: 'Create party' }).hasAttribute('disabled')).toBe(true)
    expect(document.querySelector('.concert-window')).toBeNull()
    fireEvent.click(within(nav).getByRole('link', { name: 'People' })); expect(screen.getByRole('heading', { name: 'People route' })).toBeTruthy()
  })
  it('JOIN opens the actual detail route without sending an application or claiming membership', async () => {
    const h = setup(); const join = await screen.findByRole('link', { name: `View details for ${party().title}` })
    expect(join.textContent).toBe('Join'); expect(join.getAttribute('href')).toBe(`/meet/parties/${ids.party}`)
    fireEvent.click(join); expect(screen.getByRole('heading', { name: 'Party detail route' })).toBeTruthy()
    expect(h.requests.filter(r => r.method !== 'GET')).toHaveLength(0)
  })
  it('uses only supplied current-data projections and removes imagery on privacy invalidation', async () => {
    const renderOwnerPortrait = vi.fn(() => <span role="img" aria-label="Caller-bound reviewed portrait" />)
    const placeLabel = vi.fn(() => 'Reviewed plaza')
    const h = setup({ renderOwnerPortrait, placeLabel, canCreate: true })
    await screen.findByRole('img', { name: 'Caller-bound reviewed portrait' })
    expect(renderOwnerPortrait).toHaveBeenCalledWith(party().owner); expect(placeLabel).toHaveBeenCalledWith(ids.place)
    expect(screen.getByText(/Reviewed plaza ·/)).toBeTruthy()
    expect(screen.getByRole('link', { name: 'Create party' }).getAttribute('href')).toBe('/meet/parties/new')
    expect(screen.getByText('Member portraits unavailable')).toBeTruthy()
    h.auth.privacy.changePolicy(h.auth.privacy.capture(), 'block')
    await waitFor(() => expect(screen.queryByRole('img', { name: 'Caller-bound reviewed portrait' })).toBeNull())
    expect(screen.queryByRole('link', { name: 'Create party' })).toBeNull()
    expect(screen.getByRole('button', { name: 'Create party' }).hasAttribute('disabled')).toBe(true)
  })
  it('keeps a full party readable without a JOIN promise and never resolves raw fixture avatar IDs', async () => {
    setup(undefined, true); const detail = await screen.findByRole('link', { name: `View details for ${party().title}` })
    expect(detail.textContent).toBe('Full'); expect(detail.getAttribute('href')).toBe(`/meet/parties/${ids.party}`)
    expect(screen.getByRole('img', { name: 'Host portrait unavailable' })).toBeTruthy()
    expect(document.querySelector('img,canvas')).toBeNull()
    expect(document.querySelector('.meet-party__capacity')?.textContent).toBe('5/5 members')
    expect(document.body.textContent).not.toContain('fixture_body')
  })
  it('only enables the creation route for an explicitly supplied current capability', async () => {
    setup({ canCreate: true }); fireEvent.click(await screen.findByRole('link', { name: 'Create party' }))
    expect(screen.getByRole('heading', { name: 'Create route' })).toBeTruthy()
  })
})
