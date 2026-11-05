// @vitest-environment jsdom
import { cleanup, render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { afterEach, expect, it } from 'vitest'
import { PreWorldFrame } from '../../parade-ink/PreWorldFrame'

afterEach(cleanup)

it('keeps the landmark label bound to the current heading across a state change', () => {
  const view = render(<MemoryRouter><PreWorldFrame title="Checking event availability" actions={<button>Retry</button>}>
    <p>Waiting for the current decision.</p>
  </PreWorldFrame></MemoryRouter>)
  const landmark = screen.getByRole('main', { name: 'Checking event availability' })
  const heading = screen.getByRole('heading', { level: 1 })
  expect(landmark.getAttribute('aria-labelledby')).toBe(heading.id)
  view.rerender(<MemoryRouter><PreWorldFrame title="World entry is paused" actions={<button>Retry</button>}>
    <p>Waiting for the current decision.</p>
  </PreWorldFrame></MemoryRouter>)
  expect(screen.getByRole('main', { name: 'World entry is paused' })).toBe(landmark)
  expect(screen.getByRole('heading', { level: 1 }).id).toBe(heading.id)
})

it('assigns distinct labels to concurrent frames without duplicating content IDs', () => {
  render(<MemoryRouter>
    <PreWorldFrame title="Current state" actions={null}><p>Current copy</p></PreWorldFrame>
    <PreWorldFrame title="Next state" actions={null}><p>Next copy</p></PreWorldFrame>
  </MemoryRouter>)
  const headings = screen.getAllByRole('heading', { level: 1 })
  expect(headings[0]?.id).toBeTruthy()
  expect(headings[0]?.id).not.toBe(headings[1]?.id)
  for (const heading of headings) expect(document.getElementById(heading.id)).toBe(heading)
})

it('leaves supplied native focus and dismissal behavior with the caller', () => {
  render(<MemoryRouter><PreWorldFrame title="Current state" actions={<button disabled>Check again</button>}>
    <p>No private content has been opened.</p>
  </PreWorldFrame></MemoryRouter>)
  const button = screen.getByRole('button', { name: 'Check again' }) as HTMLButtonElement
  expect(button.disabled).toBe(true)
  button.focus()
  expect(document.activeElement).not.toBe(button)
  const brand = screen.getByRole('link', { name: 'MCR Concert World' })
  brand.focus()
  expect(document.activeElement).toBe(brand)
  expect(brand.getAttribute('href')).toBe('/')
  expect(screen.queryByRole('button', { name: /close|dismiss/i })).toBeNull()
})
