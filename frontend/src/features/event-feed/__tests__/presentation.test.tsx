// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { EventFeed } from '../EventFeed'
import { relativePostTime } from '../presentation'
import { id, notice, post, setup } from './fixtures'
let f: ReturnType<typeof setup> | undefined
afterEach(() => { cleanup(); f?.dispose(); vi.restoreAllMocks() })
async function start() {
  const requests: URL[] = []
  f = setup(async input => {
    const url = new URL((input as Request).url); requests.push(url)
    if (url.pathname.endsWith(`/notices/${notice.notice_id}`)) return Response.json(notice)
    if (url.pathname.endsWith('/notices')) return Response.json({ items: [notice], next_cursor: null })
    const filter = url.searchParams.get('filter')
    const rows = [{ ...post, media_ids: [id(75)], links: [{ target_type: 'PARTY', target_id: id(80) }], reaction_count: 42, comment_count: 7 }]
    return Response.json({ items: filter === 'GIVEAWAY_LINK' ? [] : rows, next_cursor: null })
  })
  await act(async () => { f!.controller.start() })
  return requests
}
it('uses contract filters, local notices and preserves cached reads without tab refetches', async () => {
  const requests = await start()
  render(<EventFeed controller={f!.controller} onPost={vi.fn()} onCompose={vi.fn()} onLink={vi.fn()} />)
  expect(screen.getAllByRole('tab').map(t => t.textContent)).toEqual(['ALL', 'MEETUP', 'GIVEAWAY', 'NOTICE'])
  fireEvent.click(screen.getByRole('tab', { name: 'NOTICE' }))
  const before = requests.length
  expect(screen.getByRole('button', { name: notice.title })).toBeDefined()
  fireEvent.click(screen.getByRole('tab', { name: 'ALL' }))
  expect(requests).toHaveLength(before)
  await act(async () => { fireEvent.focus(screen.getByRole('tab', { name: 'MEETUP' })) })
  expect(f!.controller.read().filter).toBe('PARTY_LINK')
  await act(async () => { fireEvent.focus(screen.getByRole('tab', { name: 'GIVEAWAY' })) })
  expect(f!.controller.read().filter).toBe('GIVEAWAY_LINK')
  expect(screen.getByText('No posts yet.')).toBeDefined()
})
it('routes real counts to the thread, preserves linked validation callbacks and composes once', async () => {
  await start(); const thread = vi.fn(), compose = vi.fn(), link = vi.fn()
  render(<EventFeed controller={f!.controller} onPost={thread} onCompose={compose} onLink={link} />)
  fireEvent.click(screen.getByRole('button', { name: 'View 42 reactions in thread' }))
  fireEvent.click(screen.getByRole('button', { name: 'Open thread · 7 comments' }))
  fireEvent.click(screen.getByRole('button', { name: 'View post sharing in thread' }))
  expect(thread.mock.calls).toEqual([[post.post_id], [post.post_id], [post.post_id]])
  fireEvent.click(screen.getByRole('button', { name: 'Open party' }))
  expect(link).toHaveBeenCalledWith({ target_type: 'PARTY', target_id: id(80) })
  fireEvent.click(screen.getByRole('button', { name: 'Write a post' })); expect(compose).toHaveBeenCalledOnce()
  await act(async () => { f!.controller.removeLink({ target_type: 'PARTY', target_id: id(80) }) })
  expect(screen.queryByRole('button', { name: 'Open party' })).toBeNull()
})
it('passes exact public author/media values to caller renderers and scrubs them on canonical invalidation', async () => {
  await start(); const author = vi.fn(() => <span>Reviewed author seam</span>), media = vi.fn((mediaId: string) => <span data-media={mediaId}>Scoped media seam</span>)
  render(<EventFeed controller={f!.controller} onPost={vi.fn()} onCompose={vi.fn()} onLink={vi.fn()} renderAuthor={author} renderMedia={media} />)
  expect(author).toHaveBeenCalledWith(post.author)
  expect(media.mock.calls[0]?.[0]).toBe(id(75))
  expect(screen.getAllByText('Scoped media seam')).toHaveLength(1)
  await act(async () => { f!.auth.session.replace(f!.auth.session.read().generation, null) })
  expect(screen.queryByText('Reviewed author seam')).toBeNull()
  expect(screen.queryByText('Scoped media seam')).toBeNull()
  expect(screen.queryByRole('button', { name: 'Write a post' })).toBeNull()
})
it('displays an honest author placeholder, source-stamped notice and read-only controls', async () => {
  await start()
  render(<EventFeed controller={f!.controller} onPost={vi.fn()} onCompose={vi.fn()} onLink={vi.fn()} />)
  expect(screen.getByRole('img', { name: `${post.author.display_name}: avatar unavailable` })).toBeDefined()
  expect(document.querySelector('img[src],canvas')).toBeNull()
  fireEvent.click(screen.getByRole('tab', { name: 'NOTICE' }))
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: notice.title })) })
  expect(screen.getByRole('link', { name: 'Source' }).getAttribute('href')).toBe(notice.source_uri)
  await act(async () => { f!.setAccess({ available: true, readOnly: true }) })
  expect(screen.queryByRole('button', { name: 'Write a post' })).toBeNull()
  expect(screen.getByText('This event is read-only.')).toBeDefined()
})
it('keeps the media filter reachable without inventing a fifth main tab', async () => {
  await start()
  render(<EventFeed controller={f!.controller} onPost={vi.fn()} onCompose={vi.fn()} onLink={vi.fn()} />)
  fireEvent.click(screen.getByLabelText('Feed options'))
  await act(async () => { fireEvent.click(screen.getByRole('checkbox', { name: 'Images only' })) })
  await waitFor(() => expect(f!.controller.read().filter).toBe('WITH_MEDIA'))
  expect(screen.getAllByRole('tab')).toHaveLength(4)
})
it('formats truthful relative metadata, future-clock skew and absolute older dates', () => {
  const now = Date.parse('2026-11-04T03:00:00Z')
  expect(relativePostTime('2026-11-04T01:00:00Z', now)).toBe('2h ago')
  expect(relativePostTime('2026-11-04T04:00:00Z', now)).toBe('Just now')
  expect(relativePostTime('2026-11-04T02:58:00Z', now)).toBe('2m ago')
  expect(relativePostTime('2026-10-01T00:00:00Z', now)).toContain('2026')
  expect(relativePostTime('invalid', now)).toBe('Date unavailable')
})
