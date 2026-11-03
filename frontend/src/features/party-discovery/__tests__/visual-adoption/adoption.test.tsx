// @vitest-environment jsdom
import { afterEach, expect, it } from 'vitest'
import { cleanup, render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { PartyDiscovery } from '../../ui/PartyDiscovery'
import { harness } from '../fixtures/harness'
import { party } from '../fixtures/data'
let f: ReturnType<typeof harness> | undefined
afterEach(() => { cleanup(); f?.dispose() })
it('shows capacity and a detail route without promising membership or rendering fixture avatar metadata', async () => {
  f = harness(); render(<MemoryRouter><PartyDiscovery controller={f.controller} /></MemoryRouter>)
  const link = await screen.findByRole('link', { name: `View details for ${party().title}` })
  expect(link.getAttribute('href')).toBe(`/meet/parties/${party().party_id}`)
  expect(screen.getByText('2/5 members')).toBeTruthy(); expect(screen.getByText('3 spaces')).toBeTruthy()
  expect(screen.queryByRole('button', { name: /join|apply/i })).toBeNull()
  expect(document.querySelector('img,canvas')).toBeNull()
})
