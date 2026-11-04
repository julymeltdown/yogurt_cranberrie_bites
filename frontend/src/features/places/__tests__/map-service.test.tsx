// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { afterEach, expect, it } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { MapServiceLinks } from '../ui/reference-scene/map-service-links'
import { spatialV5SceneIdentity } from '../../../world/reference-scene/spatial-v5/model'

const identity = { ...spatialV5SceneIdentity, mapRevision:7005, themeId:'PARADE' }
afterEach(cleanup)
it.each([['wall','/wall'],['feed','/feed'],['meet','/meet'],['gifts','/gifts']] as const)('opens the fixed %s feature without a place ID or mutation', (intent, path) => {
  render(<MemoryRouter initialEntries={['/world/map']}><Routes>
    <Route path="/world/map" element={<MapServiceLinks identity={identity} width={360} height={608} />} />
    <Route path={path} element={<h1>{intent} destination</h1>} />
  </Routes></MemoryRouter>)
  const links = screen.getAllByRole('link')
  expect(links).toHaveLength(4)
  expect(links.every(a => !a.getAttribute('href')!.includes('/places/'))).toBe(true)
  const target = screen.getByRole('link', { name:`Open ${intent} — virtual feature` })
  expect(target).toHaveAttribute('href', path)
  expect(Number.parseFloat(target.style.height)).toBeGreaterThanOrEqual(44)
  expect(Number.parseFloat(target.style.width)).toBeGreaterThanOrEqual(44)
  target.focus(); expect(target).toHaveFocus()
  fireEvent.click(target)
  expect(screen.getByRole('heading', { name:`${intent} destination` })).toBeVisible()
})
it('does not invent food, retail, stage or plaza destinations and withdraws all links for a foreign overview', () => {
  const { rerender } = render(<MemoryRouter><MapServiceLinks identity={identity} width={360} height={608} /></MemoryRouter>)
  expect(screen.queryByRole('link', { name:/food|merch|stage|plaza|physical/i })).not.toBeInTheDocument()
  rerender(<MemoryRouter><MapServiceLinks identity={{ ...identity, geometryHash:'sha256:FOREIGN' }} width={360} height={608} /></MemoryRouter>)
  expect(screen.queryByRole('link')).not.toBeInTheDocument()
})
