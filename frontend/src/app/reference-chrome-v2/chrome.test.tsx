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
  // Existing content precedes its real More actions disclosure in natural order.
  expect(container.firstElementChild).toHaveTextContent('Feature content')
  expect(disclosure.previousElementSibling).toBe(container.firstElementChild)
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
  // Photo 2 keeps printed ink inside separate real buttons, now currentColor SVG.
  expect(screen.getByRole('button', { name: 'Reduce panel' }).querySelector('svg')).toHaveAttribute('aria-hidden', 'true')
  expect(screen.getByRole('button', { name: 'Reduce panel' }).querySelector('svg path')).toHaveAttribute('d', 'M5 12h14')
  expect(screen.getByRole('button', { name: 'Minimize panel' }).querySelector('svg')).toHaveAttribute('focusable', 'false')
  expect(screen.getByRole('button', { name: 'Minimize panel' }).querySelector('svg')).toHaveClass('mcr-reference-window-ink--collapse')
  expect(screen.getByRole('button', { name: 'Minimize panel' }).querySelector('svg path')).toHaveAttribute('fill', 'currentColor')
  fireEvent.click(screen.getByRole('button', { name: 'Reduce panel' }))
  fireEvent.click(screen.getByRole('button', { name: 'Minimize panel' }))
  fireEvent.click(screen.getByRole('button', { name: 'Back to previous panel' }))
  expect([toggleSize.mock.calls.length, toggleContent.mock.calls.length, back.mock.calls.length]).toEqual([1, 1, 1])
  rerender(<PanelHeader {...props} size="peek" />)
  expect(screen.getByRole('button', { name: 'Expand panel' })).toHaveAttribute('aria-expanded', 'false')
  expect(screen.getByRole('button', { name: 'Show panel content' })).toHaveAttribute('aria-expanded', 'false')
  expect(screen.getByRole('button', { name: 'Show panel content' })).toHaveAttribute('aria-controls', 'shell-panel-body')
  expect(screen.getByRole('button', { name: 'Show panel content' }).querySelector('svg')).toHaveClass('mcr-reference-window-ink--restore')
  expect(screen.queryByRole('img')).toBeNull()
})

it('preserves the public window heading and the caller close action without inventing other actions', () => {
  const close = vi.fn()
  const { container } = render(<ConcertWindow title="Sign in" headingLevel={1} onClose={close} closeLabel="Return home"><label>Email<input type="email" /></label></ConcertWindow>)
  expect(screen.getByRole('heading', { name: 'Sign in', level: 1 })).toBeVisible()
  expect(screen.getAllByRole('button')).toHaveLength(1)
  fireEvent.click(screen.getByRole('button', { name: 'Return home' }))
  expect(close).toHaveBeenCalledOnce()
  // The selected Photo 2 title strip has no decorative title stars.
  expect(container.querySelector('.mcr-reference-window-stars')).toBeNull()
  expect(screen.getByRole('button', { name: 'Return home' }).querySelector('img')).toHaveAttribute('src', '/reference-window-controls-v2/close.svg')
  expect(screen.queryByRole('img')).toBeNull()
})

it('keeps real profile content in its scroll owner and hides the entire flow in peek without an ornamental footer', async () => {
  vi.stubGlobal('matchMedia', () => ({ matches: true, addEventListener() {}, removeEventListener() {} }))
  // WP-09: `/me` is the My Profile section home (the owner profile page). The
  // concert MeHub it used to render lives at `/me/profile`, which is where this
  // scroll-owner assertion now points.
  const fixture = createFixturePorts(), router = createMemoryRouter(createAppRoutes(fixture.ports), { initialEntries: ['/me/profile'] })
  try {
    render(<RouterProvider router={router} />)
    await screen.findByRole('heading', { name: 'Edit profile', level: 1 }, { timeout: 4000 })
    await screen.findAllByText(/^Could not load your profile/, {}, { timeout: 4000 })
    const body = document.getElementById('shell-panel-body')!
    const flow = body.querySelector('.mcr-reference-panel-flow')!
    const content = flow.querySelector('.mcr-reference-panel-content')!
    expect(body.querySelector('.mcr-chrome-parade-footer')).toBeNull()
    expect(content.parentElement).toBe(flow)
    expect(content.querySelector('[role="status"]')).toHaveTextContent('Could not load your profile')
    fireEvent.click(screen.getByRole('button', { name: 'Minimize panel' }))
    expect(body).not.toBeVisible()
    expect(content).not.toBeVisible()
    fireEvent.click(screen.getByRole('button', { name: 'Show panel content' }))
    expect(content).toBeVisible()
    expect(fixture.stats.mounts).toBe(1)
  } finally { cleanup(); router.dispose(); vi.unstubAllGlobals() }
})
