// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { afterEach, expect, it } from 'vitest'
import { cleanup, render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { MapServiceLinks } from '../../../../features/places/ui/reference-scene/map-service-links'
import { map4Parade, map4ThemeHashes } from '../../map-presentation-v1/identity'
import { presentationHash, presentationVersion } from '../../../../world/reference-scene/map4-service-v1/annotations'

afterEach(cleanup)
it.each(['REVENGE', 'PARADE', 'DANGER'] as const)('requires the separately published %s marker for four accessible virtual routes', era => {
  const identity = { ...map4Parade, themeId: era, themeHash: map4ThemeHashes[era], presentationHash, presentationVersion }
  const view = render(<MemoryRouter><MapServiceLinks identity={{ ...identity, presentationHash: '' }} width={360} height={608} /></MemoryRouter>)
  expect(screen.queryAllByRole('link')).toHaveLength(0)
  view.rerender(<MemoryRouter><MapServiceLinks identity={identity} width={360} height={608} /></MemoryRouter>)
  const links = screen.getAllByRole('link'); expect(links).toHaveLength(4)
  expect(links.map(link => link.getAttribute('href')).sort()).toEqual(['/feed', '/gifts', '/meet', '/wall'])
  for (const link of links) {
    expect(link).toHaveAccessibleName(/Open .* — virtual feature/)
    expect(parseFloat(link.style.height)).toBeGreaterThanOrEqual(44)
    expect(parseFloat(link.style.width)).toBeGreaterThanOrEqual(44)
  }
  expect(screen.queryByRole('link', { name: /stage|plaza|food|merch/i })).toBeNull()
})
