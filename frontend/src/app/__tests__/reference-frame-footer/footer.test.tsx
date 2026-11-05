// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, expect, it } from 'vitest'
import { ParadeFooter } from '../../shell-chrome/parade-footer'

afterEach(cleanup)

// Photo 2 supersedes the ornamental crowd footer. Real caller actions remain.
it('keeps real actions in the tab order without adding the superseded ornament', () => {
  const { container } = render(<section aria-label="Party actions"><button>Create party</button><ParadeFooter /></section>)
  expect(screen.getAllByRole('button')).toHaveLength(1)
  expect(screen.queryByRole('img')).toBeNull()
  expect(container.querySelector('.mcr-chrome-parade-footer')).toBeNull()
  expect(screen.getByRole('region', { name: 'Party actions' }).children).toHaveLength(1)
  screen.getByRole('button').focus()
  expect(screen.getByRole('button')).toHaveFocus()
})

it('repeated former footer slots add no assets, global SVG references or focus targets', () => {
  const { container } = render(<><ParadeFooter /><ParadeFooter /></>)
  expect(container).toBeEmptyDOMElement()
  expect(container.querySelector('img, svg, use, [id], image, foreignObject, [href], [style], [srcset], a, button, input, [tabindex], [role]')).toBeNull()
})
