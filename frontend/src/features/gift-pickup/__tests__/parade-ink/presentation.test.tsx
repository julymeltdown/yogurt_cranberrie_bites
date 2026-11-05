// @vitest-environment jsdom
import { afterEach, expect, it } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { GiftPickup } from '../../ui/GiftPickup'
import { harness, gift, reservation } from '../harness'
const owners: ReturnType<typeof harness>[] = []
afterEach(() => { cleanup(); owners.splice(0).forEach(h => h.dispose()) })
function setup(options: Parameters<typeof harness>[0] = {}) { const h = harness(options); owners.push(h); return h }
it('keeps an absent image callback compatible and submits exactly the existing single-item command', async () => {
  const h = setup({ reserveOnly: true, handler: r => Response.json(r.method === 'GET' ? gift() : reservation({ state: 'HELD' })) })
  render(<GiftPickup controller={h.controller} />)
  const confirm = await screen.findByRole('button', { name: 'Confirm reservation' })
  expect((screen.getByRole('button', { name: 'Increase quantity' }) as HTMLButtonElement).disabled).toBe(true)
  expect((screen.getByRole('button', { name: 'Decrease quantity' }) as HTMLButtonElement).disabled).toBe(true)
  expect(screen.getByLabelText('Quantity').textContent).toBe('1')
  fireEvent.click(confirm)
  await screen.findByRole('heading', { name: 'Reservation status' })
  const sent = h.requests.filter(r => r.method === 'POST')
  expect(sent).toHaveLength(1)
  expect(await sent[0]!.json()).not.toHaveProperty('quantity')
  expect(screen.queryByText('Collection receipt')).toBeNull()
})
it('does not offer a live confirmation after the server reports sold-out stock', async () => {
  const h = setup({ reserveOnly: true, handler: () => Response.json({ ...gift(), available: 0 }) })
  render(<GiftPickup controller={h.controller} />)
  const confirm = await screen.findByRole('button', { name: 'Confirm reservation' })
  expect((confirm as HTMLButtonElement).disabled).toBe(true)
  fireEvent.click(confirm)
  expect(h.requests.filter(r => r.method === 'POST')).toHaveLength(0)
})
it('uses only the optional parent-provided image renderer with current membership', async () => {
  const h = setup({ reserveOnly: true, handler: () => Response.json({ ...gift(), image_media_id: '00000000-0000-4000-8000-000000000088' }) })
  render(<GiftPickup controller={h.controller} renderImage={r => <span data-testid="borrowed-image">{r.giveawayId}:{r.mediaId}</span>} />)
  await screen.findByRole('button', { name: 'Confirm reservation' })
  const g = h.controller.read().gift!
  expect(screen.getByTestId('borrowed-image').textContent).toBe(`${g.giveaway_id}:${g.image_media_id}`)
  expect(h.requests.every(r => !r.url.includes('/media/'))).toBe(true)
})
