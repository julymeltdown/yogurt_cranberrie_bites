// @vitest-environment jsdom
import { afterEach, expect, it } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { GiftDiscovery } from '../../ui/Gifts'
import { fixture, giveaway } from '../fixtures/data'
const owners: ReturnType<typeof fixture>[] = []
afterEach(() => { cleanup(); owners.splice(0).forEach(h => h.dispose()) })
it('keeps real stock, original reservation links and keyboard-controlled collections after row restyling', async () => {
  const h = fixture(); owners.push(h)
  render(<MemoryRouter><GiftDiscovery controller={h.controller} /></MemoryRouter>)
  const g = giveaway()
  const reserve = await screen.findByRole('link', { name: `Review reservation for ${g.title}` })
  expect(reserve.getAttribute('href')).toBe(`/gifts/${g.giveaway_id}/reservation`)
  expect(screen.getByText(`${g.available} left`)).toBeTruthy()
  fireEvent.keyDown(screen.getByRole('tab', { name: 'Browse' }), { key: 'ArrowRight' })
  expect(screen.getByRole('tab', { name: 'My giving' }).getAttribute('aria-selected')).toBe('true')
  expect(await screen.findByText('Only you can see this draft.')).toBeTruthy()
  expect(h.requests.every(r => r.method === 'GET')).toBe(true)
})
