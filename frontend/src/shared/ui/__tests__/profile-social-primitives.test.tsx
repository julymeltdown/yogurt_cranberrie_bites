// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { readFileSync } from 'node:fs'
import { afterEach, describe, expect, it } from 'vitest'
import { cleanup, render, screen, within } from '@testing-library/react'
import {
  AvatarChip,
  Badge,
  Card,
  EmptyState,
  InlineError,
  List,
  MediaGrid,
  MediaTile,
  Row,
  RowLink,
  Screen,
  Section,
  SectionHeader,
  TabStrip,
  /* WP-10 round 2. */
  Chips,
  Facts,
  IconButton,
  SocialIcon,
  State,
  Steps,
} from '../index'

afterEach(cleanup)

const read = (path: string) => readFileSync(new URL(path, import.meta.url), 'utf8')

/* ---------------------------------------------------------------------------
 * The prohibition guard.
 *
 * The product rule is explicit: no decorative left accent line, coloured left
 * border, inset left rail or pseudo-element left bar anywhere in product UI,
 * notices, error panels, toasts, skeletons or documentation-style panels.
 *
 * This guard reads the actual stylesheets, not just the markup, so a future
 * edit that reintroduces the pattern fails here rather than in review. It
 * covers the shared primitives and every screen this package restyled.
 * ------------------------------------------------------------------------- */

const SURFACES = [
  '../../styles/parade-ink/profile-social.css',
  '../../../features/favorites/favorites.css',
  '../../../features/discovery/discovery.css',
  '../../../features/profile-media/profile-media.css',
  '../../../features/profile-room/profile-room.css',
  '../../../features/identity/identity.css',
  '../../../features/profile-avatar/ui/profile-avatar.css',
  '../../../features/profile-avatar/ui/body-palette.css',
  /* WP-10 round 2: the screens this round restyled. */
  '../../../features/intros/intros.css',
  '../../../features/profile/profile.css',
] as const

/** Strip comments so prose that *names* the prohibition is not a violation. */
function code(css: string): string {
  return css.replace(/\/\*[\s\S]*?\*\//g, '')
}

describe('no decorative left accent anywhere', () => {
  it.each(SURFACES)('%s declares no left edge, rail or left border', path => {
    const css = code(read(path))
    // A left-only border is the prohibited pattern in every spelling.
    expect(css).not.toMatch(/border-left\s*:/)
    expect(css).not.toMatch(/border-inline-start\s*:/)
    expect(css).not.toMatch(/border-left-(width|style|color)\s*:/)
    expect(css).not.toMatch(/border-inline-start-(width|style|color)\s*:/)
    // A pseudo-element bar is the usual substitute for one.
    expect(css).not.toMatch(/::?(before|after)/)
    // No inset rail: a left inset shadow or a left-positioned absolute bar.
    expect(css).not.toMatch(/box-shadow\s*:[^;]*inset[^;]*\d+px\s+0/)
    expect(css).not.toMatch(/inset-inline-start\s*:\s*0[^;]*;[\s\S]{0,120}?inline-size\s*:\s*(?:[1-8]px)/)
  })

  it('the shared primitives stylesheet uses only symmetric borders', () => {
    const css = code(read('../../styles/parade-ink/profile-social.css'))
    // Every border declaration is the shorthand (symmetric) or a full set.
    for (const match of css.matchAll(/border(?:-[a-z]+)*\s*:\s*[^;]+;/g)) {
      const declaration = match[0]
      expect(declaration, `asymmetric border: ${declaration}`).not.toMatch(/-left|-right|-inline-start|-inline-end/)
    }
  })

  it('status and error panels carry their state by border and text, not by an edge', () => {
    const css = code(read('../../styles/parade-ink/profile-social.css'))
    const error = /\.pi-inline-error\s*\{([^}]*)\}/.exec(css)?.[1] ?? ''
    expect(error).toContain('border: var(--pi-rule-width) solid var(--pi-status-danger)')
    // The danger colour is on the whole border, never one side.
    expect(error).not.toMatch(/-left|-inline-start/)
    const badge = /\.pi-badge\s*\{([^}]*)\}/.exec(css)?.[1] ?? ''
    expect(badge).toContain('border: var(--pi-rule-width) solid var(--pi-status-neutral)')
  })
})

describe('focus and control borders are preserved', () => {
  it('every restyled surface keeps a visible focus outline', () => {
    const primitives = code(read('../../styles/parade-ink/profile-social.css'))
    expect(primitives).toContain('outline: var(--pi-focus-ring) solid var(--pi-focus)')
    // Each feature stylesheet still declares a focus rule of its own.
    for (const path of SURFACES.slice(1)) {
      expect(code(read(path)), `${path} lost its focus rule`).toMatch(/focus-visible/)
    }
  })

  it('the shared controls keep ordinary symmetric borders', () => {
    const tokens = read('../../styles/parade-ink/tokens.css')
    expect(tokens).toContain('--pi-rule-width: 1px')
    expect(tokens).toContain('--pi-focus-ring: 3px')
    expect(tokens).toContain('--pi-focus-offset: 2px')
    const primitives = code(read('../../styles/parade-ink/primitives.css'))
    expect(primitives).toMatch(/\.mcr-button[\s\S]*?border:\s*var\(--pi-rule-width\) solid var\(--pi-rule\)/)
  })

  it('no restyled screen reintroduces a local palette or its own radius system', () => {
    for (const path of SURFACES.slice(1)) {
      const css = code(read(path))
      // A local colour literal would be a second palette.
      expect(css, `${path} declares a raw colour`).not.toMatch(/#[0-9a-f]{3,8}\b/i)
      expect(css, `${path} declares rgb()/hsl()`).not.toMatch(/\b(?:rgb|rgba|hsl|hsla)\(/)
      // The shared radius is the only radius; no 12px cards or 999px pills
      // except the deliberate badge/pill shape.
      for (const match of css.matchAll(/border-radius\s*:\s*([^;]+);/g)) {
        const value = match[1]!.trim()
        expect(value, `${path} radius ${value}`).toMatch(/^(?:var\(--pi-(?:radius|media-radius)\)|999px|0)$/)
      }
    }
  })
})

describe('the primitives render accessible, symmetric markup', () => {
  it('SectionHeader renders a heading with an optional count', () => {
    render(<SectionHeader title="Favorites" count={3} description="Pick up to five." />)
    const heading = screen.getByRole('heading', { name: /Favorites/ })
    expect(heading).toHaveClass('pi-section__heading')
    expect(heading.tagName).toBe('H2')
    expect(screen.getByText('3')).toHaveClass('pi-section__count')
    expect(screen.getByText('Pick up to five.')).toHaveClass('pi-section__description')
  })

  it('Section honours the heading level so the outline stays correct', () => {
    render(<Section title="Details" level={3}>Body</Section>)
    expect(screen.getByRole('heading', { name: 'Details', level: 3 })).toBeInTheDocument()
  })

  it('Card renders a titled object with an optional footer', () => {
    render(<Card title="Room" footer={<button type="button">Open</button>}>Layout</Card>)
    expect(screen.getByRole('heading', { name: 'Room', level: 3 })).toHaveClass('pi-card__title')
    expect(screen.getByRole('button', { name: 'Open' })).toBeInTheDocument()
  })

  it('List and Row build a dense list without per-row boxes', () => {
    render(
      <List label="People">
        <li><Row title="Ada" meta={<span>2 shared</span>} actions={<button type="button">Follow</button>}>Hi</Row></li>
        <li><Row layout="no-media" title="Grace" /></li>
      </List>,
    )
    expect(screen.getByRole('list', { name: 'People' })).toHaveClass('pi-list')
    expect(screen.getByText('Ada')).toHaveClass('pi-row__title')
    expect(screen.getByText('2 shared').parentElement).toHaveClass('pi-row__meta')
    expect(screen.getByRole('button', { name: 'Follow' }).parentElement).toHaveClass('pi-row__actions')
  })

  it('RowLink is one focusable target with a symmetric focus ring', () => {
    render(<RowLink href="/profiles/1" title="Ada" meta="Open profile" />)
    const link = screen.getByRole('link', { name: /Ada/ })
    expect(link).toHaveClass('pi-row-link')
    expect(link).toHaveAttribute('href', '/profiles/1')
  })

  it('MediaTile reserves its ratio and states a missing image explicitly', () => {
    const { rerender } = render(<MediaTile src="/a.png" alt="A poster" caption="Poster" />)
    expect(screen.getByRole('img', { name: 'A poster' })).toHaveAttribute('loading', 'lazy')
    expect(screen.getByText('Poster')).toHaveClass('pi-media-tile__caption')
    rerender(<MediaTile src={null} alt="A poster" emptyLabel="No image yet" />)
    // No substitute image is rendered; the state is text.
    expect(screen.queryByRole('img')).toBeNull()
    expect(screen.getByText('No image yet')).toHaveClass('pi-media-tile__state')
  })

  it('MediaGrid is a labelled list', () => {
    render(<MediaGrid label="Photos"><li><MediaTile src="/a.png" alt="One" /></li></MediaGrid>)
    expect(screen.getByRole('list', { name: 'Photos' })).toHaveClass('pi-media-grid')
  })

  it('AvatarChip falls back to the initial, never to substitute art', () => {
    const { container } = render(<AvatarChip name="julymeltdown" meta="Lv.12" />)
    // The initial is decorative: the chip's accessible name is the visible name,
    // so the frame is aria-hidden and the initial is queried directly.
    const initial = container.querySelector('.pi-avatar-chip__initial')
    // The raw first character is rendered; the uppercase presentation is CSS.
    expect(initial).toHaveTextContent('j')
    expect(initial!.parentElement).toHaveAttribute('aria-hidden', 'true')
    expect(code(read('../../styles/parade-ink/profile-social.css')))
      .toMatch(/\.pi-avatar-chip__initial\s*\{[^}]*text-transform:\s*uppercase/)
    expect(screen.getByText('julymeltdown')).toHaveClass('pi-avatar-chip__name')
    expect(screen.getByText('Lv.12')).toHaveClass('pi-avatar-chip__meta')
    // A chip with no character renders no image element at all.
    expect(screen.queryByRole('img')).toBeNull()
  })

  it('AvatarChip accepts a rendered character and an optional link', () => {
    render(<AvatarChip name="ada" href="/profiles/1" media={<canvas width={64} height={64} />} />)
    expect(screen.getByRole('link', { name: /ada/ })).toHaveClass('pi-avatar-chip--link')
  })

  it('Badge carries its meaning in text, not only in colour', () => {
    render(<><Badge tone="ok">Verified</Badge><Badge tone="danger">Failed</Badge></>)
    expect(screen.getByText('Verified')).toHaveClass('pi-badge', 'pi-badge--ok')
    expect(screen.getByText('Failed')).toHaveClass('pi-badge', 'pi-badge--danger')
  })

  it('EmptyState states what is missing and offers the next step', () => {
    render(<EmptyState title="No posts yet" action={<button type="button">Add a post</button>}>Your photos will appear here.</EmptyState>)
    expect(screen.getByText('No posts yet')).toHaveClass('pi-empty__title')
    expect(screen.getByRole('button', { name: 'Add a post' })).toBeInTheDocument()
  })

  it('InlineError is announced and keeps its actions reachable', () => {
    render(<InlineError title="Not saved" action={<button type="button">Retry</button>}>The room changed elsewhere.</InlineError>)
    const alert = screen.getByRole('alert')
    expect(alert).toHaveClass('pi-inline-error')
    expect(alert).toHaveAttribute('aria-live', 'assertive')
    expect(screen.getByRole('button', { name: 'Retry' })).toBeInTheDocument()
  })

  it('InlineError can be a polite message', () => {
    render(<InlineError title="Note" live="polite">Nothing was changed.</InlineError>)
    expect(screen.getByRole('alert')).toHaveAttribute('aria-live', 'polite')
  })

  it('Screen provides one h1 and a lead line', () => {
    render(<Screen title="Discover" lead="Find people">Body</Screen>)
    expect(screen.getByRole('heading', { level: 1, name: 'Discover' })).toHaveClass('pi-screen__title')
    expect(screen.getByText('Find people')).toHaveClass('pi-screen__lead')
  })

  it('TabStrip keeps the tab contract and marks the selected tab', () => {
    const calls: string[] = []
    render(<TabStrip label="Lists" idPrefix="d" selected="people"
      onSelect={id => calls.push(id)}
      items={[{ id: 'people', label: 'People', count: 2 }, { id: 'intros', label: 'Intros' }]} />)
    const tabs = screen.getAllByRole('tab')
    expect(tabs).toHaveLength(2)
    expect(tabs[0]).toHaveAttribute('aria-selected', 'true')
    expect(tabs[0]).toHaveAttribute('tabindex', '0')
    expect(tabs[1]).toHaveAttribute('aria-selected', 'false')
    expect(tabs[1]).toHaveAttribute('tabindex', '-1')
    tabs[1]!.click()
    expect(calls).toEqual(['intros'])
  })
})

describe('round-2 primitives', () => {
  it('State shares one geometry across empty, loading and error', () => {
    const { rerender } = render(<State kind="empty" title="No posts yet">Nothing here.</State>)
    expect(screen.getByRole('status')).toHaveClass('pi-state')
    expect(screen.getByText('No posts yet')).toHaveClass('pi-state__title')
    rerender(<State kind="loading" title="Loading posts" />)
    expect(screen.getByRole('status')).toHaveClass('pi-state', 'pi-state--loading')
    rerender(<State kind="error" title="Could not load" action={<button type="button">Retry</button>} />)
    // An error is announced assertively, not as a passive status.
    expect(screen.getByRole('alert')).toHaveClass('pi-state', 'pi-state--error')
    expect(screen.getByRole('button', { name: 'Retry' })).toBeInTheDocument()
  })

  it('Chips renders a labelled pill row and nothing when empty', () => {
    const { rerender } = render(<Chips label="Favorites" items={['MCR', 'NANA']} />)
    const list = screen.getByRole('list', { name: 'Favorites' })
    expect(list).toHaveClass('pi-chips')
    expect(within(list).getAllByRole('listitem')).toHaveLength(2)
    rerender(<Chips label="Favorites" items={[]} />)
    expect(screen.queryByRole('list')).toBeNull()
  })

  it('Steps keeps ordered-list semantics and marks the current step', () => {
    render(<Steps label="Composer steps" current="content"
      items={[{ id: 'audience', label: 'Audience' }, { id: 'content', label: 'Content' }, { id: 'review', label: 'Review' }]} />)
    const list = screen.getByRole('list', { name: 'Composer steps' })
    expect(list.tagName).toBe('OL')
    const items = within(list).getAllByRole('listitem')
    expect(items).toHaveLength(3)
    expect(items[1]).toHaveAttribute('aria-current', 'step')
    expect(items[0]).not.toHaveAttribute('aria-current')
  })

  it('Facts renders a label/value definition list', () => {
    render(<Facts rows={[{ label: 'Audience', value: 'Everyone' }, { label: 'Body', value: 'Hello' }]} />)
    expect(screen.getByText('Audience').tagName).toBe('DT')
    expect(screen.getByText('Everyone').tagName).toBe('DD')
  })

  it('IconButton carries its accessible name and pressed state', () => {
    const calls: number[] = []
    render(<IconButton icon={<SocialIcon name="heart" />} label="Leave a heart" count={42}
      pressed={false} onClick={() => calls.push(1)} />)
    const button = screen.getByRole('button', { name: 'Leave a heart' })
    expect(button).toHaveClass('pi-icon-button')
    expect(button).toHaveAttribute('aria-pressed', 'false')
    expect(button).toHaveTextContent('42')
    button.click()
    expect(calls).toHaveLength(1)
  })

  it('IconButton blocks activation while busy', () => {
    const calls: number[] = []
    render(<IconButton icon={<SocialIcon name="heart" />} label="Leave a heart" busy onClick={() => calls.push(1)} />)
    const button = screen.getByRole('button', { name: 'Leave a heart' })
    expect(button).toBeDisabled()
    expect(button).toHaveAttribute('aria-busy', 'true')
    button.click()
    expect(calls).toHaveLength(0)
  })
})

describe('the icon set', () => {
  it('renders one family: 24px viewBox, stroke 2, square caps, decorative', () => {
    const { container } = render(<SocialIcon name="heart" />)
    const svg = container.querySelector('svg')!
    expect(svg).toHaveAttribute('viewBox', '0 0 24 24')
    expect(svg).toHaveAttribute('stroke', 'currentColor')
    expect(svg).toHaveAttribute('stroke-width', '2')
    expect(svg).toHaveAttribute('stroke-linecap', 'square')
    expect(svg).toHaveAttribute('fill', 'none')
    // Decorative: the containing control supplies the name.
    expect(svg).toHaveAttribute('aria-hidden', 'true')
    expect(svg).toHaveAttribute('focusable', 'false')
  })

  it('every named icon renders a path', () => {
    const names = ['heart', 'heart-filled', 'comment', 'share', 'people', 'person', 'photo',
      'calendar', 'location', 'lock', 'chevron-right', 'edit', 'plus', 'warning'] as const
    for (const name of names) {
      const { container, unmount } = render(<SocialIcon name={name} />)
      expect(container.querySelector('svg')!.children.length, name).toBeGreaterThan(0)
      unmount()
    }
  })

  it('the filled heart is a distinct pressed state, not a colour swap', () => {
    const { container, rerender } = render(<SocialIcon name="heart" />)
    expect(container.querySelector('path[fill="currentColor"]')).toBeNull()
    rerender(<SocialIcon name="heart-filled" />)
    expect(container.querySelector('path[fill="currentColor"]')).not.toBeNull()
  })
})
