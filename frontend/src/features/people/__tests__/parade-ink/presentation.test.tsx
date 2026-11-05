// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, expect, it } from 'vitest'
import { MemoryRouter } from 'react-router-dom'
import { People } from '../../People'
import { harness, publicIntro } from '../fixtures/harness'

const owned: ReturnType<typeof harness>[] = []
afterEach(() => { cleanup(); owned.splice(0).forEach(h => h.dispose()) })
function setup() {
  const h = harness(); owned.push(h)
  render(<MemoryRouter><People controller={h.controller} /></MemoryRouter>)
  return h
}

it('provides real Meet and introduction destinations without inventing a MY PLAN action', async () => {
  const h = setup()
  await screen.findByRole('link', { name: publicIntro().author.display_name })
  expect(screen.getByRole('link', { name: 'Parties' }).getAttribute('href')).toBe('/meet')
  expect(screen.getByRole('link', { name: 'Your introduction' }).getAttribute('href')).toBe('/meet/intro/edit')
  expect(screen.queryByRole('button', { name: 'MY PLAN' })).toBeNull()
  expect(h.requests.every(request => request.method === 'GET')).toBe(true)
})

it('retains Unicode validation and the server-owned public projection when search is too long', async () => {
  const h = setup()
  await screen.findByRole('link', { name: publicIntro().author.display_name })
  const before = h.requests.length
  fireEvent.change(screen.getByRole('searchbox', { name: 'Find introductions' }), { target: { value: '🎸'.repeat(101) } })
  fireEvent.click(screen.getByRole('button', { name: 'Search' }))
  expect(screen.getByRole('alert').textContent).toContain('101 / 100 Unicode code points')
  expect(h.requests).toHaveLength(before)
})

it('clears public content and disables search when access is withdrawn and privacy is invalidated', async () => {
  const h = setup()
  await screen.findByRole('link', { name: publicIntro().author.display_name })
  act(() => { h.access({ available: false }); h.privacy.changePolicy(h.privacy.capture(), 'block') })
  await waitFor(() => expect(screen.queryByRole('link', { name: publicIntro().author.display_name })).toBeNull())
  expect(screen.getByRole('button', { name: 'Search' }).hasAttribute('disabled')).toBe(true)
  expect(screen.getByText('People are unavailable for this session.')).toBeTruthy()
})
