import { act } from 'react'
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import selected from '../../../../../../contracts/fixtures/assets/selected-static/avatar-manifest.json'
import { FeedThread } from '../../../feed-thread/FeedThread'
import { createFeedThreadController } from '../../../feed-thread/controller'
import { parse, type Author, type Post } from '../../contract'
import { post, setup } from '../fixtures'

Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })
const disposers: (() => void)[] = []
afterEach(() => { cleanup(); disposers.splice(0).forEach(dispose => dispose()) })
async function opened(value: Post) {
  const base = setup(vi.fn(async () => Response.json({ post: value, comments: [], next_cursor: null })))
  const controller = createFeedThreadController({ auth: base.auth, access: base.access })
  disposers.push(() => { controller.dispose(); base.dispose() })
  controller.start()
  await act(async () => { controller.select(value.post_id) })
  return { ...base, controller }
}
const chosen = parse<{ items: Post[] }>('Domain_feed_FeedPage', { items: [{ ...post,
  author: { ...post.author, avatar: { revision: 1, config: selected.presets[2]!.config } },
}], next_cursor: null }).items[0]!

it('passes the exact public author to the supplied port and withdraws its output on privacy change', async () => {
  const value = await opened(chosen)
  // A contract probe only: the actual selected-art renderer is qualified in the
  // default-shell native fixture, never replaced by this unit-test marker.
  const renderAuthor = vi.fn((_author: Author) => <span role="img" aria-label="Renderer contract probe" />)
  render(<FeedThread controller={value.controller} onLink={vi.fn()} onEditPost={vi.fn()} onReport={vi.fn()} renderAuthor={renderAuthor} />)
  expect(renderAuthor).toHaveBeenCalledWith(chosen.author)
  expect(renderAuthor.mock.calls[0]![0].avatar.config).toEqual(selected.presets[2]!.config)
  expect(screen.getByRole('img', { name: 'Renderer contract probe' })).toBeDefined()
  expect(screen.queryByLabelText(`${chosen.author.display_name}: avatar unavailable`)).toBeNull()
  await act(async () => { value.auth.privacy.changePolicy(value.auth.privacy.capture(), 'block') })
  expect(screen.queryByRole('img', { name: 'Renderer contract probe' })).toBeNull()
})

it('preserves the honest initial when no approved author port is supplied', async () => {
  const value = await opened(post)
  render(<FeedThread controller={value.controller} onLink={vi.fn()} onEditPost={vi.fn()} onReport={vi.fn()} />)
  const fallback = screen.getByRole('img', { name: `${post.author.display_name}: avatar unavailable` })
  expect(fallback.textContent).toBe(Array.from(post.author.display_name.trim())[0]?.toLocaleUpperCase() ?? '?')
  expect(document.querySelector('.feed-portrait canvas')).toBeNull()
})
