// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { createFeedThreadController } from '../controller'
import { FeedThread, PostComposer } from '../FeedThread'
import { EventFeed } from '../../event-feed/EventFeed'
import { post, setup } from '../../event-feed/__tests__/fixtures'
import { act } from 'react'
const removers: (() => void)[] = []
afterEach(() => { cleanup(); removers.splice(0).forEach(fn => fn()) })
it('renders content as text and routes report entry without interpreting markup', async () => {
  const s = setup(vi.fn(async () => Response.json({ post: { ...post, text: '<script>private()</script>' }, comments: [], next_cursor: null }))), controller = createFeedThreadController({ auth: s.auth, access: s.access })
  removers.push(() => { controller.dispose(); s.dispose() }); controller.start()
  await act(async () => { controller.select(post.post_id) })
  const report = vi.fn(); render(<FeedThread controller={controller} onLink={vi.fn()} onEditPost={vi.fn()} onReport={report} />)
  expect(screen.getByText('<script>private()</script>')).toBeDefined(); expect(document.querySelector('script')).toBeNull()
  fireEvent.click(screen.getByRole('button', { name: 'Report post' })); expect(report).toHaveBeenCalledWith({ target_type: 'POST', target_id: post.post_id })
  expect(screen.getByRole('textbox', { name: 'Comment text' })).toBeDefined()
})
it('retains composer text on definitive error and strips it immediately on identity change', async () => {
  const s = setup(vi.fn(async () => Response.json({ code: 'BAD', message: 'private-error-do-not-render', details: { reason: 'PERMISSION_DENIED' } }, { status: 403 }))), controller = createFeedThreadController({ auth: s.auth, access: s.access })
  removers.push(() => { controller.dispose(); s.dispose() }); controller.start()
  render(<PostComposer controller={controller} onPost={vi.fn()} />)
  fireEvent.change(screen.getByRole('textbox', { name: 'Post text' }), { target: { value: 'Keep my draft' } })
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Publish post' })) })
  expect((screen.getByRole('textbox', { name: 'Post text' }) as HTMLTextAreaElement).value).toBe('Keep my draft')
  expect(document.body.textContent).not.toContain('private-error-do-not-render')
  await act(async () => { s.auth.session.replace(s.auth.session.read().generation, null) })
  expect((screen.getByRole('textbox', { name: 'Post text' }) as HTMLTextAreaElement).value).toBe('')
})
it('read-only feed has no compose action and shows empty/notices states', async () => {
  const s = setup(vi.fn(async () => Response.json({ items: [], next_cursor: null }))); removers.push(s.dispose)
  s.setAccess({ available: true, readOnly: true }); await act(async () => { s.controller.start() })
  render(<EventFeed controller={s.controller} onPost={vi.fn()} onCompose={vi.fn()} onLink={vi.fn()} />)
  expect(screen.queryByRole('button', { name: 'Write a post' })).toBeNull(); expect(screen.getByText('No posts yet.')).toBeDefined(); expect(screen.getByText('No published notices.')).toBeDefined()
})
