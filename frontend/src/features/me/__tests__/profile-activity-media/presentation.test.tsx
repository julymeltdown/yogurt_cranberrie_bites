// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { useEffect } from 'react'
import { act, cleanup, render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { afterEach, expect, it, vi } from 'vitest'
import { MeHub } from '../../ui/MePages'
import { ProfileActivity, type ProfileActivityInput } from '../../ui/reference-profile/ProfileActivity'
import type { Post } from '../../../event-feed/contract'
import { activity } from '../reference-target/data'
import { eventId, participant, setup, userId } from '../fixtures'

const mediaA = '00000000-0000-4000-8000-000000000071', mediaB = '00000000-0000-4000-8000-000000000072'
const otherUser = '00000000-0000-4000-8000-000000000073', otherEvent = '00000000-0000-4000-8000-000000000074'
const disposers: (() => void)[] = []
afterEach(() => { cleanup(); disposers.splice(0).forEach(dispose => dispose()); vi.restoreAllMocks() })
function fixture() {
  const fetcher = vi.fn(async () => Response.json(participant())), h = setup(fetcher)
  disposers.push(h.dispose)
  const base = activity(h.auth.privacy.capture(), userId)
  const input: ProfileActivityInput = { ...base, page: { ...base.page,
    items: base.page.items.map((post, i) => ({ ...post, media_ids: i === 0 ? [mediaA, mediaB] : [mediaB] })),
  } }
  return { ...h, input, fetcher }
}
const preview = (post: Post) => <output data-media-id={post.media_ids[0]}>Protected thumbnail probe</output>

it('forwards only two schema-qualified subject posts to the presentation port, without starting any read', () => {
  const h = fixture(), own = h.input.page.items
  const input = { ...h.input, page: { ...h.input.page, items: [
    { ...own[0]!, post_id: '00000000-0000-4000-8000-000000000081', created_at: '2026-11-04T11:00:00Z', author: { ...own[0]!.author, user_id: otherUser }, text: 'Another author' },
    ...own,
    { ...own[0]!, post_id: '00000000-0000-4000-8000-000000000082', created_at: '2026-11-04T08:00:00Z', text: 'Outside partial two-row bound' },
  ] } }
  const renderer = vi.fn(preview)
  render(<MemoryRouter><ProfileActivity input={input} subject={userId} privacy={h.auth.privacy} renderPostMedia={renderer} /></MemoryRouter>)
  expect(renderer).toHaveBeenCalledTimes(2)
  expect(renderer.mock.calls.map(([post]) => post.post_id)).toEqual(own.map(post => post.post_id))
  expect(renderer.mock.calls[0]![0].media_ids).toEqual([mediaA, mediaB])
  expect(Object.isFrozen(renderer.mock.calls[0]![0])).toBe(true)
  expect(screen.getAllByRole('link', { name: /Posted to the feed/ })).toHaveLength(2)
  expect(screen.queryByText('Another author')).toBeNull()
  expect(screen.queryByText('Outside partial two-row bound')).toBeNull()
  expect(h.fetcher).not.toHaveBeenCalled()
})

it('preserves the feed icon with no renderer, and skips the renderer for a post without media', () => {
  const h = fixture(), renderer = vi.fn(preview)
  const input = { ...h.input, page: { ...h.input.page, items: h.input.page.items.map(post => ({ ...post, media_ids: [] })) } }
  const view = render(<MemoryRouter><ProfileActivity input={h.input} subject={userId} privacy={h.auth.privacy} /></MemoryRouter>)
  expect(view.container.querySelectorAll('.reference-activity__visual .concert-icon')).toHaveLength(2)
  view.rerender(<MemoryRouter><ProfileActivity input={input} subject={userId} privacy={h.auth.privacy} renderPostMedia={renderer} /></MemoryRouter>)
  expect(renderer).not.toHaveBeenCalled()
  expect(view.container.querySelectorAll('.reference-activity__visual .concert-icon')).toHaveLength(2)
  expect(screen.getAllByRole('link', { name: /Posted to the feed/ })).toHaveLength(2)
})

it.each(['null', 'undefined', 'false', 'throw'] as const)('keeps the icon and readable post when the media port returns %s', kind => {
  const h = fixture(), renderer = vi.fn(() => {
    if (kind === 'throw') throw Error('Do not expose a protected read error')
    return kind === 'false' ? false : kind === 'null' ? null : undefined
  })
  const view = render(<MemoryRouter><ProfileActivity input={h.input} subject={userId} privacy={h.auth.privacy} renderPostMedia={renderer} /></MemoryRouter>)
  expect(view.container.querySelectorAll('.reference-activity__visual .concert-icon')).toHaveLength(2)
  expect(screen.getAllByRole('link', { name: /Posted to the feed/ })).toHaveLength(2)
  expect(view.container.textContent).not.toContain('protected read error')
  expect(h.fetcher).not.toHaveBeenCalled()
})

it.each(['subject', 'event', 'private-field', 'media-id'] as const)('never invokes the renderer for an invalid %s page', kind => {
  const h = fixture(), renderer = vi.fn(preview)
  const input = { ...h.input, ...(kind === 'subject' ? { subjectUserId: otherUser } : {}), page: { ...h.input.page,
    items: h.input.page.items.map(post => ({ ...post, ...(kind === 'event' ? { event_id: otherEvent } : {}),
      ...(kind === 'private-field' ? { private_handle: 'NEVER_DISCLOSE' } : {}), ...(kind === 'media-id' ? { media_ids: ['https://untrusted.test/image'] } : {}),
    })),
  } }
  render(<MemoryRouter><ProfileActivity input={input} subject={userId} privacy={h.auth.privacy} renderPostMedia={renderer} /></MemoryRouter>)
  expect(renderer).not.toHaveBeenCalled()
  expect(screen.getByText('Recent activity is unavailable in this connection.')).toBeVisible()
  expect(document.body.textContent).not.toContain('NEVER_DISCLOSE')
})

it.each(['policy', 'identity', 'event', 'generation', 'signout'] as const)('unmounts stale media on %s change without a parent rerender or another read', change => {
  const h = fixture(), disposed = vi.fn()
  function Probe() { useEffect(() => disposed, []); return <output>Revocable media probe</output> }
  const renderer = vi.fn(() => <Probe />)
  render(<MemoryRouter><ProfileActivity input={h.input} subject={userId} privacy={h.auth.privacy} renderPostMedia={renderer} /></MemoryRouter>)
  expect(screen.getAllByText('Revocable media probe')).toHaveLength(2)
  const calls = renderer.mock.calls.length
  act(() => {
    if (change === 'policy') h.auth.privacy.changePolicy(h.auth.privacy.capture(), 'block')
    else h.auth.session.replace(h.auth.session.read().generation, change === 'signout' ? null : {
      userId: change === 'identity' ? otherUser : userId, eventId: change === 'event' ? otherEvent : eventId, accessToken: 'SYNTHETIC_ONLY_NEW_GENERATION',
    })
  })
  expect(screen.queryByText('Revocable media probe')).toBeNull()
  expect(screen.queryByRole('link', { name: /Posted to the feed/ })).toBeNull()
  expect(disposed).toHaveBeenCalledTimes(2)
  expect(renderer).toHaveBeenCalledTimes(calls)
  expect(h.fetcher).not.toHaveBeenCalled()
})

it('does not revive a stale captured page after policy revalidation', () => {
  const h = fixture(), renderer = vi.fn(preview)
  render(<MemoryRouter><ProfileActivity input={h.input} subject={userId} privacy={h.auth.privacy} renderPostMedia={renderer} /></MemoryRouter>)
  act(() => { h.auth.privacy.changePolicy(h.auth.privacy.capture(), 'consent-revoked') })
  const calls = renderer.mock.calls.length
  act(() => { h.auth.privacy.changePolicy(h.auth.privacy.capture(), 'revalidated') })
  expect(screen.queryByText('Protected thumbnail probe')).toBeNull()
  expect(renderer).toHaveBeenCalledTimes(calls)
})

it('forwards the optional MeHub port using saved authorized activity and leaves avatar rendering unchanged', async () => {
  const h = fixture(); await h.controller.load()
  const renderer = vi.fn(preview), avatar = vi.fn(() => <output>Existing avatar presentation</output>)
  const view = render(<MemoryRouter><MeHub controller={h.controller} auth={h.auth} activity={h.input} renderPostMedia={renderer} renderAvatar={avatar} /></MemoryRouter>)
  expect(renderer).toHaveBeenCalledTimes(2)
  expect(avatar).toHaveBeenCalledWith(h.controller.read().participant!.profile.avatar, `${participant().profile.display_name}'s avatar`)
  expect(view.container.querySelectorAll('.reference-activity__visual [data-media-id]')).toHaveLength(2)
  expect(screen.getByText('Existing avatar presentation')).toBeVisible()
  expect(h.fetcher).toHaveBeenCalledTimes(1)
  act(() => { h.auth.privacy.changePolicy(h.auth.privacy.capture(), 'consent-revoked') })
  expect(screen.queryByText('Protected thumbnail probe')).toBeNull()
  expect(screen.queryByText('Existing avatar presentation')).toBeNull()
  expect(h.fetcher).toHaveBeenCalledTimes(1)
})
