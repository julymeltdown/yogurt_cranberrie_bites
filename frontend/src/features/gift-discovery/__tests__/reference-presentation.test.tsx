// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { afterEach, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { GiftDiscovery, GiftDetail, type GiftImageRequest } from '../ui/Gifts'
import { allCapabilities, fixture, giveaway, now, uid } from './fixtures/data'
const fixtures: ReturnType<typeof fixture>[] = []
afterEach(() => { cleanup(); fixtures.splice(0).forEach(f => f.dispose()) })
function mount(patch = {}, image = false) {
  const gift = giveaway(patch), f = fixture({ handler: request => Response.json(new URL(request.url).pathname.endsWith('/me/giveaways')
    ? { giving: { items: [], next_cursor: null }, receiving: { items: [], next_cursor: null } }
    : new URL(request.url).pathname.endsWith('/giveaways') ? { items: [gift], next_cursor: null } : gift) }); fixtures.push(f)
  const media = vi.fn((_request: GiftImageRequest) => <span>Authorized root media slot</span>)
  render(<MemoryRouter><GiftDiscovery controller={f.controller} {...(image ? { renderGiftImage: media } : {})} /></MemoryRouter>)
  return { f, gift, media }
}
it('uses three semantic compact collections and enters the existing server-authorized reservation journey without POST', async () => {
  const { f, gift } = mount()
  const entry = await screen.findByRole('link', { name: `Review reservation for ${gift.title}` })
  expect(entry).toHaveAttribute('href', `/gifts/${gift.giveaway_id}/reservation`)
  expect(screen.getAllByRole('tab').map(e => e.textContent)).toEqual(['Browse', 'My giving', 'My pickups'])
  expect(screen.getByRole('tab', { name: 'Browse' })).toHaveAttribute('aria-selected', 'true')
  expect(document.querySelector('.concert-window')).toBeNull()
  expect(screen.getByText('No photo')).toBeInTheDocument()
  expect(document.body.textContent).not.toContain('recipient_id')
  expect(f.requests.every(r => r.method === 'GET')).toBe(true)
})
it.each([{ state: 'PAUSED' }, { available: 0 }, { state: 'CANCELLED' }, { state: 'CLOSED' }, { pickup_ends_at: new Date(now - 1).toISOString(), pickup_starts_at: new Date(now - 3600000).toISOString() }])('disables the reservation entry for unavailable authoritative gift %j', async patch => {
  const { gift } = mount(patch)
  expect(await screen.findByRole('button', { name: `Reservations unavailable for ${gift.title}` })).toBeDisabled()
  expect(screen.queryByRole('link', { name: /Review reservation/ })).toBeNull()
  expect(screen.getByRole('link', { name: gift.title })).toHaveAttribute('href', `/gifts/${gift.giveaway_id}`)
})
it('checks actor, event phase and mutation capability before offering reservation navigation', async () => {
  const { f, gift } = mount()
  await screen.findByRole('link', { name: /Review reservation/ })
  f.publish({ scope: f.auth.privacy.capture(), capabilities: { ...allCapabilities(), mutations: { available: false, reason: 'EVENT_READ_ONLY' } }, eventPhase: 'READ_ONLY' })
  expect(await screen.findByRole('button', { name: `Reservations unavailable for ${gift.title}` })).toBeDisabled()
  expect(screen.queryByRole('link', { name: /Review reservation/ })).toBeNull()
})
it('loads scoped private activity once, keeps independent collection resources and does not refetch on tab switches', async () => {
  const { f } = mount()
  await screen.findByRole('link', { name: /Review reservation/ })
  fireEvent.click(screen.getByRole('tab', { name: 'My giving' })); await screen.findByText('Nothing shared on this page')
  fireEvent.click(screen.getByRole('tab', { name: 'My pickups' })); await screen.findByText('No pickups on this page')
  fireEvent.click(screen.getByRole('tab', { name: 'Browse' })); await screen.findByRole('link', { name: /Review reservation/ })
  fireEvent.click(screen.getByRole('tab', { name: 'My pickups' })); await screen.findByText('No pickups on this page')
  expect(f.requests.filter(r => new URL(r.url).pathname.endsWith('/me/giveaways'))).toHaveLength(1)
  expect(f.requests.filter(r => new URL(r.url).pathname.endsWith('/giveaways') && !r.url.includes('/me/'))).toHaveLength(1)
})
it('supplies the exact media identity/full privacy scope to the root port, performs no media reads and evicts at policy invalidation', async () => {
  const { f, gift, media } = mount({ image_media_id: uid(891) }, true)
  await screen.findByText('Authorized root media slot')
  expect(media).toHaveBeenCalledWith({ giveawayId: gift.giveaway_id, mediaId: uid(891), scope: f.auth.privacy.capture(), alt: gift.title })
  expect(f.requests).toHaveLength(1)
  f.auth.privacy.changePolicy(f.auth.privacy.capture(), 'block')
  await waitFor(() => expect(screen.queryByText('Authorized root media slot')).toBeNull())
  expect(screen.queryByRole('tab')).toBeNull()
})
it('keeps detail eligibility unconfirmed and links the real journey instead of pretending to reserve', async () => {
  const f = fixture(); fixtures.push(f)
  render(<MemoryRouter><GiftDetail controller={f.controller} giveawayId={giveaway().giveaway_id} /></MemoryRouter>)
  expect(await screen.findByRole('link', { name: 'Review reservation' })).toHaveAttribute('href', `/gifts/${giveaway().giveaway_id}/reservation`)
  expect(screen.getByText('Your current reservation eligibility has not been confirmed.')).toBeInTheDocument()
  expect(f.requests.every(r => r.method === 'GET')).toBe(true)
})
