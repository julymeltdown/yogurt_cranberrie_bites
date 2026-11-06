// @vitest-environment jsdom
import { afterEach, expect, it } from 'vitest'
import { cleanup, render, screen, within } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { WorldHelp } from '../WorldHelp'

afterEach(cleanup)
it('keeps the help article landmarks and its two world destinations reachable', () => {
  render(<MemoryRouter><WorldHelp /></MemoryRouter>)
  const article = screen.getByRole('article', { name: 'Controls and meeting safety' })
  expect(within(article).getAllByRole('heading', { level: 2 })).toHaveLength(1)
  expect(within(article).getAllByRole('heading', { level: 3 })).toHaveLength(4)
  const navigation = within(article).getByRole('navigation', { name: 'Explore after reading' })
  const links = within(navigation).getAllByRole('link')
  expect(links.map(link => link.getAttribute('href'))).toEqual(['/world/map', '/world'])
  expect(links.map(link => link.textContent)).toEqual(['Browse places', 'Return to world'])
})
