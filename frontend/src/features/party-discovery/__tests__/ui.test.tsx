// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { PartyDetail, PartyDiscovery } from '../ui/PartyDiscovery'
import { activity, application, ids, party } from './fixtures/data'
import { harness, ready } from './fixtures/harness'
const owned: ReturnType<typeof harness>[] = []
afterEach(() => { cleanup(); owned.splice(0).forEach(h => h.dispose()) })
function setup(handle?: Parameters<typeof harness>[0]) { const h = harness(handle); owned.push(h); return h }
describe('FE21 accessible feature views', () => {
  it('renders discovery and independent MY PLAN navigation with safe empty copy and no art', async () => {
    const h = setup(); render(<MemoryRouter><PartyDiscovery controller={h.controller} /></MemoryRouter>)
    await screen.findByRole('link', { name: party().title }); fireEvent.click(screen.getByRole('button', { name: 'MY PLAN' }))
    await screen.findByRole('heading', { name: 'Your applications' }); expect(screen.getByText('No applications on this page.')).toBeTruthy(); expect(document.querySelector('img,canvas')).toBeNull()
    fireEvent.click(screen.getByRole('button', { name: 'Parties' })); expect(screen.getByRole('heading', { name: 'Find your company' })).toBeTruthy()
  })
  it('renders detail, requirements, reviewed-place link and note; full/closed states disable submission', async () => {
    const h = setup(r => {
      const path = new URL(r.url).pathname
      return Response.json(path.endsWith('/me/parties') ? activity() : path.endsWith('/parties') ? { items: [], next_cursor: null } : party({ member_count: 5 }))
    }); await ready(h); render(<MemoryRouter><PartyDetail controller={h.controller} partyId={ids.party} /></MemoryRouter>)
    await screen.findByRole('heading', { name: party().title }); await waitFor(() => expect(screen.getByText('This party is full.')).toBeTruthy())
    expect(screen.getByRole('button', { name: 'Apply to party' }).hasAttribute('disabled')).toBe(true)
    expect(screen.getByRole('link', { name: 'Review place information' }).getAttribute('href')).toBe(`/world/places/${ids.place}`)
    expect(screen.getByText(/do not confirm real arrival/)).toBeTruthy()
  })
  it('clears mounted form draft and private note on same-identity privacy changes', async () => {
    const h = setup(); render(<MemoryRouter><PartyDiscovery controller={h.controller} /></MemoryRouter>); await screen.findByRole('link', { name: party().title })
    const input = screen.getByRole('textbox', { name: 'Ticket section' }) as HTMLInputElement; fireEvent.change(input, { target: { value: 'Private draft' } }); expect(input.value).toBe('Private draft')
    h.auth.privacy.changePolicy(h.auth.privacy.capture(), 'block')
    await waitFor(() => expect((screen.getByRole('textbox', { name: 'Ticket section' }) as HTMLInputElement).value).toBe(''))
    expect(screen.queryByRole('link', { name: party().title })).toBeNull()
  })
  it('renders only pending application cancellation and explains approved membership boundary', async () => {
    const h = setup(r => Response.json(new URL(r.url).pathname.endsWith('/me/parties') ? { ...activity(), applications: { items: [application(), application({ application_id: ids.next, state: 'APPROVED' })], next_cursor: null } } : { items: [], next_cursor: null }))
    render(<MemoryRouter><PartyDiscovery controller={h.controller} /></MemoryRouter>); fireEvent.click(screen.getByRole('button', { name: 'MY PLAN' }))
    await screen.findByRole('button', { name: 'Cancel application' }); expect(screen.getAllByRole('button', { name: 'Cancel application' })).toHaveLength(1); expect(screen.getByText('Approved membership changes are managed separately.')).toBeTruthy()
  })
})
