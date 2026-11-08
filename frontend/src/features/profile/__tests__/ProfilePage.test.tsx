// @vitest-environment jsdom
// WP-09 T-902: the owner profile page.
//
// The acceptance statements are:
//
// * the room opens only from its entry point;
// * the visitor variant never shows owner-only controls.
//
// "Only from its entry point" is proved structurally: the page renders exactly
// one link to the room, and it renders no room scene, no room thumbnail and no
// room data. A separate test walks the page's markup for anything that would be a
// room preview.
import '@testing-library/jest-dom/vitest'
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { ProfilePage } from '../ProfilePage'
import { roomPath } from '../RoomEntry'
import type { ProfilePageView } from '../ProfilePage'

afterEach(cleanup)
const routers: ReturnType<typeof createMemoryRouter>[] = []
afterEach(() => { routers.splice(0).forEach(router => router.dispose()) })

const PROFILE = '00000000-0000-4000-8000-0000000000b1'

function view(overrides: Partial<ProfilePageView> = {}): ProfilePageView {
  return {
    profileId: PROFILE,
    nickname: 'Cranberrie',
    bio: 'I like loud guitars.',
    favorites: [
      { category: 'MUSIC', label: 'Parade' },
      { category: 'ANIME', label: 'Bocchi the Rock' },
    ],
    roomAvailable: true,
    ...overrides,
  }
}

function mount(ui: React.ReactNode) {
  const router = createMemoryRouter([{ path: '*', element: ui }], { initialEntries: ['/me'] })
  routers.push(router)
  render(<RouterProvider router={router} />)
}

describe('the owner profile page', () => {
  it('shows the introduction, the favorites, the character, the posts and the room entry', () => {
    mount(<ProfilePage view={view()} isOwner />)
    expect(screen.getByRole('heading', { level: 1, name: 'My Profile' })).toBeVisible()
    expect(screen.getByText('Cranberrie')).toBeVisible()
    expect(screen.getByText('I like loud guitars.')).toBeVisible()
    // Favorites, grouped by category.
    expect(screen.getByRole('heading', { name: 'Favorites' })).toBeVisible()
    expect(screen.getByText('Parade')).toBeVisible()
    expect(screen.getByText('Bocchi the Rock')).toBeVisible()
    // The emo character section.
    expect(screen.getByRole('heading', { name: 'Character' })).toBeVisible()
    // The posts section and its composer entry.
    expect(screen.getByRole('heading', { name: 'Photos and artwork' })).toBeVisible()
    expect(screen.getByRole('link', { name: 'Post a photo or artwork' })).toBeVisible()
    // The room entry.
    expect(screen.getByRole('heading', { name: 'Room' })).toBeVisible()
    expect(screen.getByRole('link', { name: 'Open room' })).toBeVisible()
  })

  it('renders no age anywhere on the owner page', () => {
    mount(<ProfilePage view={view()} isOwner />)
    const text = document.body.textContent ?? ''
    // The owner's verified age belongs to the identity settings screen. The
    // profile page never renders it.
    expect(text).not.toMatch(/\bage\b/i)
    expect(text).not.toMatch(/\b\d{1,2} years?\b/i)
  })

  it('offers the room only through its entry, with no inline preview', () => {
    mount(<ProfilePage view={view()} isOwner />)
    const roomLinks = Array.from(document.querySelectorAll('a')).filter(link =>
      (link.getAttribute('href') ?? '').includes('/room'))
    expect(roomLinks).toHaveLength(1)
    expect(roomLinks[0]!.getAttribute('href')).toBe(roomPath(PROFILE))
    // No room scene, canvas, poster or layout data is rendered on the profile.
    expect(document.querySelector('canvas')).toBeNull()
    expect(document.querySelector('[data-room-scene]')).toBeNull()
    expect(document.querySelector('[data-room-preview]')).toBeNull()
    expect(document.querySelector('[data-poster]')).toBeNull()
    // The room entry's own copy may *describe* the room ("arrange furniture and
    // posters"); what must not appear is a room preview: no scene element, no
    // poster image, no layout data. Those are asserted above by element.
    expect(document.querySelectorAll('img')).toHaveLength(0)
  })

  it('shows the room entry as unavailable rather than a link that would 404', () => {
    mount(<ProfilePage view={view({ roomAvailable: false })} isOwner />)
    expect(screen.queryByRole('link', { name: 'Open room' })).toBeNull()
    expect(screen.getByText('Unavailable')).toBeVisible()
  })

  it('shows owner-only controls for the owner', () => {
    mount(<ProfilePage view={view()} isOwner />)
    expect(screen.getByRole('link', { name: 'Edit profile' })).toBeVisible()
    expect(screen.getByRole('link', { name: 'Customize character' })).toBeVisible()
    expect(screen.getByRole('link', { name: 'Identity verification' })).toBeVisible()
    expect(screen.getByRole('link', { name: 'Settings' })).toBeVisible()
  })

  it('never shows owner-only controls to a visitor', () => {
    mount(<ProfilePage view={view()} isOwner={false} />)
    for (const name of ['Edit profile', 'Customize character', 'Identity verification', 'Settings',
      'Post a photo or artwork']) {
      expect(screen.queryByRole('link', { name }), `${name} must not be shown`).toBeNull()
    }
    // A visitor still sees the profile itself.
    expect(screen.getByText('Cranberrie')).toBeVisible()
    expect(screen.getByRole('link', { name: 'Open room' })).toBeVisible()
  })

  it('omits the favorites section when there are none', () => {
    mount(<ProfilePage view={view({ favorites: [] })} isOwner />)
    // The heading is absent rather than an empty list claiming the user has none.
    expect(screen.queryByRole('heading', { name: 'Favorites' })).toBeNull()
  })

  it('says the profile is unavailable rather than rendering a stale one', () => {
    mount(<ProfilePage view={null} isOwner />)
    expect(screen.getByRole('status')).toHaveTextContent('unavailable')
    expect(screen.queryByRole('heading', { name: 'Room' })).toBeNull()
  })

  it('gives every control an accessible name', () => {
    mount(<ProfilePage view={view()} isOwner />)
    const controls = Array.from(document.querySelectorAll('a, button, input, select, textarea'))
    for (const control of controls) {
      const name = control.getAttribute('aria-label') ?? control.textContent?.trim()
        ?? control.getAttribute('title') ?? ''
      expect(name, `control ${control.outerHTML.slice(0, 80)}`).not.toBe('')
    }
  })

  it('orders headings without skipping a level', () => {
    mount(<ProfilePage view={view()} isOwner />)
    const levels = Array.from(document.querySelectorAll('h1, h2, h3, h4, h5, h6'))
      .map(heading => Number(heading.tagName.slice(1)))
    expect(levels[0]).toBe(1)
    for (let index = 1; index < levels.length; index++) {
      expect(levels[index]! - levels[index - 1]!).toBeLessThanOrEqual(1)
    }
  })
})
