// @vitest-environment jsdom
// WP-09 T-903: the visitor profile page.
//
// The acceptance statements are:
//
// * no age is rendered for any visitor state;
// * shared interests are omitted when empty.
//
// The first is proved three ways: the view model has no age field at all, the
// rendered text is walked for any age-shaped string in every state (verified,
// unverified, opted-in, opted-out, unavailable), and a payload that *tries* to
// carry an age is dropped by the decoder rather than rendered.
import '@testing-library/jest-dom/vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { PublicProfilePage, admitGenderBucket, decodeVisitorProfile, sharedInterestLine } from '../PublicProfilePage'
import type { VisitorProfileView } from '../PublicProfilePage'

afterEach(cleanup)
const routers: ReturnType<typeof createMemoryRouter>[] = []
afterEach(() => { routers.splice(0).forEach(router => router.dispose()) })

const PROFILE = '00000000-0000-4000-8000-0000000000c1'

function view(overrides: Partial<VisitorProfileView> = {}): VisitorProfileView {
  return {
    profileId: PROFILE,
    nickname: 'Nightparade',
    bio: 'Looking for a band to join.',
    genderBucket: null,
    sharedInterests: ['Parade', 'Bocchi the Rock'],
    following: false,
    roomAvailable: true,
    ...overrides,
  }
}

function mount(ui: React.ReactNode) {
  const router = createMemoryRouter([{ path: '*', element: ui }], { initialEntries: [`/discover/people/${PROFILE}`] })
  routers.push(router)
  render(<RouterProvider router={router} />)
}

/** Any text that could be read as an age. */
const AGE_SHAPED = [
  /\bage\b/i,
  /\b\d{1,3}\s*(?:years?|yrs?|yo|y\/o)\b/i,
  /\b(?:aged|age:)\s*\d/i,
  /\b(?:1[4-9]|[2-9]\d|100)\b/, // the verified age range 14..100
]

function expectNoAge() {
  const text = document.body.textContent ?? ''
  for (const pattern of AGE_SHAPED) {
    expect(text, `rendered text must not match ${pattern}: ${text}`).not.toMatch(pattern)
  }
}

describe('no age is rendered for any visitor state', () => {
  it('renders no age for an unverified owner', () => {
    mount(<PublicProfilePage view={view({ genderBucket: null })} />)
    expectNoAge()
    expect(screen.queryByRole('heading', { name: 'Age' })).toBeNull()
    expect(screen.queryByRole('heading', { name: 'Gender' })).toBeNull()
  })

  it('renders no age for a verified owner who opted in to showing gender', () => {
    mount(<PublicProfilePage view={view({ genderBucket: 'female' })} />)
    expectNoAge()
    // The gender bucket is shown; the age is not.
    expect(screen.getByRole('heading', { name: 'Gender' })).toBeVisible()
    expect(screen.getByText('Female')).toBeVisible()
  })

  it('renders no age for a verified owner who did not opt in', () => {
    mount(<PublicProfilePage view={view({ genderBucket: null })} />)
    expectNoAge()
    expect(screen.queryByRole('heading', { name: 'Gender' })).toBeNull()
  })

  it('renders no age in the unavailable state', () => {
    mount(<PublicProfilePage view={null} />)
    expectNoAge()
    expect(screen.getByRole('status')).toHaveTextContent('unavailable')
  })

  it('drops an age that a payload tries to carry', () => {
    // A server payload with an age field is refused rather than rendered: the
    // decoder is closed, so an unexpected field is an error, not a value to show.
    expect(() => decodeVisitorProfile({
      profile_id: PROFILE,
      nickname: 'Nightparade',
      bio: null,
      age: 27,
      gender_bucket: null,
      shared_items: [],
      room_available: true,
    })).toThrow()
    // And a payload without the age field decodes, with no age anywhere.
    const decoded = decodeVisitorProfile({
      profile_id: PROFILE,
      nickname: 'Nightparade',
      bio: null,
      gender_bucket: 'male',
      shared_items: [],
      room_available: true,
    })
    expect(decoded).not.toHaveProperty('age')
    expect(JSON.stringify(decoded)).not.toContain('age')
  })

  it('admits only the three closed gender buckets', () => {
    expect(admitGenderBucket('female')).toBe('female')
    expect(admitGenderBucket('male')).toBe('male')
    expect(admitGenderBucket('unknown')).toBe('unknown')
    for (const refused of [null, undefined, '', 'FEMALE', 'nonbinary', 27, {}]) {
      expect(admitGenderBucket(refused)).toBeNull()
    }
  })
})

describe('shared interests are omitted when empty', () => {
  it('renders no shared-interest section for an empty list', () => {
    mount(<PublicProfilePage view={view({ sharedInterests: [] })} />)
    expect(screen.queryByRole('heading', { name: 'Shared interests' })).toBeNull()
    const text = document.body.textContent ?? ''
    expect(text).not.toContain('You both like')
    expect(text).not.toMatch(/you might get along/i)
    expect(text).not.toMatch(/no shared interests/i)
  })

  it('renders no shared-interest section for a list of blank names', () => {
    mount(<PublicProfilePage view={view({ sharedInterests: ['', '   '] })} />)
    expect(screen.queryByRole('heading', { name: 'Shared interests' })).toBeNull()
  })

  it('renders the shared interests when there is an overlap', () => {
    mount(<PublicProfilePage view={view()} />)
    expect(screen.getByRole('heading', { name: 'Shared interests' })).toBeVisible()
    expect(screen.getByText('You both like Parade and Bocchi the Rock.')).toBeVisible()
  })

  it('summarizes more than three shared items without listing them all', () => {
    expect(sharedInterestLine(['A', 'B', 'C', 'D', 'E'])).toBe('You both like A, B and C and 2 more.')
    expect(sharedInterestLine(['A'])).toBe('You both like A.')
    expect(sharedInterestLine(['A', 'B'])).toBe('You both like A and B.')
    expect(sharedInterestLine([])).toBeNull()
    expect(sharedInterestLine(['', '  '])).toBeNull()
  })
})

describe('the visitor page shows what a visitor may see', () => {
  it('shows the introduction, the photos, the follow action and the room entry', () => {
    mount(<PublicProfilePage view={view()} />)
    expect(screen.getByRole('heading', { level: 1, name: 'Profile' })).toBeVisible()
    expect(screen.getByText('Nightparade')).toBeVisible()
    expect(screen.getByText('Looking for a band to join.')).toBeVisible()
    expect(screen.getByRole('heading', { name: 'Photos and artwork' })).toBeVisible()
    expect(screen.getByRole('button', { name: 'Follow' })).toBeVisible()
    expect(screen.getByRole('heading', { name: 'Room' })).toBeVisible()
    expect(screen.getByRole('link', { name: 'Open room' })).toBeVisible()
  })

  it('never shows an owner-only control', () => {
    mount(<PublicProfilePage view={view()} />)
    for (const name of ['Edit profile', 'Customize character', 'Identity verification', 'Settings',
      'Post a photo or artwork', 'Delete post']) {
      expect(screen.queryByRole('link', { name }), `${name} must not be shown`).toBeNull()
      expect(screen.queryByRole('button', { name }), `${name} must not be shown`).toBeNull()
    }
  })

  it('toggles the follow action with aria-pressed', () => {
    const onFollow = vi.fn()
    mount(<PublicProfilePage view={view()} onFollow={onFollow} />)
    const follow = screen.getByRole('button', { name: 'Follow' })
    expect(follow.getAttribute('aria-pressed')).toBe('false')
    fireEvent.click(follow)
    expect(onFollow).toHaveBeenCalledTimes(1)
    cleanup()
    mount(<PublicProfilePage view={view({ following: true })} onFollow={onFollow} />)
    const following = screen.getByRole('button', { name: 'Following' })
    expect(following.getAttribute('aria-pressed')).toBe('true')
  })

  it('disables follow when no action is available rather than hiding it', () => {
    mount(<PublicProfilePage view={view()} />)
    expect((screen.getByRole('button', { name: 'Follow' }) as HTMLButtonElement).disabled).toBe(true)
  })

  it('shows the room entry as unavailable rather than a link that would 404', () => {
    mount(<PublicProfilePage view={view({ roomAvailable: false })} />)
    expect(screen.queryByRole('link', { name: 'Open room' })).toBeNull()
    expect(screen.getByText('Unavailable')).toBeVisible()
  })

  it('offers no room preview inline', () => {
    mount(<PublicProfilePage view={view()} />)
    expect(document.querySelector('canvas')).toBeNull()
    expect(document.querySelector('[data-room-scene]')).toBeNull()
    expect(document.querySelector('[data-room-preview]')).toBeNull()
    expect(document.querySelectorAll('img')).toHaveLength(0)
    const roomLinks = Array.from(document.querySelectorAll('a')).filter(link =>
      (link.getAttribute('href') ?? '').includes('/room'))
    expect(roomLinks).toHaveLength(1)
  })

  it('gives every control an accessible name', () => {
    mount(<PublicProfilePage view={view({ genderBucket: 'male' })} onFollow={() => undefined} />)
    for (const control of Array.from(document.querySelectorAll('a, button, input, select, textarea'))) {
      const name = control.getAttribute('aria-label') ?? control.textContent?.trim()
        ?? control.getAttribute('title') ?? ''
      expect(name, `control ${control.outerHTML.slice(0, 80)}`).not.toBe('')
    }
  })
})
