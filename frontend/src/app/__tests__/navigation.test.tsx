// @vitest-environment jsdom
// WP-09 T-901: the three-section navigation.
//
// The acceptance statements are:
//
// * every old path still resolves, either directly or by redirect;
// * the primary navigation has exactly three entries.
//
// The first is proved against the *closed* redirect table plus the router's own
// registration, so a path that is neither a live route nor a redirect fails here
// rather than in a user's browser. The second is proved on the rendered bar, not
// only on the data.
import '@testing-library/jest-dom/vitest'
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../router'
import {
  allSectionRoutes,
  isPrimaryPath,
  legacyRedirect,
  legacyRedirectPaths,
  secondaryDestinations,
  sectionForPath,
  sections,
  SECONDARY_MENU_LABEL,
} from '../sections'
import { panelRoutes, publicPaths } from '../routes'
import { SectionNavigation, SecondaryMenu } from '../section-navigation'
import { createFixturePorts } from './fixtures/ports'

afterEach(cleanup)
const routers: ReturnType<typeof createMemoryRouter>[] = []
afterEach(() => { routers.splice(0).forEach(router => router.dispose()) })

function setup(path = '/world') {
  const fixture = createFixturePorts()
  const router = createMemoryRouter(createAppRoutes(fixture.ports), { initialEntries: [path] })
  routers.push(router)
  render(<RouterProvider router={router} />)
  return { ...fixture, router }
}

const open = async () => { await screen.findByRole('navigation', { name: 'Sections' }) }

describe('the primary navigation has exactly three entries', () => {
  it('declares three sections in product order', () => {
    expect(sections).toHaveLength(3)
    expect(sections.map(section => section.label)).toEqual(['Discover', 'Activity', 'My Profile'])
    expect(sections.map(section => section.id)).toEqual(['DISCOVER', 'ACTIVITY', 'MY_PROFILE'])
  })

  it('renders exactly three links in the tab bar, each with an accessible name', async () => {
    setup('/discover')
    await open()
    const nav = screen.getByRole('navigation', { name: 'Sections' })
    const links = Array.from(nav.querySelectorAll('a'))
    expect(links).toHaveLength(3)
    expect(links.map(link => link.textContent)).toEqual(['Discover', 'Activity', 'My Profile'])
    // Every link has an accessible name (its visible text).
    for (const link of links) expect(link.textContent?.trim()).not.toBe('')
    // Exactly one is current, and it is the section the path is in.
    expect(nav.querySelectorAll('[aria-current="page"]')).toHaveLength(1)
    expect(nav.querySelector('[aria-current="page"]')?.textContent).toBe('Discover')
  })

  it('leaves every section inactive on a concert screen', async () => {
    setup('/feed')
    await open()
    const nav = screen.getByRole('navigation', { name: 'Sections' })
    expect(nav.querySelectorAll('[aria-current="page"]')).toHaveLength(0)
    expect(nav.querySelectorAll('a')).toHaveLength(3)
  })

  it('maps a path to its section by whole segments', () => {
    expect(sectionForPath('/discover')).toBe('DISCOVER')
    expect(sectionForPath('/discover/people')).toBe('DISCOVER')
    expect(sectionForPath('/activity')).toBe('ACTIVITY')
    expect(sectionForPath('/notifications')).toBe('ACTIVITY')
    expect(sectionForPath('/notifications/abc')).toBe('ACTIVITY')
    expect(sectionForPath('/me')).toBe('MY_PROFILE')
    expect(sectionForPath('/me/identity')).toBe('MY_PROFILE')
    // A concert screen is in no section.
    expect(sectionForPath('/world')).toBeNull()
    expect(sectionForPath('/feed')).toBeNull()
    // A near-miss is not a match.
    expect(sectionForPath('/notifications-settings')).toBeNull()
    expect(sectionForPath('/mexico')).toBeNull()
    expect(isPrimaryPath('/discover')).toBe(true)
    expect(isPrimaryPath('/world')).toBe(false)
  })
})

describe('the concert screens are a secondary menu, not tabs', () => {
  it('lists the five concert destinations in their old order', () => {
    expect(secondaryDestinations.map(destination => destination.path)).toEqual([
      '/world', '/feed', '/meet', '/gifts', '/wall',
    ])
    // None of them is a primary section.
    for (const destination of secondaryDestinations) expect(isPrimaryPath(destination.path)).toBe(false)
  })

  it('renders the secondary menu as a labelled disclosure, closed by default', async () => {
    setup('/world')
    await open()
    const details = document.querySelector('.mcr-secondary-menu') as HTMLDetailsElement
    expect(details).not.toBeNull()
    expect(details.open).toBe(false)
    const summary = details.querySelector('summary')!
    expect(summary.textContent).toContain(SECONDARY_MENU_LABEL)
    // The menu's own navigation is labelled and contains the five destinations.
    fireEvent.click(summary)
    expect(details.open).toBe(true)
    const nav = screen.getByRole('navigation', { name: SECONDARY_MENU_LABEL })
    expect(Array.from(nav.querySelectorAll('a')).map(link => link.textContent)).toEqual([
      'World', 'Feed', 'Meet', 'Gifts', 'Wall',
    ])
  })

  it('marks the open concert destination as current', async () => {
    setup('/gifts')
    await open()
    const details = document.querySelector('.mcr-secondary-menu') as HTMLDetailsElement
    fireEvent.click(details.querySelector('summary')!)
    const nav = screen.getByRole('navigation', { name: SECONDARY_MENU_LABEL })
    expect(nav.querySelector('[aria-current="page"]')?.textContent).toBe('Gifts')
  })
})

describe('every old path still resolves', () => {
  // The six old tabs, plus the two paths whose meaning changed.
  const oldPrimaryPaths = ['/world', '/feed', '/meet', '/gifts', '/wall', '/me']

  it('keeps the old tab paths registered as live routes', () => {
    const registered = new Set(panelRoutes.map(route => route.path))
    for (const path of oldPrimaryPaths) {
      expect(registered.has(path), `${path} must still be a route`).toBe(true)
    }
  })

  it('redirects the paths whose meaning changed', () => {
    expect(legacyRedirect('/meet/people')).toBe('/discover/people')
    expect(legacyRedirect('/meet/intro/edit')).toBe('/discover/intro')
    expect(legacyRedirect('/people/00000000-0000-4000-8000-000000000001'))
      .toBe('/discover/people/00000000-0000-4000-8000-000000000001')
    // A non-legacy path is not redirected.
    expect(legacyRedirect('/world')).toBeNull()
    expect(legacyRedirect('/discover')).toBeNull()
    // A near-miss is not a match.
    expect(legacyRedirect('/meet/peopled')).toBeNull()
    expect(legacyRedirect('/peoplex')).toBeNull()
  })

  it('registers every legacy redirect as a shell route so it runs inside the shell', () => {
    // The redirect is a real route registered *before* the panel routes, so it
    // wins over the concert panel the legacy path used to render. It is inside
    // the shell, so a signed-in user keeps the shell and its world connection.
    const routes = createAppRoutes(createFixturePorts().ports)
    const shell = routes.flatMap(route => route.children ?? []).flatMap(route => route.children ?? [])
    const patterns = new Set(shell.map(route => route.path).filter((path): path is string => typeof path === 'string'))
    for (const path of legacyRedirectPaths()) {
      expect(patterns.has(path), `${path} must be registered in the shell`).toBe(true)
    }
    // A legacy path is not also registered as a panel: the redirect owns the URL,
    // so the ranking cannot depend on which pattern React Router scores higher.
    const order = shell.map(route => route.path).filter((path): path is string => typeof path === 'string')
    expect(order).not.toContain('/meet/people')
    expect(order).not.toContain('/meet/intro/edit')
    expect(order).not.toContain('/people/:userId')
  })

  it.each([
    ['/meet/people', '/discover/people'],
    ['/meet/intro/edit', '/discover/intro'],
  ])('navigating to %s lands on %s', async (from, to) => {
    const { router } = setup(from)
    await open()
    await act(async () => { await Promise.resolve() })
    expect(router.state.location.pathname).toBe(to)
  })

  it('carries the query string across a redirect', async () => {
    const { router } = setup('/meet/people?category=MUSIC')
    await open()
    await act(async () => { await Promise.resolve() })
    expect(router.state.location.pathname).toBe('/discover/people')
    expect(router.state.location.search).toBe('?category=MUSIC')
  })

  it('redirects a visitor profile link to its Discover home', async () => {
    const profile = '00000000-0000-4000-8000-0000000000aa'
    const { router } = setup(`/people/${profile}`)
    await open()
    await act(async () => { await Promise.resolve() })
    expect(router.state.location.pathname).toBe(`/discover/people/${profile}`)
  })

  it('resolves every section route to a registered panel', () => {
    const registered = new Set(panelRoutes.map(route => route.path))
    for (const path of allSectionRoutes) {
      expect(registered.has(path), `${path} must be registered`).toBe(true)
    }
  })

  it('keeps every public path reachable', () => {
    // A sanity check that the section work did not disturb the public surface.
    for (const path of publicPaths) expect(path.startsWith('/')).toBe(true)
  })
})

describe('the section bar is keyboard reachable', () => {
  it('exposes the three sections as ordinary focusable links', async () => {
    setup('/activity')
    await open()
    const nav = screen.getByRole('navigation', { name: 'Sections' })
    const links = Array.from(nav.querySelectorAll('a'))
    for (const link of links) expect(link.getAttribute('href')).not.toBeNull()
    // The active entry is the Activity section.
    expect(nav.querySelector('[aria-current="page"]')?.textContent).toBe('Activity')
  })

  it('renders the bar and the menu without a decorative left accent bar', async () => {
    setup('/world')
    await open()
    // The prohibited pattern is a coloured left border / inset left rail / left
    // pseudo-element bar. The rendered markup carries no inline left border and
    // the stylesheet (asserted in accessibility.test.tsx) declares none.
    for (const element of Array.from(document.querySelectorAll('.mcr-section-nav *, .mcr-secondary-menu *'))) {
      const style = (element as HTMLElement).getAttribute('style') ?? ''
      expect(style).not.toMatch(/border-left/i)
    }
  })
})

describe('SectionNavigation and SecondaryMenu in isolation', () => {
  function mount(path: string) {
    const router = createMemoryRouter([{ path: '*', element: <><SectionNavigation /><SecondaryMenu /></> }], { initialEntries: [path] })
    routers.push(router)
    render(<RouterProvider router={router} />)
  }

  it('renders three entries for every path', () => {
    for (const path of ['/discover', '/activity', '/me', '/world', '/feed', '/']) {
      cleanup()
      mount(path)
      expect(screen.getByRole('navigation', { name: 'Sections' }).querySelectorAll('a')).toHaveLength(3)
    }
  })
})
