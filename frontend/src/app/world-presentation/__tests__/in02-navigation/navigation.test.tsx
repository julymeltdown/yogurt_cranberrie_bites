// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { MemoryRouter, useLocation } from 'react-router-dom'
import { WorldTools } from '../../WorldTools'

afterEach(cleanup)
function Location() { return <output aria-label="Current panel">{useLocation().pathname}</output> }

it('opens the existing check-in panel through a same-document ShellLink', () => {
  const changed = vi.fn()
  render(<MemoryRouter initialEntries={['/world']}><WorldTools open onOpenChange={changed} /><Location /></MemoryRouter>)
  const host = screen.getByRole('main'), link = screen.getByRole('link', { name: 'Check-in' })
  expect(link.getAttribute('href')).toBe('/world/check-in')
  expect(link.id).toBe('world-check-in')
  fireEvent.click(link)
  expect(screen.getByLabelText('Current panel').textContent).toBe('/world/check-in')
  expect(screen.getByRole('main')).toBe(host)
  expect(changed).not.toHaveBeenCalled()
})

it('keeps check-in within the existing collapsible navigation', () => {
  render(<MemoryRouter><WorldTools open={false} onOpenChange={vi.fn()} /></MemoryRouter>)
  expect(screen.queryByRole('link', { name: 'Check-in' })).toBeNull()
  expect(document.getElementById('world-check-in')?.closest('nav')?.hidden).toBe(true)
})
