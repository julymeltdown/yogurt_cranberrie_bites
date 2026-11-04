// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, expect, it } from 'vitest'
import { ParadeFooter } from '../../shell-chrome/parade-footer'

afterEach(cleanup)

it('keeps the ornament out of the accessibility tree and the action tab order', () => {
  const { container } = render(<section aria-label="Party actions"><button>Create party</button><ParadeFooter /></section>)
  expect(screen.getAllByRole('button')).toHaveLength(1)
  expect(screen.queryByRole('img')).toBeNull()
  const footer = container.querySelector('.mcr-chrome-parade-footer')!
  expect(footer).toHaveAttribute('aria-hidden', 'true')
  expect(footer.querySelector('svg')).toHaveAttribute('focusable', 'false')
  expect(footer.querySelector('a, button, input, [tabindex], [role]')).toBeNull()
  screen.getByRole('button').focus()
  expect(screen.getByRole('button')).toHaveFocus()
})

it('can share a page with other footers without document-global SVG references or external imagery', () => {
  const { container } = render(<><ParadeFooter /><ParadeFooter /></>)
  expect(container.querySelectorAll('svg')).toHaveLength(2)
  expect(container.querySelector('use, [id], image, foreignObject, [href], [style]')).toBeNull()
})
