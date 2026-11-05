// @vitest-environment jsdom
import { act } from 'react'
import { afterEach, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { createFeedThreadController } from '../../controller'
import { FeedThread, PostComposer } from '../../FeedThread'
import { EventFeed, PostContent } from '../../../event-feed/EventFeed'
import { setup, post, comment, id } from './fixtures'

Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })
const disposers: (() => void)[] = []
afterEach(() => { cleanup(); disposers.splice(0).forEach(dispose => dispose()) })

async function thread(fetcher: typeof fetch) {
  const fixture = setup(fetcher)
  const controller = createFeedThreadController({ auth: fixture.auth, access: fixture.access })
  disposers.push(() => { controller.dispose(); fixture.dispose() })
  await act(async () => { controller.start(); controller.select(post.post_id) })
  await waitFor(() => expect(controller.read().detail.phase).toBe('ready'))
  return { ...fixture, controller }
}
const detail = () => Response.json({ post, comments: [comment], next_cursor: null })

it('keeps authorized author/media inputs exact and renders hostile and long text as text', () => {
  const author = vi.fn((value: typeof post.author) => <span>{value.display_name}</span>), media = vi.fn((mediaId: string) => <span>{mediaId}</span>)
  const text = '<script>private()</script>\n' + 'Long content '.repeat(50)
  render(<PostContent post={{ ...post, text, media_ids: [id(75)] }} onLink={vi.fn()} renderAuthor={author} renderMedia={media} />)
  expect(author).toHaveBeenCalledWith(post.author)
  expect(media.mock.calls[0]?.[0]).toBe(id(75))
  expect(document.querySelector('.feed-post-copy')?.textContent).toBe(text)
  expect(document.querySelector('script')).toBeNull()
})

it('uses existing Feed navigation callbacks without issuing a reaction or share mutation', async () => {
  const calls: string[] = []
  const fixture = setup(async (input, init) => {
    const request = input instanceof Request ? input : new Request(input, init)
    calls.push(request.method)
    return Response.json({ items: new URL(request.url).pathname.endsWith('/feed') ? [post] : [], next_cursor: null })
  })
  disposers.push(fixture.dispose)
  await act(async () => { fixture.controller.start() })
  await waitFor(() => expect(fixture.controller.read().feed.phase).toBe('ready'))
  const onPost = vi.fn(), onCompose = vi.fn()
  render(<EventFeed controller={fixture.controller} onPost={onPost} onCompose={onCompose} onLink={vi.fn()} />)
  fireEvent.click(screen.getByRole('button', { name: `View ${post.reaction_count} reactions in thread` }))
  fireEvent.click(screen.getByRole('button', { name: 'View post sharing in thread' }))
  fireEvent.click(screen.getByRole('button', { name: 'Write a post' }))
  expect(onPost.mock.calls).toEqual([[post.post_id], [post.post_id]])
  expect(onCompose).toHaveBeenCalledOnce()
  expect(calls.every(method => method === 'GET')).toBe(true)
})

it('preserves comment ownership, exact portrait inputs and focused local edit cancellation', async () => {
  const fixture = await thread(async () => detail())
  const renderAuthor = vi.fn((value: typeof post.author) => <span>{value.display_name}</span>), report = vi.fn()
  render(<FeedThread controller={fixture.controller} onLink={vi.fn()} onEditPost={vi.fn()} onReport={report} renderAuthor={renderAuthor} />)
  expect(renderAuthor.mock.calls.every(([author]) => author === post.author || author.user_id === post.author.user_id)).toBe(true)
  fireEvent.click(screen.getByRole('button', { name: 'Report comment' }))
  expect(report).toHaveBeenCalledWith({ target_type: 'COMMENT', target_id: comment.comment_id })
  fireEvent.click(screen.getByRole('button', { name: 'Edit comment' }))
  const input = screen.getByRole('textbox', { name: 'Edit comment text' }) as HTMLTextAreaElement
  expect(input.value).toBe(comment.text)
  expect(document.activeElement).toBe(input)
  fireEvent.change(input, { target: { value: 'Unsent edit' } })
  fireEvent.click(screen.getByRole('button', { name: 'Discard edit' }))
  expect(fixture.controller.read().reply).toEqual({ text: '', edit: null })
})

it('cancels deletion without dispatch and confirms exactly the current post revision', async () => {
  const writes: { method: string; path: string; revision: string | null }[] = []
  const fixture = await thread(async (input, init) => {
    const request = input instanceof Request ? input : new Request(input, init), url = new URL(request.url)
    if (request.method === 'GET') return detail()
    writes.push({ method: request.method, path: url.pathname, revision: url.searchParams.get('expected_revision') })
    return Response.json({ resource_id: post.post_id, revision: post.revision + 1, state: 'DELETED' })
  })
  render(<FeedThread controller={fixture.controller} onLink={vi.fn()} onEditPost={vi.fn()} onReport={vi.fn()} />)
  fireEvent.click(screen.getByRole('button', { name: 'Delete post' }))
  fireEvent.click(screen.getByRole('button', { name: 'Keep content' }))
  expect(writes).toEqual([])
  fireEvent.click(screen.getByRole('button', { name: 'Delete post' }))
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Confirm deletion' })) })
  expect(writes).toEqual([{ method: 'DELETE', path: `/api/concert/events/${post.event_id}/posts/${post.post_id}`, revision: String(post.revision) }])
  expect(screen.queryByRole('button', { name: 'Delete post' })).toBeNull()
})

it('retains a pending comment and locks repeat submission until explicit command status', async () => {
  let writes = 0
  const fixture = await thread(async (input, init) => {
    const request = input instanceof Request ? input : new Request(input, init)
    if (request.method === 'GET') return detail()
    writes++
    return Response.json({ receipt_id: id(801), event_id: post.event_id, actor_id: post.author.user_id,
      operation_id: 'createConcertComment', idempotency_key: request.headers.get('Idempotency-Key'), state: 'PENDING',
      resource_id: null, resource_revision: null, failure_reason: null, created_at: '2026-11-04T12:00:00Z',
      expires_at: '2026-11-05T12:00:00Z', result_status: null }, { status: 202 })
  })
  render(<FeedThread controller={fixture.controller} onLink={vi.fn()} onEditPost={vi.fn()} onReport={vi.fn()} />)
  fireEvent.change(screen.getByRole('textbox', { name: 'Comment text' }), { target: { value: 'Retain pending reply' } })
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Send comment' })) })
  expect(fixture.controller.read().mutation.phase).toBe('pending')
  expect((screen.getByRole('textbox', { name: 'Comment text' }) as HTMLTextAreaElement).value).toBe('Retain pending reply')
  expect((screen.getByRole('button', { name: 'Send comment' }) as HTMLButtonElement).disabled).toBe(true)
  fireEvent.click(screen.getByRole('button', { name: 'Send comment' }))
  expect(writes).toBe(1)
  expect(screen.getByRole('button', { name: 'Check command status' })).toBeDefined()
  expect(screen.queryByText('The server confirmed this change.')).toBeNull()
})

it('keeps composer drafts on failure and removes private text when canonical identity ends', async () => {
  const fixture = setup(async () => Response.json({ code: 'INVALID_ARGUMENT', message: 'Private backend detail', details: { reason: 'VALIDATION_FAILED' } }, { status: 400 }))
  const controller = createFeedThreadController({ auth: fixture.auth, access: fixture.access })
  disposers.push(() => { controller.dispose(); fixture.dispose() })
  await act(async () => { controller.start() })
  render(<PostComposer controller={controller} onPost={vi.fn()} />)
  fireEvent.change(screen.getByRole('textbox', { name: 'Post text' }), { target: { value: 'Keep this draft' } })
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Publish post' })) })
  expect((screen.getByRole('textbox', { name: 'Post text' }) as HTMLTextAreaElement).value).toBe('Keep this draft')
  expect(document.body.textContent).not.toContain('Private backend detail')
  expect(screen.queryByText('The server confirmed this change.')).toBeNull()
  await act(async () => { fixture.auth.session.replace(fixture.auth.session.read().generation, null) })
  expect((screen.getByRole('textbox', { name: 'Post text' }) as HTMLTextAreaElement).value).toBe('')
})
