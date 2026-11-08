// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest'
import { cleanup, render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { IntroPostCard, excerptOf, DEFAULT_EXCERPT } from '../IntroPostCard'
import { IntroPostDetail, RESTRICTED_PROMPT } from '../IntroPostDetail'
import type { IntroPostDetail as Detail } from '../contract'
import type { LikeTransport } from '../api/intro-likes'

afterEach(cleanup)

const transport: LikeTransport = {
  request: (async () => ({ data: { liked: true, like_count: 1 }, status: 200, headers: new Headers() })) as unknown as LikeTransport['request'],
}

function post(overrides: Partial<Detail> = {}): Detail {
  return {
    intro_id: '00000000-0000-4000-8000-0000000000aa',
    author: {
      profile_id: '00000000-0000-4000-8000-0000000000bb',
      nickname: 'Mina',
      profile_image_media_id: null,
    },
    body: 'I like sad songs and long walks.',
    favorites_snapshot: [
      { catalog_item_id: '00000000-0000-4000-8000-0000000000c1', label: 'My Chemical Romance', category: 'MUSIC', external_url: null },
    ],
    currently_seeking: null,
    interaction_style: null,
    attachments: [],
    like: { count: 3, liked_by_viewer: false },
    comment_count: 2,
    published_at: '2026-11-09T00:00:00Z',
    expires_at: null,
    ...overrides,
  }
}

function renderCard(value: Detail) {
  return render(
    <MemoryRouter>
      <IntroPostCard post={value} transport={transport} />
    </MemoryRouter>,
  )
}

describe('IntroPostCard', () => {
  it('shows the body excerpt, favorites, attachment count, heart and comment count', () => {
    renderCard(post())
    expect(screen.getByText('I like sad songs and long walks.')).toBeTruthy()
    expect(screen.getByText('My Chemical Romance')).toBeTruthy()
    expect(screen.getByText('2 comments')).toBeTruthy()
    expect(screen.getByText('0 photographs')).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Leave a heart' })).toBeTruthy()
  })

  it('truncates a long body', () => {
    const long = 'a'.repeat(400)
    expect(excerptOf(long, 10)).toBe(`${'a'.repeat(10)}\u2026`)
    expect(excerptOf('short', 10)).toBe('short')
    // The default excerpt is 180 characters.
    expect([...excerptOf(long, DEFAULT_EXCERPT)].length).toBe(181)
    renderCard(post({ body: long }))
    expect(screen.getByText(new RegExp(`^a{180}\u2026$`))).toBeTruthy()
  })

  it('links to the author profile', () => {
    renderCard(post())
    const link = screen.getByRole('link', { name: 'Mina' })
    expect(link.getAttribute('href')).toBe('/profiles/00000000-0000-4000-8000-0000000000bb')
  })

  // The acceptance case: no room preview anywhere.
  it('offers no room preview or room link', () => {
    renderCard(post())
    const text = (document.body.textContent ?? '').toLowerCase()
    expect(text).not.toContain('room')
    for (const link of screen.getAllByRole('link')) {
      expect(link.getAttribute('href')).not.toContain('room')
    }
  })
})

describe('IntroPostDetail', () => {
  function renderDetail(value: Detail | null, unavailable = false) {
    return render(
      <MemoryRouter>
        <IntroPostDetail post={value} comments={[]} transport={transport} unavailable={unavailable} />
      </MemoryRouter>,
    )
  }

  it('shows the full body, attachments, comments, heart and author link', () => {
    renderDetail(
      post({
        attachments: [
          {
            media_id: '00000000-0000-4000-8000-0000000000d1',
            content_type: 'image/png',
            width: 2,
            height: 2,
            revision: 1,
            read_path: '/api/media/00000000-0000-4000-8000-0000000000d1/content',
          },
        ],
      }),
    )
    expect(screen.getByText('I like sad songs and long walks.')).toBeTruthy()
    // The photograph is decorative (the post's text carries the meaning), so it
    // is presentational: assert it exists without claiming an accessible name.
    expect(document.querySelectorAll('.intro-detail-attachments img')).toHaveLength(1)
    expect(screen.getByRole('link', { name: 'Mina' })).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Leave a heart' })).toBeTruthy()
    expect(screen.getByRole('heading', { name: 'Comments' })).toBeTruthy()
  })

  // The acceptance case: a restricted post renders the neutral prompt, never
  // partial content.
  it('renders the neutral prompt for a restricted post', () => {
    renderDetail(post(), true)
    expect(screen.getByRole('status').textContent).toBe(RESTRICTED_PROMPT)
    // No partial content leaks.
    expect(screen.queryByText('I like sad songs and long walks.')).toBeNull()
    expect(screen.queryByText('My Chemical Romance')).toBeNull()
    expect(screen.queryByRole('button')).toBeNull()
  })

  it('renders the neutral prompt when there is no post at all', () => {
    renderDetail(null)
    expect(screen.getByRole('status').textContent).toBe(RESTRICTED_PROMPT)
  })

  it('offers no room preview or room link', () => {
    renderDetail(post())
    const text = (document.body.textContent ?? '').toLowerCase()
    expect(text).not.toContain('room')
    for (const link of screen.getAllByRole('link')) {
      expect(link.getAttribute('href')).not.toContain('room')
    }
  })

  it('renders a deleted comment as deleted without its body', () => {
    render(
      <MemoryRouter>
        <IntroPostDetail
          post={post()}
          comments={[
            {
              comment_id: '00000000-0000-4000-8000-0000000000e1',
              intro_id: '00000000-0000-4000-8000-0000000000aa',
              author: { profile_id: '00000000-0000-4000-8000-0000000000bb', nickname: 'Mina', profile_image_media_id: null },
              parent_comment_id: null,
              body: '',
              created_at: '2026-11-09T00:00:00Z',
              deleted: true,
            },
          ]}
          transport={transport}
        />
      </MemoryRouter>,
    )
    expect(screen.getByText('This comment was deleted.')).toBeTruthy()
  })
})

// The acceptance case: neither component imports anything from
// features/profile-room/. This is a source-level assertion, because an import
// boundary is a fact about the module graph rather than about a render.
describe('import boundary', () => {
  it('neither IntroPostCard nor IntroPostDetail imports features/profile-room', async () => {
    for (const module of ['../IntroPostCard?raw', '../IntroPostDetail?raw']) {
      const source = await import(module).then((value) => (value as { default: string }).default)
      // Strip comments: the modules legitimately *mention* the room while
      // explaining that they never touch it.
      const code = source
        .split('\n')
        .filter((line) => !/^\s*(\/\/|\*|\/\*)/.test(line))
        .join('\n')
      expect(code).not.toContain('profile-room')
      expect(code).not.toContain('ProfileRoom')
      expect(code).not.toContain('room_id')
      expect(code).not.toContain('roomId')
      // No import statement names a room module.
      for (const line of code.split('\n')) {
        if (/^\s*import\b/.test(line)) expect(line.toLowerCase()).not.toContain('room')
      }
    }
  })
})
