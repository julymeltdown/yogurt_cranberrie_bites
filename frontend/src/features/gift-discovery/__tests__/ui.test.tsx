// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { afterEach, expect, it } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { GiftDetail, GiftDiscovery } from '../ui/Gifts'
import { MyGifts } from '../../my-gifts/MyGifts'
import { fixture, giveaway } from './fixtures/data'
const owned: ReturnType<typeof fixture>[] = []
const setup = () => { const f = fixture(); owned.push(f); return f }
afterEach(() => { cleanup(); owned.splice(0).forEach(f => f.dispose()) })
it('offers time/stock/state/place filters, list/detail navigation and own activity without a fake command', async () => {
  const f = setup()
  render(<MemoryRouter><GiftDiscovery controller={f.controller} places={[{ id: giveaway().public_place_id, label: 'Test reference' }]} /></MemoryRouter>)
  expect(await screen.findByRole('link', { name: 'Fan badges' })).toHaveAttribute('href', `/gifts/${giveaway().giveaway_id}`)
  expect(screen.getByRole('link', { name: 'My gifts →' })).toHaveAttribute('href', '/gifts/mine')
  fireEvent.click(screen.getByText('Filter giveaways'))
  fireEvent.change(screen.getByLabelText('Pickup starts from (KST)'), { target: { value: '2026-11-08T19:00' } })
  fireEvent.change(screen.getByLabelText('Pickup starts before (KST)'), { target: { value: '2026-11-07T19:00' } })
  fireEvent.click(screen.getByRole('button', { name: 'Apply filters' }))
  expect(await screen.findByRole('alert')).toHaveTextContent('end of the range')
  expect(f.requests).toHaveLength(1)
  expect(screen.queryByRole('img')).not.toBeInTheDocument()
})
it('shows authoritative stock/time/state and a disabled reservation action without recipients or synthetic art', async () => {
  const f = setup()
  render(<MemoryRouter><GiftDetail controller={f.controller} giveawayId={giveaway().giveaway_id} /></MemoryRouter>)
  expect(await screen.findByRole('heading', { name: 'Fan badges' })).toBeInTheDocument()
  expect(screen.getByText('1 remaining')).toBeInTheDocument()
  expect(screen.getByRole('button', { name: 'Reserve unavailable' })).toBeDisabled()
  expect(screen.getByRole('link', { name: 'View public place reference' })).toHaveAttribute('href', `/world/places/${giveaway().public_place_id}`)
  expect(screen.queryByRole('img')).not.toBeInTheDocument()
  expect(document.body.textContent).not.toContain('recipient_id')
})
it('separates private giving drafts from receiving and provides independent paging controls', async () => {
  const f = setup()
  render(<MemoryRouter><MyGifts controller={f.controller} /></MemoryRouter>)
  expect(await screen.findByText('Only you can see this draft.')).toBeInTheDocument()
  expect(screen.queryByRole('link', { name: 'Fan badges' })).not.toBeInTheDocument()
  expect(screen.getByRole('button', { name: 'GIVING' })).toHaveAttribute('aria-pressed', 'true')
  fireEvent.click(screen.getByRole('button', { name: 'RECEIVING' }))
  expect(await screen.findByText('Held')).toBeInTheDocument()
  expect(screen.getByRole('navigation', { name: 'Receiving pages' })).toBeInTheDocument()
  expect(screen.queryByText('Only you can see this draft.')).not.toBeInTheDocument()
  expect(screen.queryByRole('button', { name: /confirm|claim|cancel/i })).not.toBeInTheDocument()
})
