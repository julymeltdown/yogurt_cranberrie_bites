// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { ActivityFeed } from '../ActivityFeed'
import {
  ACTIVITY_KINDS,
  activityLabel,
  activityPageSchema,
  activityTarget,
  decodeActivityPage,
  type ActivityItem,
} from '../contract'

afterEach(cleanup)

function item(overrides: Partial<ActivityItem> = {}): ActivityItem {
  return {
    activity_id: '00000000-0000-4000-8000-0000000000a1',
    kind: 'INTRO_LIKED',
    actor: {
      profile_id: '00000000-0000-4000-8000-0000000000bb',
      nickname: 'Mina',
      profile_image_media_id: null,
      shared_with_author: [],
      interaction_style: null,
    },
    intro_id: '00000000-0000-4000-8000-0000000000aa',
    comment_id: null,
    created_at: '2026-11-09T00:00:00Z',
    read: false,
    ...overrides,
  }
}

function renderFeed(items: ActivityItem[], next: string | null = null, onMore = vi.fn()) {
  render(
    <MemoryRouter>
      <ActivityFeed items={items} nextCursor={next} loading={false} onMore={onMore} />
    </MemoryRouter>,
  )
  return onMore
}

describe('ActivityFeed', () => {
  it('lists the four kinds of entry', () => {
    expect([...ACTIVITY_KINDS]).toEqual(['INTRO_LIKED', 'INTRO_COMMENT', 'INTRO_REPLY', 'FOLLOW'])
    renderFeed([
      item({ activity_id: '00000000-0000-4000-8000-0000000000a1', kind: 'INTRO_LIKED' }),
      item({ activity_id: '00000000-0000-4000-8000-0000000000a2', kind: 'INTRO_COMMENT' }),
      item({ activity_id: '00000000-0000-4000-8000-0000000000a3', kind: 'INTRO_REPLY' }),
      item({ activity_id: '00000000-0000-4000-8000-0000000000a4', kind: 'FOLLOW' }),
    ])
    expect(screen.getByText('Mina left a heart on your mutuals-intro post')).toBeTruthy()
    expect(screen.getByText('Mina commented on your mutuals-intro post')).toBeTruthy()
    expect(screen.getByText('Mina replied to your comment')).toBeTruthy()
    expect(screen.getByText('Mina followed you')).toBeTruthy()
  })

  // The acceptance case: a like entry opens the liker's profile.
  it('opens the liker profile from a like entry', () => {
    renderFeed([item({ kind: 'INTRO_LIKED' })])
    expect(activityTarget(item({ kind: 'INTRO_LIKED' }))).toBe(
      '/profiles/00000000-0000-4000-8000-0000000000bb',
    )
    const link = screen.getByRole('link')
    expect(link.getAttribute('href')).toBe('/profiles/00000000-0000-4000-8000-0000000000bb')
  })

  it('opens the post from a comment or reply entry', () => {
    for (const kind of ['INTRO_COMMENT', 'INTRO_REPLY'] as const) {
      expect(activityTarget(item({ kind }))).toBe(
        '/intro-posts/00000000-0000-4000-8000-0000000000aa',
      )
    }
    cleanup()
    renderFeed([item({ kind: 'INTRO_COMMENT' })])
    expect(screen.getByRole('link').getAttribute('href')).toBe(
      '/intro-posts/00000000-0000-4000-8000-0000000000aa',
    )
  })

  it('opens the follower profile from a follow entry', () => {
    renderFeed([item({ kind: 'FOLLOW' })])
    expect(screen.getByRole('link').getAttribute('href')).toBe(
      '/profiles/00000000-0000-4000-8000-0000000000bb',
    )
  })

  it('is one tap: the whole row is the link', () => {
    renderFeed([item()])
    const links = screen.getAllByRole('link')
    expect(links).toHaveLength(1)
    expect(links[0]!.className).toContain('activity-link')
    expect(links[0]!.textContent).toContain('Mina')
  })

  // The acceptance case: no entry contains an approval affordance.
  it('contains no approval affordance and no pending state', () => {
    renderFeed([
      item({ activity_id: '00000000-0000-4000-8000-0000000000a1', kind: 'INTRO_LIKED' }),
      item({ activity_id: '00000000-0000-4000-8000-0000000000a2', kind: 'FOLLOW' }),
      item({ activity_id: '00000000-0000-4000-8000-0000000000a3', kind: 'INTRO_COMMENT' }),
    ])
    // The only interactive element is the entry link: there is no button at all.
    expect(screen.queryAllByRole('button')).toHaveLength(0)
    const text = (document.body.textContent ?? '').toLowerCase()
    for (const forbidden of [
      'accept',
      'approve',
      'decline',
      'reject',
      'pending',
      'request',
      'deadline',
      'respond',
      'room',
    ]) {
      expect(text).not.toContain(forbidden)
    }
  })

  it('shows the shared-interest line only when there is overlap', () => {
    renderFeed([
      item({
        activity_id: '00000000-0000-4000-8000-0000000000a1',
        actor: {
          profile_id: '00000000-0000-4000-8000-0000000000bb',
          nickname: 'Mina',
          profile_image_media_id: null,
          shared_with_author: [
            {
              catalog_item_id: '00000000-0000-4000-8000-0000000000c1',
              label: 'My Chemical Romance',
              category: 'MUSIC',
              external_url: null,
            },
          ],
          interaction_style: 'Slow replies',
        },
      }),
      item({ activity_id: '00000000-0000-4000-8000-0000000000a2', kind: 'FOLLOW' }),
    ])
    expect(screen.getByText('You both love My Chemical Romance.')).toBeTruthy()
    expect(screen.getByText('Slow replies')).toBeTruthy()
    // Only the first entry has an overlap, so there is exactly one shared line.
    expect(document.querySelectorAll('.activity-shared')).toHaveLength(1)
  })

  it('paginates', () => {
    const onMore = renderFeed([item()], 'cursor-1')
    fireEvent.click(screen.getByRole('button', { name: 'More activity' }))
    expect(onMore).toHaveBeenCalledTimes(1)
  })

  it('reports an empty feed', () => {
    renderFeed([])
    expect(screen.getByRole('status').textContent).toBe('Nothing yet.')
  })

  // The schema is closed: a server that sent an approval state would fail to
  // decode rather than render one.
  it('refuses an activity item that carries a pending or approval field', () => {
    const base = {
      activity_id: '00000000-0000-4000-8000-0000000000a1',
      kind: 'INTRO_LIKED',
      actor: {
        profile_id: '00000000-0000-4000-8000-0000000000bb',
        nickname: 'Mina',
        profile_image_media_id: null,
        shared_with_author: [],
        interaction_style: null,
      },
      intro_id: null,
      comment_id: null,
      created_at: '2026-11-09T00:00:00Z',
      read: false,
    }
    expect(decodeActivityPage({ items: [base], next_cursor: null }).items).toHaveLength(1)
    for (const extra of [{ pending: true }, { accepted: true }, { state: 'PENDING' }, { deadline: '2026-12-01' }]) {
      expect(() => decodeActivityPage({ items: [{ ...base, ...extra }], next_cursor: null })).toThrow()
    }
    // The schema itself has no such field.
    const keys = Object.keys(activityPageSchema.shape)
    expect(keys).toEqual(['items', 'next_cursor'])
  })

  it('labels each kind in the product words', () => {
    for (const kind of ACTIVITY_KINDS) {
      const label = activityLabel(item({ kind }))
      expect(label).toContain('Mina')
      expect(label.toLowerCase()).not.toContain('request')
      expect(label.toLowerCase()).not.toContain('pending')
    }
  })
})
