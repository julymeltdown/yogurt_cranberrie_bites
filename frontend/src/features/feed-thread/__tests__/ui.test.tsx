// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { createFeedThreadController } from '../controller'
import { FeedThread, PostComposer } from '../FeedThread'
import { EventFeed } from '../../event-feed/EventFeed'
import { post, setup } from '../../event-feed/__tests__/fixtures'
import { act, StrictMode } from 'react'
Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })
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

it('renders only scoped blob images and cancels media immediately on privacy invalidation', async () => {
  const { FeedImage } = await import('../../event-feed/FeedImage')
  const s = setup(vi.fn(async () => Response.json({}))); removers.push(s.dispose)
  const listeners = new Set<() => void>(), scope = s.auth.session.read()
  let state = { phase: 'ready' as 'ready' | 'cancelled', scope, previewUrl: 'blob:test-only', media: { media_id: post.media_ids[0] ?? '00000000-0000-4000-8000-000000000901', purpose: 'POST' as const, read_path: '', width: 64, height: 64 }, error: null }
  state.media.read_path = `/api/concert/events/${post.event_id}/media/${state.media.media_id}/content`
  const cancel = vi.fn(() => { state = { ...state, phase: 'cancelled' }; listeners.forEach(fn => fn()) }), dispose = vi.fn()
  const controller = { read: () => state, subscribe(fn: () => void) { listeners.add(fn); return () => { listeners.delete(fn) } }, load: async () => undefined, cancel, dispose }
  const rendered = render(<FeedImage createPreview={() => controller} privacy={s.auth.privacy} mediaId={state.media.media_id} eventId={post.event_id} alt="Post attachment" />)
  expect(screen.getByRole('img', { name: 'Post attachment' }).getAttribute('src')).toBe('blob:test-only')
  await act(async () => { s.auth.privacy.changePolicy(s.auth.privacy.capture(), 'block') })
  expect(screen.queryByRole('img')).toBeNull(); expect(cancel).toHaveBeenCalledOnce()
  rendered.unmount(); expect(dispose).toHaveBeenCalledOnce()
})

it('constructs a fresh preview lifetime after StrictMode cleanup', async () => {
  const { FeedImage } = await import('../../event-feed/FeedImage')
  const s = setup(vi.fn(async () => Response.json({}))); removers.push(s.dispose)
  const mediaId = '00000000-0000-4000-8000-000000000901', disposals: ReturnType<typeof vi.fn>[] = []
  const createPreview = vi.fn(() => {
    const dispose = vi.fn(); disposals.push(dispose)
    const state = { phase: 'ready' as const, scope: s.auth.session.read(), previewUrl: 'blob:strict-fixture', media: { media_id: mediaId, purpose: 'POST' as const, read_path: `/api/concert/events/${post.event_id}/media/${mediaId}/content`, width: 64, height: 64 }, error: null }
    return { read: () => state, subscribe: () => () => undefined, load: async () => { if (dispose.mock.calls.length) throw new Error('Disposed reuse') }, cancel: () => undefined, dispose }
  })
  const rendered = render(<StrictMode><FeedImage createPreview={createPreview} privacy={s.auth.privacy} mediaId={mediaId} eventId={post.event_id} alt="Strict attachment" /></StrictMode>)
  expect(createPreview).toHaveBeenCalledTimes(2); expect(disposals[0]).toHaveBeenCalledOnce()
  expect(screen.getByRole('img', { name: 'Strict attachment' })).toBeDefined()
  rendered.unmount(); expect(disposals[1]).toHaveBeenCalledOnce()
})
