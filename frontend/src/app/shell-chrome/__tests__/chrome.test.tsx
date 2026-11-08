// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { createMemoryRouter, Outlet, RouterProvider, useLocation } from 'react-router-dom'
import { NavigationMemory, ShellLink, usePanelBack } from '../../navigation'
import { ShellHeader, BottomNavigation, EraControls, PanelHeader, type EraPort } from '../index'
import { SecondaryMenu } from '../../section-navigation'
import { createThemeController, type ThemeOptions } from '../../../features/theme/model/controller'
import type { LoadedMap } from '../../../world/map/resources'
import type { ThemeId } from '../../../features/theme/model/preferences'
const routers: ReturnType<typeof createMemoryRouter>[] = []
afterEach(() => { cleanup(); routers.splice(0).forEach(r => r.dispose()) })
const gift = '00000000-0000-4000-8000-000000000001'
function Page() {
  const location = useLocation(), back = usePanelBack()
  return <><h1>{location.pathname}</h1><button onClick={back}>Back</button><ShellLink to={`/gifts/${gift}`} id="fixture-gift">Open gift</ShellLink></>
}
// WP-09: the concert screens are reached from the secondary menu, which the shell
// renders above the bottom bar. The layout here mirrors that order.
function layout() { return <NavigationMemory><ShellHeader eventTitle="Authorized event" /><Outlet /><SecondaryMenu /><BottomNavigation /></NavigationMemory> }
function setup(path: string) {
  const router = createMemoryRouter([{ element: layout(), children: [{ path: '*', element: <Page /> }] }], { initialEntries: [path] })
  routers.push(router); render(<RouterProvider router={router} />); return router
}
// WP-09: the bottom bar is the three primary sections. A concert screen is not a
// tab, so it leaves every section inactive; a section screen activates exactly
// one. The concert destinations are asserted separately, in
// `__tests__/navigation.test.tsx`, through the secondary menu.
for (const [path, label] of [
  ['/discover', 'Discover'], ['/discover/people', 'Discover'],
  ['/activity', 'Activity'], ['/notifications', 'Activity'],
  ['/me', 'My Profile'], ['/me/avatar', 'My Profile'],
]) it(`resolves the active section for ${path}`, () => {
  setup(path!); const nav = screen.getByRole('navigation', { name: 'Sections' })
  expect(nav.querySelectorAll('[aria-current="page"]')).toHaveLength(1)
  expect(nav.querySelector('[aria-current="page"]')?.textContent).toBe(label)
  expect(nav.querySelectorAll('a')).toHaveLength(3); expect(nav.querySelectorAll('svg')).toHaveLength(3)
})
for (const path of ['/world/map', '/feed/new', `/meet/beacons/${gift}`, `/gifts/${gift}/reservation`, '/wall', `/notices/${gift}`]) it(`leaves every section inactive on the concert screen ${path}`, () => {
  setup(path!); const nav = screen.getByRole('navigation', { name: 'Sections' })
  expect(nav.querySelectorAll('[aria-current="page"]')).toHaveLength(0)
  expect(nav.querySelectorAll('a')).toHaveLength(3)
})
it('invalid deep link does not advertise an active authorized destination', () => {
  setup('/gifts/not-a-canonical-id'); expect(document.querySelector('[aria-current]')).toBeNull()
})
it('routes Inbox/Settings and bottom links with existing router state and stable focus IDs', async () => {
  const router = setup('/world')
  fireEvent.click(screen.getByRole('link', { name: 'Inbox' })); await screen.findByRole('heading', { name: '/notifications' })
  expect(router.state.location.state).toHaveProperty('shellPreviousKey')
  fireEvent.click(screen.getByRole('button', { name: 'Back' })); await screen.findByRole('heading', { name: '/world' })
  expect(document.activeElement?.id).toBe('shell-inbox')
  fireEvent.click(screen.getByRole('link', { name: 'Settings' })); await screen.findByRole('heading', { name: '/me/settings' })
  // The concert screens live in the secondary menu, which is a disclosure.
  fireEvent.click(screen.getByText('Concert screens'))
  fireEvent.click(screen.getByRole('link', { name: 'Gifts' })); await screen.findByRole('heading', { name: '/gifts' })
  fireEvent.click(screen.getByRole('link', { name: 'Open gift' })); await screen.findByRole('heading', { name: `/gifts/${gift}` })
  fireEvent.click(screen.getByRole('button', { name: 'Back' })); await screen.findByRole('heading', { name: '/gifts' })
  expect(document.activeElement?.id).toBe('fixture-gift')
})
it('direct detail Back safely replaces with its parent', async () => {
  const router = setup(`/gifts/${gift}`); fireEvent.click(screen.getByRole('button', { name: 'Back' }))
  await screen.findByRole('heading', { name: '/gifts' }); expect(router.state.historyAction).toBe('REPLACE')
})
function theme(overrides: Partial<ThemeOptions> = {}) {
  let skin = 'REVENGE'
  const preferences = { read: vi.fn(), write: vi.fn() }, controller = createThemeController({ initial: 'REVENGE', scope: { userId: 'test-user', eventId: 'test-event' }, preferences,
    runtime: { readSkin: () => skin, async replaceSkin(load, signal) { const map = await load(signal!); if (!signal?.aborted) skin = map.theme.theme_id } },
    load: async (id: ThemeId) => ({ theme: { theme_id: id }, dispose() {} }) as LoadedMap, ...overrides })
  return { controller, preferences }
}
it('absent theme controller is disabled, honest, and has no selected era', () => {
  render(<EraControls />); expect(screen.getByRole('status').textContent).toBe('Eras unavailable.')
  for (const button of screen.getAllByRole('button')) { expect((button as HTMLButtonElement).disabled).toBe(true); expect(button.getAttribute('aria-pressed')).toBe('false') }
})
it('actual controller keeps committed pressed state while loading; only confirms after runtime commit', async () => {
  let resolve!: (map: LoadedMap) => void
  const h = theme({ load: () => new Promise(done => { resolve = done }) })
  try {
    render(<EraControls controller={h.controller} />)
    fireEvent.click(screen.getByRole('button', { name: 'Parade' }))
    expect(screen.getByRole('button', { name: 'Revenge' }).getAttribute('aria-pressed')).toBe('true')
    expect(screen.getByRole('button', { name: 'Parade, loading' }).getAttribute('aria-pressed')).toBe('false')
    expect(h.preferences.write).not.toHaveBeenCalled(); expect(screen.getByRole('status').textContent).toContain('Revenge is still selected')
    await act(async () => resolve({ theme: { theme_id: 'PARADE' }, dispose() {} } as LoadedMap))
    expect(screen.getByRole('button', { name: 'Parade' }).getAttribute('aria-pressed')).toBe('true'); expect(h.preferences.write).toHaveBeenCalledOnce()
  } finally { h.controller.dispose() }
})
it('load failure retains actual old selection and shows safe retry text, never raw errors', async () => {
  const h = theme({ load: async () => { throw new Error('https://private.test/?token=raw') } })
  try {
    render(<EraControls controller={h.controller} />); fireEvent.click(screen.getByRole('button', { name: 'Danger' }))
    await waitFor(() => expect(h.controller.read().phase).toBe('error'))
    expect(screen.getByRole('status').textContent).toContain('Revenge is selected'); expect(document.body.textContent).not.toMatch(/private.test|token|raw/)
    expect(screen.getByRole('button', { name: 'Revenge' }).getAttribute('aria-pressed')).toBe('true')
  } finally { h.controller.dispose() }
})
it('storage failure reports actual new selected skin without a false successful save', async () => {
  const h = theme({ preferences: { read() {}, write() { throw new Error('private storage detail') } } })
  try {
    render(<EraControls controller={h.controller} />); fireEvent.click(screen.getByRole('button', { name: 'Parade' }))
    await waitFor(() => expect(h.controller.read().phase).toBe('error'))
    expect(screen.getByRole('button', { name: 'Parade' }).getAttribute('aria-pressed')).toBe('true'); expect(screen.getByRole('status').textContent).toContain('could not be loaded or saved')
  } finally { h.controller.dispose() }
})
it('disposal/withdrawal clears selection and releases old subscriptions; late results cannot restamp the view', async () => {
  let done!: (map: LoadedMap) => void, count = 0
  const h = theme({ load: () => new Promise(resolve => { done = resolve }) }), callbacks: (() => void)[] = []
  const port: EraPort = { ...h.controller, subscribe(fn) { count++; callbacks.push(fn); const stop = h.controller.subscribe(fn); return () => { count--; stop() } } }
  const result = render(<EraControls controller={port} />); expect(count).toBe(1)
  fireEvent.click(screen.getByRole('button', { name: 'Parade' }))
  act(() => h.controller.dispose()); expect(screen.getByRole('status').textContent).toBe('Eras unavailable.')
  result.rerender(<EraControls controller={null} />); expect(count).toBe(0)
  await act(async () => { done({ theme: { theme_id: 'PARADE' }, dispose() {} } as LoadedMap); callbacks.forEach(fn => fn()) })
  expect(screen.getByRole('status').textContent).toBe('Eras unavailable.'); expect(h.preferences.write).not.toHaveBeenCalled()
  expect(document.querySelector('[aria-pressed="true"]')).toBeNull()
})
it('panel header forwards only caller actions and exposes all sizing labels without owning navigation', () => {
  const back = vi.fn(), size = vi.fn(), content = vi.fn()
  const result = render(<PanelHeader title="Find each other" size="half" back={back} toggleSize={size} toggleContent={content} />)
  fireEvent.click(screen.getByRole('button', { name: 'Back to previous panel' })); fireEvent.click(screen.getByRole('button', { name: 'Expand panel' })); fireEvent.click(screen.getByRole('button', { name: 'Minimize panel' }))
  expect(back).toHaveBeenCalledOnce(); expect(size).toHaveBeenCalledOnce(); expect(content).toHaveBeenCalledOnce()
  result.rerender(<PanelHeader title="Find each other" size="peek" back={back} toggleSize={size} toggleContent={content} />)
  expect(screen.getByRole('button', { name: 'Show panel content' }).getAttribute('aria-expanded')).toBe('false')
})
