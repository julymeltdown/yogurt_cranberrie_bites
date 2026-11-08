// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { IntroLikersPanel } from '../IntroLikersPanel'
import { sharedInterestLine, type LikerProfile } from '../contract'

afterEach(cleanup)

function liker(overrides: Partial<LikerProfile> = {}): LikerProfile {
  return {
    profile_id: '00000000-0000-4000-8000-0000000000bb',
    nickname: 'Mina',
    profile_image_media_id: null,
    shared_with_author: [
      { catalog_item_id: '00000000-0000-4000-8000-0000000000c1', label: 'My Chemical Romance', category: 'MUSIC', external_url: null },
    ],
    interaction_style: 'Slow replies, long messages',
    liked_at: '2026-11-09T00:00:00Z',
    ...overrides,
  }
}

function renderPanel(items: LikerProfile[], next: string | null = null, onMore = vi.fn()) {
  render(
    <MemoryRouter>
      <IntroLikersPanel likers={items} nextCursor={next} loading={false} onMore={onMore} />
    </MemoryRouter>,
  )
  return onMore
}

describe('IntroLikersPanel', () => {
  // The acceptance case: tapping a row navigates directly to the profile.
  it('links each row straight to the profile', () => {
    renderPanel([liker()])
    const link = screen.getByRole('link')
    expect(link.getAttribute('href')).toBe('/profiles/00000000-0000-4000-8000-0000000000bb')
    // One tap: the link wraps the whole row, so there is no intermediate screen.
    expect(link.className).toContain('intro-liker-row')
    expect(link.textContent).toContain('Mina')
  })

  it('shows the nickname, shared interests and interaction style', () => {
    renderPanel([liker()])
    expect(screen.getByText('Mina')).toBeTruthy()
    expect(screen.getByText('You both love My Chemical Romance.')).toBeTruthy()
    expect(screen.getByText('Slow replies, long messages')).toBeTruthy()
  })

  // The acceptance case: the shared-interest line is omitted when there is no
  // overlap.
  it('omits the shared-interest line when there is no overlap', () => {
    expect(sharedInterestLine([])).toBeNull()
    renderPanel([liker({ shared_with_author: [] })])
    expect(screen.queryByText(/You both love/)).toBeNull()
    expect(document.querySelector('.intro-liker-shared')).toBeNull()
    // The row itself is still there.
    expect(screen.getByText('Mina')).toBeTruthy()
  })

  it('omits the interaction style when the profile did not set one', () => {
    renderPanel([liker({ interaction_style: null })])
    expect(document.querySelector('.intro-liker-style')).toBeNull()
  })

  it('paginates', () => {
    const onMore = renderPanel([liker()], 'cursor-1')
    fireEvent.click(screen.getByRole('button', { name: 'More people' }))
    expect(onMore).toHaveBeenCalledTimes(1)
  })

  it('hides the more button on the last page', () => {
    renderPanel([liker()], null)
    expect(screen.queryByRole('button', { name: 'More people' })).toBeNull()
  })

  it('reports an empty list', () => {
    renderPanel([])
    expect(screen.getByText('No hearts yet.')).toBeTruthy()
  })

  // No acceptance screen and no room screen in the path.
  it('offers no acceptance or room affordance', () => {
    renderPanel([liker()], 'cursor-1')
    const text = (document.body.textContent ?? '').toLowerCase()
    for (const forbidden of ['accept', 'approve', 'pending', 'decline', 'room']) {
      expect(text).not.toContain(forbidden)
    }
    // The only interactive elements are the row link and the pagination button.
    expect(screen.getAllByRole('button')).toHaveLength(1)
    expect(screen.getAllByRole('link')).toHaveLength(1)
  })
})
