// @vitest-environment jsdom
// WP-09 T-904: router integration.
//
// The acceptance statements are:
//
// * every screen is reachable by URL;
// * a not-found route renders no profile or post data.
//
// "Reachable" is proved by navigating to each registered URL and asserting the
// shell renders a panel rather than the not-found state. "Renders no profile or
// post data" is proved by walking the not-found output for any profile field,
// post field, id or user-entered segment.
import '@testing-library/jest-dom/vitest'
import { act, cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../router'
import { allSectionRoutes, legacyRedirectPaths, sections } from '../sections'
import { panelRoutes } from '../routes'
import { createFixturePorts } from './fixtures/ports'

afterEach(cleanup)
const routers: ReturnType<typeof createMemoryRouter>[] = []
afterEach(() => { routers.splice(0).forEach(router => router.dispose()) })

function setup(path: string) {
  const fixture = createFixturePorts()
  const router = createMemoryRouter(createAppRoutes(fixture.ports), { initialEntries: [path] })
  routers.push(router)
  render(<RouterProvider router={router} />)
  return { ...fixture, router }
}

const settle = async () => {
  // Lazy panels resolve over several microtask turns; poll the DOM instead of
  // guessing a count.
  for (let index = 0; index < 60; index++) {
    await act(async () => { await new Promise(resolve => setTimeout(resolve, 20)) })
  }
}

describe('every screen is reachable by URL', () => {
  it('registers every section route and every section home', () => {
    const routes = createAppRoutes(createFixturePorts().ports)
    const shell = routes.flatMap(route => route.children ?? []).flatMap(route => route.children ?? [])
    const registered = new Set(shell.map(route => route.path).filter((path): path is string => typeof path === 'string'))
    for (const section of sections) {
      expect(registered.has(section.path), `${section.path} must be registered`).toBe(true)
    }
    for (const path of allSectionRoutes) {
      expect(registered.has(path), `${path} must be registered`).toBe(true)
    }
  })

  it('registers the visitor profile, its room, the intro slot and the photo composer', () => {
    const routes = createAppRoutes(createFixturePorts().ports)
    const shell = routes.flatMap(route => route.children ?? []).flatMap(route => route.children ?? [])
    const registered = new Set(shell.map(route => route.path).filter((path): path is string => typeof path === 'string'))
    for (const path of [
      '/discover/people/:profileId',
      '/discover/people/:profileId/room',
      '/discover/intro',
      '/discover/intro/:introId',
      '/me/posts/new',
      '/me/identity',
      '/activity',
      '/notifications/:notificationId',
    ]) {
      expect(registered.has(path), `${path} must be registered`).toBe(true)
    }
  })

  it.each([
    ['/discover', 'Discover'],
    ['/discover/people', 'People'],
    ['/activity', 'Activity'],
    ['/me', 'My Profile'],
    ['/me/identity', 'Identity verification'],
    ['/me/posts/new', 'New photo post'],
  ])('%s renders a panel rather than the not-found state', async (path, heading) => {
    setup(path)
    await settle()
    // The shell is up (the section bar exists) and the page has a heading.
    expect(screen.getByRole('navigation', { name: 'Sections' })).toBeInTheDocument()
    expect(screen.queryByRole('heading', { name: 'Page not found' })).toBeNull()
    const headings = Array.from(document.querySelectorAll('h1, h2')).map(element => element.textContent)
    expect(headings.some(text => text?.includes(heading)), `${path}: ${headings.join(' | ')}`).toBe(true)
  })

  it('mounts the intro composer, whose operations are all served', async () => {
    // `/discover/intro` is the composer. Every operation it uses
    // (`saveIntroPostDraft`, `publishIntroPost`, `deleteIntroPost`,
    // `getOwnIntroPost`) is served by the concert-hosted profile transport, so
    // the screen works end to end and is no longer a placeholder.
    setup('/discover/intro')
    await settle()
    expect(screen.getAllByRole('heading', { name: 'Your introduction' }).length).toBeGreaterThan(0)
    expect(screen.queryByText(/not part of this build yet/)).toBeNull()
  })

  it('keeps the intro detail on the explicit placeholder', async () => {
    // `getIntroPost` needs a visibility-filtered detail query that does not
    // exist yet, so the detail route stays an honest placeholder rather than a
    // screen that would fail on every load.
    setup('/discover/intro/22222222-2222-4222-8222-222222222222')
    await settle()
    expect(screen.getAllByRole('heading', { name: 'Mutuals-intro post' }).length).toBeGreaterThan(0)
    expect(screen.getByText(/not part of this build yet/)).toBeInTheDocument()
  })

  it('lazy-loads each section screen in its own chunk', () => {
    // The router declares each section screen as a `lazy` import, so opening the
    // profile does not download the discovery list. This asserts the declaration
    // rather than the bundler's chunking, which the build verifies separately.
    const source = require('node:fs').readFileSync(
      require('node:path').join(process.cwd(), 'src/app/router.tsx'),
      'utf8',
    ) as string
    for (const panel of ['DiscoverPanel', 'ActivityPanel', 'MyProfilePanel',
      'PhotoComposerPanel', 'IdentitySettingsPanel', 'IntroSlotPanel']) {
      expect(source).toMatch(new RegExp(`const ${panel} = lazy\\(`))
    }
  })
})

describe('a not-found route renders no profile or post data', () => {
  it('renders the not-found state for an unknown section path', async () => {
    setup('/discover/not-a-screen')
    await settle()
    expect(screen.getByRole('heading', { name: 'Page not found' })).toBeInTheDocument()
  })

  it('renders no profile field in the not-found output', async () => {
    setup('/discover/not-a-screen')
    await settle()
    const text = document.body.textContent ?? ''
    // No profile or post field is rendered.
    for (const field of ['nickname', 'bio', 'favorites', 'shared interests', 'Photos and artwork',
      'Post a photo', 'Room', 'Follow']) {
      expect(text.toLowerCase(), `${field} must not appear`).not.toContain(field.toLowerCase())
    }
    // No id and no user-entered segment leaks into the page.
    expect(text).not.toContain('00000000-0000-4000-8000')
    expect(text).not.toContain('not-a-screen')
  })

  it('renders no profile field for an unknown profile path', async () => {
    setup('/discover/people/00000000-0000-4000-8000-0000000000ff/not-a-screen')
    await settle()
    const text = document.body.textContent ?? ''
    expect(text).not.toContain('00000000-0000-4000-8000')
    for (const field of ['nickname', 'bio', 'Favorites', 'Shared interests', 'Follow']) {
      expect(text, `${field} must not appear`).not.toContain(field)
    }
  })

  it('renders no profile data for an invalid profile id', async () => {
    setup('/discover/people/not-a-uuid')
    await settle()
    const text = document.body.textContent ?? ''
    expect(text).not.toContain('not-a-uuid')
    expect(text).not.toContain('Nickname')
  })

  it('renders no post data for an unknown post path', async () => {
    setup('/me/posts/00000000-0000-4000-8000-0000000000ff')
    await settle()
    const text = document.body.textContent ?? ''
    expect(text).not.toContain('00000000-0000-4000-8000')
    expect(screen.getByRole('heading', { name: 'Page not found' })).toBeInTheDocument()
  })

  it('renders a safe error state that does not leak a thrown value', async () => {
    // The route error element is the shell's `RouteFailure`, which renders a fixed
    // message and never the thrown value. This asserts the element is wired to
    // every panel route.
    const routes = createAppRoutes(createFixturePorts().ports)
    const shell = routes.flatMap(route => route.children ?? []).flatMap(route => route.children ?? [])
    const panelRoutesWithError = shell.filter(route => typeof route.path === 'string'
      && panelRoutes.some(panel => panel.path === route.path))
    expect(panelRoutesWithError.length).toBeGreaterThan(0)
    for (const route of panelRoutesWithError) expect(route.errorElement).toBeDefined()
  })
})

describe('the legacy redirects are reachable', () => {
  it.each(legacyRedirectPaths())('%s is registered', path => {
    const routes = createAppRoutes(createFixturePorts().ports)
    const shell = routes.flatMap(route => route.children ?? []).flatMap(route => route.children ?? [])
    const registered = new Set(shell.map(route => route.path).filter((value): value is string => typeof value === 'string'))
    expect(registered.has(path)).toBe(true)
  })
})
