// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { PanelHeader } from '../shell-chrome'
import { ConcertWindow } from '../../shared/ui/concert-window'
import { createMemoryRouter, MemoryRouter, RouterProvider } from 'react-router-dom'
import { PanelScreen } from '../WorldShell'
import { panelRoutes } from '../routes'
import { createAppRoutes } from '../router'
import { createFixturePorts } from '../__tests__/fixtures/ports'

afterEach(cleanup)

it.each(['/meet', '/gifts'])('retains every %s fallback destination and ID inside a native disclosure', path => {
  const route = panelRoutes.find(route => route.path === path)!
  const { container } = render(<MemoryRouter initialEntries={[path]}><PanelScreen route={route}><p>Feature content</p></PanelScreen></MemoryRouter>)
  const disclosure = container.querySelector('details')!
  expect(container.firstElementChild).toBe(disclosure)
  expect(disclosure).not.toHaveAttribute('open')
  expect(disclosure.querySelector('summary')).toHaveTextContent('More actions')
  for (const link of route.links!) {
    const retained = disclosure.querySelector(`#section-${link.path.replaceAll('/', '-')}`)!
    expect(retained).toHaveAttribute('href', link.path)
    expect(retained).toHaveTextContent(link.label)
  }
})

it('keeps distinct native size, content and Back actions through full and peek states', () => {
  const back = vi.fn(), toggleSize = vi.fn(), toggleContent = vi.fn()
  const props = { title: 'Meet', back, toggleSize, toggleContent }
  const { rerender } = render(<PanelHeader {...props} size="full" />)
  expect(screen.getByRole('heading', { name: 'Meet' })).toHaveAttribute('tabindex', '-1')
  expect(screen.getByRole('button', { name: 'Reduce panel' }).querySelector('img')).toHaveAttribute('src', '/reference-window-controls-v2/reduce.svg')
  expect(screen.getByRole('button', { name: 'Minimize panel' }).querySelector('img')).toHaveAttribute('src', '/reference-window-controls-v2/back.svg')
  expect(screen.getByRole('button', { name: 'Minimize panel' }).querySelector('img')).toHaveClass('mcr-reference-window-ink--collapse')
  fireEvent.click(screen.getByRole('button', { name: 'Reduce panel' }))
  fireEvent.click(screen.getByRole('button', { name: 'Minimize panel' }))
  fireEvent.click(screen.getByRole('button', { name: 'Back to previous panel' }))
  expect([toggleSize.mock.calls.length, toggleContent.mock.calls.length, back.mock.calls.length]).toEqual([1, 1, 1])
  rerender(<PanelHeader {...props} size="peek" />)
  expect(screen.getByRole('button', { name: 'Expand panel' })).toHaveAttribute('aria-expanded', 'false')
  expect(screen.getByRole('button', { name: 'Show panel content' })).toHaveAttribute('aria-expanded', 'false')
  expect(screen.getByRole('button', { name: 'Show panel content' })).toHaveAttribute('aria-controls', 'shell-panel-body')
  expect(screen.getByRole('button', { name: 'Show panel content' }).querySelector('img')).toHaveClass('mcr-reference-window-ink--restore')
  expect(screen.queryByRole('img')).toBeNull()
})

it('preserves the public window heading and the caller close action without inventing other actions', () => {
  const close = vi.fn()
  const { container } = render(<ConcertWindow title="Sign in" headingLevel={1} onClose={close} closeLabel="Return home"><label>Email<input type="email" /></label></ConcertWindow>)
  expect(screen.getByRole('heading', { name: 'Sign in', level: 1 })).toBeVisible()
  expect(screen.getAllByRole('button')).toHaveLength(1)
  fireEvent.click(screen.getByRole('button', { name: 'Return home' }))
  expect(close).toHaveBeenCalledOnce()
  expect(container.querySelector('.mcr-reference-window-stars')).toHaveAttribute('src', '/reference-window-controls-v2/stars.svg')
  expect(screen.queryByRole('img')).toBeNull()
})

it('places decoration after real profile content in the existing scroll owner and hides the entire flow in peek', async () => {
  vi.stubGlobal('matchMedia', () => ({ matches: true, addEventListener() {}, removeEventListener() {} }))
  const fixture = createFixturePorts(), router = createMemoryRouter(createAppRoutes(fixture.ports), { initialEntries: ['/me'] })
  try {
    render(<RouterProvider router={router} />)
    await screen.findByRole('heading', { name: 'My profile', level: 1 }, { timeout: 4000 })
    const body = document.getElementById('shell-panel-body')!
    const footer = body.querySelector('.mcr-chrome-parade-footer')!
    expect(footer.parentElement).toHaveClass('mcr-reference-panel-flow')
    expect(footer.previousElementSibling).toHaveClass('mcr-reference-panel-content')
    fireEvent.click(screen.getByRole('button', { name: 'Minimize panel' }))
    expect(body).not.toBeVisible()
    expect(footer).not.toBeVisible()
    fireEvent.click(screen.getByRole('button', { name: 'Show panel content' }))
    expect(footer).toBeVisible()
    expect(fixture.stats.mounts).toBe(1)
  } finally { cleanup(); router.dispose(); vi.unstubAllGlobals() }
})
