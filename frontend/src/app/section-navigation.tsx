/**
 * WP-09 T-901/T-905: the three-section tab bar and the concert secondary menu.
 *
 * Two navigations, deliberately separate:
 *
 * * **Primary** — exactly three sections, in product order, always visible. It is
 *   the tab bar. Its active state follows the *path*, not a stored selection, so
 *   a deep link and a click agree.
 * * **Secondary** — the concert screens, behind one labelled disclosure. It is
 *   not a fourth tab and it is not a second tab bar; a user who never opens it
 *   never sees it.
 *
 * Accessibility, all of which is asserted in `__tests__/navigation.test.tsx`:
 *
 * * the primary navigation is a `<nav>` with an accessible name, and each entry
 *   is a link with visible text, so every control has an accessible name;
 * * the active section carries `aria-current="page"` and nothing else does;
 * * the secondary menu is a `<details>`/`<summary>` disclosure, so it is
 *   reachable and operable by keyboard with no custom key handling and no trap;
 * * focus outlines are never removed — the shell's `:focus-visible` rule in
 *   `shared/styles/base.css` applies to every link here, and this module adds no
 *   `outline: none`.
 *
 * The prohibited decorative pattern — a coloured left border, inset left rail or
 * pseudo-element left bar on a tab, card or notice — does not appear in this
 * module or its stylesheet. Separation is done with spacing, weight and the
 * existing rule colour, which is what the user asked for.
 */
import { useId } from 'react'
import { useLocation } from 'react-router-dom'
import { ShellLink } from './navigation'
import './section-navigation.css'
import {
  secondaryDestinations,
  sectionForPath,
  sections,
  SECONDARY_MENU_LABEL,
  type SectionId,
} from './sections'

/** The icon glyph for a section. Text-first: the glyph is decorative. */
function SectionGlyph({ icon }: { icon: 'discover' | 'activity' | 'me' }) {
  const path = {
    discover: 'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18Zm3 6-2 5-5 2 2-5 5-2Z',
    activity: 'M3 12h4l2-6 4 12 2-6h6',
    me: 'M12 4a4 4 0 1 0 0 8 4 4 0 0 0 0-8Zm-8 16a8 8 0 0 1 16 0',
  }[icon]
  return <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false" className="mcr-section-nav__glyph">
    <path d={path} fill={icon === 'discover' ? 'currentColor' : 'none'} stroke="currentColor"
      strokeWidth={icon === 'discover' ? 0 : 1.8} strokeLinecap="round" strokeLinejoin="round" />
  </svg>
}

/** The icon glyph for a concert destination. Decorative. */
function DestinationGlyph({ icon }: { icon: 'world' | 'feed' | 'meet' | 'gifts' | 'wall' }) {
  const path = {
    world: 'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18Zm0 0c3 3 3 15 0 18M3 12h18',
    feed: 'M4 5h16v14H4zM4 9h16M8 13h8M8 16h5',
    meet: 'M8 11a3 3 0 1 0 0-6 3 3 0 0 0 0 6Zm8 0a3 3 0 1 0 0-6 3 3 0 0 0 0 6ZM3 20a5 5 0 0 1 10 0M13 20a5 5 0 0 1 8-4',
    gifts: 'M4 9h16v11H4zM2 9h20v4H2zM12 9v11M12 9c-3 0-5-1-5-3s3-3 5 3c2-6 5-5 5-3s-2 3-5 3',
    wall: 'M3 4h18v16H3zM3 10h18M9 4v16M15 10v10',
  }[icon]
  return <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false" className="mcr-section-nav__glyph">
    <path d={path} fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
  </svg>
}

/** The section a path belongs to, for the active state. */
export const activeSection = (pathname: string): SectionId | null => sectionForPath(pathname)

/**
 * The primary tab bar: exactly three sections.
 *
 * The active entry is derived from the current path on every render. A path that
 * is in no section (a concert screen, a public page) leaves every entry
 * inactive, which is the honest state: the user is not in a section.
 */
export function SectionNavigation() {
  const location = useLocation()
  const active = activeSection(location.pathname)
  return <nav className="mcr-section-nav" aria-label="Sections">
    {sections.map(section => <ShellLink key={section.id} id={`section-tab-${section.id.toLowerCase()}`}
      to={section.path} className="mcr-section-nav__link"
      aria-current={active === section.id ? 'page' : undefined}>
      <SectionGlyph icon={section.icon} />
      <span className="mcr-section-nav__label">{section.label}</span>
    </ShellLink>)}
  </nav>
}

/**
 * The concert secondary menu.
 *
 * A disclosure, not a tab. Opening it reveals the five concert screens in their
 * old order. It is labelled so a screen reader announces what it contains, and
 * the summary is a real control with an accessible name.
 */
export function SecondaryMenu() {
  const location = useLocation()
  const id = useId()
  // A concert screen is current only when the path is exactly one of the five
  // destinations; a detail screen inside one is not the destination itself.
  const current = secondaryDestinations.find(destination => destination.path === location.pathname)
  return <details className="mcr-secondary-menu" data-open={current ? 'true' : undefined}>
    <summary className="mcr-secondary-menu__summary">
      <span>{SECONDARY_MENU_LABEL}</span>
      {current && <span className="mcr-secondary-menu__current">{current.label}</span>}
    </summary>
    <nav aria-label={SECONDARY_MENU_LABEL} className="mcr-secondary-menu__nav">
      <ul id={`${id}-list`}>
        {secondaryDestinations.map(destination => <li key={destination.path}>
          <ShellLink to={destination.path} id={`concert-${destination.label.toLowerCase()}`}
            aria-current={current?.path === destination.path ? 'page' : undefined}>
            <DestinationGlyph icon={destination.icon} />
            <span>{destination.label}</span>
          </ShellLink>
        </li>)}
      </ul>
    </nav>
  </details>
}
