// @vitest-environment jsdom
// WP-09 T-905: the shell and tab bar.
//
// The acceptance statements are:
//
// * keyboard reachable, labelled, focus visible at 360/390/430;
// * no prohibited decorative pattern.
//
// "No prohibited decorative pattern" is the user's explicit prohibition: no
// decorative left accent bar — a coloured `border-left`, an inset left rail, or a
// `::before`/`::after` left bar — on any tab, card, notice or panel. The test
// reads the stylesheets the shell and the sections load and fails on any such
// declaration, and it also walks the rendered markup for an inline one.
import '@testing-library/jest-dom/vitest'
import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join } from 'node:path'
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../router'
import { createFixturePorts } from './fixtures/ports'
import { SECONDARY_MENU_LABEL } from '../sections'

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

/** Every CSS file under a directory, recursively. */
function cssFiles(directory: string): string[] {
  const found: string[] = []
  for (const entry of readdirSync(directory)) {
    const path = join(directory, entry)
    if (statSync(path).isDirectory()) found.push(...cssFiles(path))
    else if (entry.endsWith('.css')) found.push(path)
  }
  return found
}

describe('the tab bar has the three sections', () => {
  it('renders three labelled links and no concert tab', async () => {
    setup('/discover')
    await open()
    const nav = screen.getByRole('navigation', { name: 'Sections' })
    expect(nav.querySelectorAll('a')).toHaveLength(3)
    for (const label of ['Discover', 'Activity', 'My Profile']) {
      expect(screen.getByRole('link', { name: label })).toBeVisible()
    }
    // No concert screen is a tab.
    for (const label of ['World', 'Feed', 'Meet', 'Gifts', 'Wall']) {
      expect(nav.textContent).not.toContain(label)
    }
  })

  it('is keyboard reachable: every entry is a real link in tab order', async () => {
    setup('/activity')
    await open()
    const nav = screen.getByRole('navigation', { name: 'Sections' })
    const links = Array.from(nav.querySelectorAll('a'))
    for (const link of links) {
      expect(link.getAttribute('href')).not.toBeNull()
      expect(link.getAttribute('tabindex')).toBeNull() // natural tab order
    }
  })

  it('activates a section by click and by keyboard focus', async () => {
    const { router } = setup('/discover')
    await open()
    fireEvent.click(screen.getByRole('link', { name: 'Activity' }))
    await act(async () => { await Promise.resolve() })
    expect(router.state.location.pathname).toBe('/activity')
    const me = screen.getByRole('link', { name: 'My Profile' })
    me.focus()
    expect(document.activeElement).toBe(me)
    fireEvent.click(me)
    await act(async () => { await Promise.resolve() })
    expect(router.state.location.pathname).toBe('/me')
  })
})

describe('the secondary menu', () => {
  it('is a disclosure the keyboard can open and close', async () => {
    setup('/world')
    await open()
    const details = document.querySelector('.mcr-secondary-menu') as HTMLDetailsElement
    const summary = details.querySelector('summary')!
    expect(details.open).toBe(false)
    // A `<summary>` is focusable and toggles on click; the browser maps Enter and
    // Space to that click, so this is the keyboard path.
    summary.focus()
    expect(document.activeElement).toBe(summary)
    fireEvent.click(summary)
    expect(details.open).toBe(true)
    fireEvent.click(summary)
    expect(details.open).toBe(false)
  })

  it('labels the menu and each destination', async () => {
    setup('/world')
    await open()
    const details = document.querySelector('.mcr-secondary-menu') as HTMLDetailsElement
    fireEvent.click(details.querySelector('summary')!)
    const nav = screen.getByRole('navigation', { name: SECONDARY_MENU_LABEL })
    for (const link of Array.from(nav.querySelectorAll('a'))) {
      expect(link.textContent?.trim()).not.toBe('')
    }
  })
})

describe('focus outlines are preserved', () => {
  it('declares a visible focus style for links and buttons in the base stylesheet', () => {
    const base = readFileSync(join(process.cwd(), 'src/shared/styles/base.css'), 'utf8')
    expect(base).toMatch(/:focus-visible/)
    expect(base).toMatch(/outline:\s*3px solid/)
  })

  it('removes no outline in the shell or section stylesheets', () => {
    for (const file of [
      join(process.cwd(), 'src/app/section-navigation.css'),
      join(process.cwd(), 'src/app/section-entry/section-entry.css'),
      join(process.cwd(), 'src/app/shell-chrome/shell-chrome.css'),
      join(process.cwd(), 'src/app/shell.css'),
    ]) {
      const css = readFileSync(file, 'utf8')
      // `outline: none` / `outline: 0` would remove the focus ring. The only
      // permitted outline declarations are visible ones.
      expect(css, `${file} must not remove an outline`).not.toMatch(/outline:\s*(?:none|0)\b/)
    }
  })
})

describe('no decorative left accent bar anywhere', () => {
  it('declares no left accent bar in the shell and section stylesheets', () => {
    const files = [
      ...cssFiles(join(process.cwd(), 'src/app')),
      ...cssFiles(join(process.cwd(), 'src/shared/styles')),
    ]
    const offenders: string[] = []
    for (const file of files) {
      const css = readFileSync(file, 'utf8').replace(/\/\*[\s\S]*?\*\//g, '')
      // A border-left that names a colour or a width is a rail. `border-left: 0`
      // and `border-left: none` are resets, not decoration.
      for (const match of css.matchAll(/border-left\s*:\s*([^;]+);/g)) {
        const value = match[1]!.trim()
        if (!/^(?:0|none|initial|unset|inherit)$/i.test(value)) {
          offenders.push(`${file}: border-left: ${value}`)
        }
      }
      // A left pseudo-element bar: a positioned ::before/::after with a width or
      // a background on the left edge.
      for (const match of css.matchAll(/::?(?:before|after)\s*\{([^}]*)\}/g)) {
        const body = match[1]!
        if (/left\s*:\s*0/.test(body) && /(?:background|border)/.test(body) && /width/.test(body)) {
          offenders.push(`${file}: left pseudo-element bar`)
        }
      }
    }
    expect(offenders, offenders.join('\n')).toEqual([])
  })

  it('renders no inline left border on the shell or the sections', async () => {
    for (const path of ['/discover', '/activity', '/me', '/world']) {
      cleanup()
      setup(path)
      await open()
      for (const element of Array.from(document.querySelectorAll('[style]'))) {
        const style = (element as HTMLElement).getAttribute('style') ?? ''
        expect(style, `${path}: ${element.outerHTML.slice(0, 120)}`).not.toMatch(/border-left/i)
      }
    }
  })

  it('does not mark a section with a decorative left rail class', async () => {
    setup('/me')
    await open()
    for (const element of Array.from(document.querySelectorAll('[class]'))) {
      const classes = element.getAttribute('class') ?? ''
      expect(classes).not.toMatch(/\b(?:accent|rail|left-bar|bar-left)\b/i)
    }
  })
})

describe('the shell layout at the three widths', () => {
  it('uses a single-column, wrapping layout that cannot overflow 360 px', () => {
    const nav = readFileSync(join(process.cwd(), 'src/app/section-navigation.css'), 'utf8')
    expect(nav).toMatch(/grid-template-columns:\s*repeat\(3,\s*minmax\(0,\s*1fr\)\)/)
    expect(nav).toMatch(/overflow-wrap:\s*anywhere/)
    const page = readFileSync(join(process.cwd(), 'src/app/section-entry/section-entry.css'), 'utf8')
    expect(page).toMatch(/overflow-wrap:\s*anywhere/)
    // A fixed width is only a problem when it is not a `max-width`: a `max-width`
    // shrinks on a narrow viewport, a `width` does not.
    for (const match of page.matchAll(/(?<!max-)width:\s*(\d+)px/g)) {
      expect(Number(match[1]), `fixed width ${match[1]}px`).toBeLessThanOrEqual(360)
    }
  })

  it('keeps every section control at least a 44 px hit target', () => {
    const nav = readFileSync(join(process.cwd(), 'src/app/section-navigation.css'), 'utf8')
    expect(nav).toMatch(/min-height:\s*(?:44px|var\(--pi-hit)/)
    const page = readFileSync(join(process.cwd(), 'src/app/section-entry/section-entry.css'), 'utf8')
    expect(page).toMatch(/min-height:\s*44px/)
  })
})
