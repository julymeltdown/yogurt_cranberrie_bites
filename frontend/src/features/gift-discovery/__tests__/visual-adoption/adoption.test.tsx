// @vitest-environment jsdom
import { afterEach, expect, it } from 'vitest'
import { cleanup, render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { GiftDiscovery } from '../../ui/Gifts'
import { fixture, giveaway } from '../fixtures/data'
let f: ReturnType<typeof fixture> | undefined
afterEach(() => { cleanup(); f?.dispose() })
it('keeps stock, pickup time, own-activity and detail navigation without an instant reservation command', async () => {
  f = fixture(); render(<MemoryRouter><GiftDiscovery controller={f.controller} /></MemoryRouter>)
  const link = await screen.findByRole('link', { name: `Review reservation for ${giveaway().title}` })
  expect(link.getAttribute('href')).toBe(`/gifts/${giveaway().giveaway_id}/reservation`)
  expect(screen.getByText('1 left')).toBeTruthy(); expect(document.querySelectorAll('time')).toHaveLength(2)
  expect(screen.getByRole('link', { name: 'My gifts →' }).getAttribute('href')).toBe('/gifts/mine')
  expect(screen.queryByRole('button', { name: /reserve/i })).toBeNull()
  expect(document.querySelector('img,canvas')).toBeNull()
})
