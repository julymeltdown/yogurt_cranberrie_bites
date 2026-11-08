// @vitest-environment jsdom
// WP-06 T-507: the profile media gallery.
//
// The acceptance statements are:
//
// * a restricted intro attachment never appears here;
// * the empty state is distinguishable from "not available to you".
//
// The first is proved twice: the server cannot associate an `INTRO_POST` asset
// with a profile post, and the gallery refuses to render one even if a payload
// carried it. The second is proved by rendering both states and asserting they
// say different things.
import { readFileSync } from 'node:fs'
import { URL as FileURL } from 'node:url'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import {
  admitGalleryMedia,
  createGalleryReader,
  createGalleryStore,
  ProfileMediaGallery,
} from '../ProfileMediaGallery'
import { PROFILE, failure, json, media, post, setup, type Fixture } from './fixtures'

const cleanups: (() => void)[] = []
afterEach(() => { cleanups.splice(0).reverse().forEach(fn => fn()); cleanup() })

function gallery(f: Fixture) {
  const reader = createGalleryReader({ auth: f.auth, scope: f.scope, profileId: PROFILE })
  const store = createGalleryStore(reader)
  cleanups.push(() => store.dispose())
  return store
}

/** The default server: one page of posts. */
function server(f: Fixture, items: unknown[], next: string | null = null) {
  f.handler((_request, call) => {
    if (call.path === `/api/profiles/${PROFILE}/posts`) return json({ items, next_cursor: next })
    throw new Error(`unexpected ${call.path}`)
  })
}

describe('the gallery reads the profile post list', () => {
  it('reads /api/profiles/{id}/posts with no event segment', async () => {
    const f = await setup()
    cleanups.push(f.dispose)
    server(f, [post()])
    const store = gallery(f)
    await store.load()
    expect(f.calls.map(call => `${call.method} ${call.path}`)).toEqual([`GET /api/profiles/${PROFILE}/posts`])
    expect(store.read().posts).toHaveLength(1)
    expect(store.read().posts[0]?.post_id).toBe('00000000-0000-4000-8000-0000000009bb')
  })

  it('renders the posts with a preserved aspect ratio and lazy loading', async () => {
    const f = await setup()
    cleanups.push(f.dispose)
    server(f, [post()])
    const store = gallery(f)
    const { container } = render(<ProfileMediaGallery store={store} isAuthor={false} />)
    await waitFor(() => expect(screen.getByRole('heading', { name: 'Posts' })).toBeTruthy())
    const image = container.querySelector('img')
    expect(image).not.toBeNull()
    // 1200 x 800 is a 3:2 box, reserved before the image arrives.
    const figure = container.querySelector('figure')
    expect(figure?.getAttribute('style')).toContain('aspect-ratio: 1.5')
    expect(image?.getAttribute('loading')).toBe('lazy')
    expect(image?.getAttribute('decoding')).toBe('async')
    expect(image?.getAttribute('width')).toBe('1200')
    expect(image?.getAttribute('height')).toBe('800')
    // The image is contained, never cropped to a square.
    expect(image?.getAttribute('src')).toBe('/api/media/00000000-0000-4000-8000-0000000009aa/content')
  })

  it('offers a retry when an image fails to load', async () => {
    const f = await setup()
    cleanups.push(f.dispose)
    server(f, [post()])
    const store = gallery(f)
    const { container } = render(<ProfileMediaGallery store={store} isAuthor={false} />)
    await waitFor(() => expect(container.querySelector('img')).not.toBeNull())
    fireEvent.error(container.querySelector('img')!)
    expect(screen.getByRole('alert').textContent).toContain('could not be loaded')
    const retry = screen.getByRole('button', { name: 'Retry image' })
    fireEvent.click(retry)
    await waitFor(() => expect(container.querySelector('img')).not.toBeNull())
    expect(container.querySelector('img')?.getAttribute('src')).toContain('#retry-1')
  })

  it('shows a delete affordance only to the author', async () => {
    const f = await setup()
    cleanups.push(f.dispose)
    server(f, [post()])
    const store = gallery(f)
    const { unmount } = render(<ProfileMediaGallery store={store} isAuthor={false} />)
    await waitFor(() => expect(screen.getByRole('heading', { name: 'Posts' })).toBeTruthy())
    expect(screen.queryByRole('button', { name: 'Delete post' })).toBeNull()
    unmount()
    const authorStore = gallery(f)
    let deleted: string | null = null
    render(<ProfileMediaGallery
      store={authorStore}
      isAuthor
      onDelete={async value => { deleted = value.post_id }}
    />)
    await waitFor(() => expect(screen.getByRole('button', { name: 'Delete post' })).toBeTruthy())
    fireEvent.click(screen.getByRole('button', { name: 'Delete post' }))
    await waitFor(() => expect(deleted).toBe('00000000-0000-4000-8000-0000000009bb'))
  })

  it('paginates with the server cursor', async () => {
    const f = await setup()
    cleanups.push(f.dispose)
    let page = 0
    f.handler((_request, call) => {
      page += 1
      if (call.path !== `/api/profiles/${PROFILE}/posts`) throw new Error(call.path)
      return page === 1
        ? json({ items: [post()], next_cursor: 'next-page' })
        : json({ items: [post({ post_id: '00000000-0000-4000-8000-0000000009cc' })], next_cursor: null })
    })
    const store = gallery(f)
    render(<ProfileMediaGallery store={store} isAuthor={false} />)
    await waitFor(() => expect(screen.getByRole('button', { name: 'Show more posts' })).toBeTruthy())
    fireEvent.click(screen.getByRole('button', { name: 'Show more posts' }))
    await waitFor(() => expect(store.read().posts).toHaveLength(2))
    expect(screen.queryByRole('button', { name: 'Show more posts' })).toBeNull()
  })
})

describe('an intro attachment never appears', () => {
  it('refuses an INTRO_POST media item even if a payload carried one', async () => {
    const f = await setup()
    cleanups.push(f.dispose)
    // A malformed payload: a profile post whose media names an INTRO_POST
    // asset. The contract decoder accepts the purpose (it is a profile
    // purpose), so the gallery's own admission check is what must refuse it.
    server(f, [post({ media: [media({ purpose: 'INTRO_POST' })] })])
    const store = gallery(f)
    const { container } = render(<ProfileMediaGallery store={store} isAuthor={false} />)
    await waitFor(() => expect(screen.getByRole('heading', { name: 'Posts' })).toBeTruthy())
    expect(container.querySelector('img')).toBeNull()
    expect(screen.queryByText('a photograph')).toBeTruthy()
  })

  it('admits only a READY PROFILE_POST asset', () => {
    expect(admitGalleryMedia(media() as never)).toBe(true)
    for (const purpose of ['PROFILE_IMAGE', 'INTRO_POST', 'ROOM_POSTER']) {
      expect(admitGalleryMedia(media({ purpose }) as never)).toBe(false)
    }
    expect(admitGalleryMedia(media({ state: 'PENDING' }) as never)).toBe(false)
  })

  it('the gallery module never names an intro or a room', () => {
    const source = readFileSync(new FileURL('../ProfileMediaGallery.tsx', import.meta.url), 'utf8')
    const code = source.replace(/\/\*[\s\S]*?\*\//g, '').split('\n')
      .filter(line => !line.trimStart().startsWith('//')).join('\n')
    expect(code).not.toContain('/intro')
    expect(code).not.toContain('/room')
    expect(code).not.toContain('intro_posts')
  })
})

describe('the empty state is not a restriction', () => {
  it('says "No posts yet" rather than "not available"', async () => {
    const f = await setup()
    cleanups.push(f.dispose)
    server(f, [])
    const store = gallery(f)
    render(<ProfileMediaGallery store={store} isAuthor={false} />)
    await waitFor(() => expect(screen.getByRole('status').textContent).toContain('No posts yet'))
    expect(screen.getByRole('status').textContent).not.toContain('not available')
    // The author's empty state is about them, not about a viewer's access.
    cleanup()
    const authorStore = gallery(f)
    render(<ProfileMediaGallery store={authorStore} isAuthor />)
    await waitFor(() => expect(screen.getByRole('status').textContent).toContain('You have not posted'))
    expect(screen.getByRole('status').textContent).not.toContain('not available')
  })

  it('a 404 says "not available to you" and never shows an empty list', async () => {
    const f = await setup()
    cleanups.push(f.dispose)
    f.handler(() => failure(404, 'RESOURCE_UNAVAILABLE'))
    const store = gallery(f)
    render(<ProfileMediaGallery store={store} isAuthor={false} />)
    await waitFor(() => expect(screen.getByRole('status').textContent).toContain('not available to you'))
    expect(screen.getByRole('status').textContent).not.toContain('No posts yet')
    // The two states are distinguishable: the gallery reports the 404 as a
    // restriction, not as an empty profile.
    expect(store.read().unavailable).toBe(true)
  })

  it('a private post is simply absent, with no hint that it exists', async () => {
    const f = await setup()
    cleanups.push(f.dispose)
    // The server never returns the private post to a visitor; the gallery must
    // not invent a placeholder for it.
    server(f, [post()])
    const store = gallery(f)
    render(<ProfileMediaGallery store={store} isAuthor={false} />)
    await waitFor(() => expect(screen.getByRole('heading', { name: 'Posts' })).toBeTruthy())
    expect(screen.queryByText(/private/i)).toBeNull()
    expect(screen.queryByText(/Only you/)).toBeNull()
  })

  it('offers a retry when the page fails', async () => {
    const f = await setup()
    cleanups.push(f.dispose)
    let attempts = 0
    f.handler((_request, call) => {
      if (call.path !== `/api/profiles/${PROFILE}/posts`) throw new Error(call.path)
      attempts += 1
      return attempts === 1
        ? failure(503, 'DEPENDENCY_UNAVAILABLE')
        : json({ items: [post()], next_cursor: null })
    })
    const store = gallery(f)
    render(<ProfileMediaGallery store={store} isAuthor={false} />)
    await waitFor(() => expect(screen.getByRole('alert')).toBeTruthy())
    fireEvent.click(screen.getByRole('button', { name: 'Try again' }))
    await waitFor(() => expect(screen.getByRole('heading', { name: 'Posts' })).toBeTruthy())
  })
})
