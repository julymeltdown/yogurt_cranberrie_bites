// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { act, cleanup, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../../../router'
import { RootMediaProvider } from '../../provider'
import { bytes } from '../fixtures'
import { authorizedPost, event, giftMedia, postMedia, rowFixture } from './rows-fixture'
const stops: (() => void)[] = []
beforeEach(() => {
  vi.stubGlobal('location', { origin: 'https://mcr.example' })
  class FixtureURL extends URL { static createObjectURL = () => 'blob:authorized-row'; static revokeObjectURL = () => undefined }
  vi.stubGlobal('URL', FixtureURL)
  vi.stubGlobal('IntersectionObserver', class { constructor(private callback: IntersectionObserverCallback) {} observe(target: Element) { this.callback([{ target, isIntersecting: true } as IntersectionObserverEntry], {} as IntersectionObserver) } disconnect() {} })
})
afterEach(() => { cleanup(); stops.splice(0).reverse().forEach(stop => stop()); vi.unstubAllGlobals() })
function setup(path: string, feed = true) {
  const f = rowFixture(async r => {
    const path = new URL(r.url).pathname, media = path.split('/').at(-1)
    return path.endsWith('/content') ? new Response(bytes(), { headers: { 'Content-Type': 'image/png' } }) : Response.json({ media_id: media, state: 'READY', revision: 1,
      width: 1, height: 1, content_type: 'image/png', purpose: media === giftMedia ? 'GIVEAWAY' : 'POST', read_path: `${path}/content` })
  }, feed)
  const router = createMemoryRouter(createAppRoutes(f.ports), { initialEntries: [path] })
  render(<RootMediaProvider auth={f.auth} scope={f.scope}><RouterProvider router={router} /></RootMediaProvider>)
  stops.push(() => { router.dispose(); f.dispose() }); return { ...f, router }
}
it('binds actual default shared-root feed/thread/Gift rows and retains POST across route changes', async () => {
  const f = setup('/feed'); await screen.findByRole('img', { name: 'Post attachment' })
  expect(screen.getByText(authorizedPost.text)).toBeVisible(); const host = document.querySelector('[data-world-host]')
  const reads = () => f.requests.filter(r => new URL(r.url).pathname.includes('/media/'))
  expect(reads()).toHaveLength(2)
  await act(() => f.router.navigate(`/feed/${authorizedPost.post_id}`)); await screen.findByRole('img', { name: 'Post attachment' }); expect(reads()).toHaveLength(2)
  await act(() => f.router.navigate('/gifts')); await screen.findByRole('img', { name: 'Authorized synthetic giveaway' }); expect(reads()).toHaveLength(4)
  expect(reads().map(r => new URL(r.url).pathname)).toEqual([`/api/concert/events/${event}/media/${postMedia}`, `/api/concert/events/${event}/media/${postMedia}/content`, `/api/concert/events/${event}/media/${giftMedia}`, `/api/concert/events/${event}/media/${giftMedia}/content`])
  await act(() => f.router.navigate('/feed')); await screen.findByRole('img', { name: 'Post attachment' }); expect(reads()).toHaveLength(4)
  expect(document.querySelector('[data-world-host]')).toBe(host); expect(f.stats.peakGames).toBe(1)
})
it('does not create feed readiness or request media when default feed availability is false', async () => {
  const f = setup('/feed', false)
  await screen.findByText('This section is unavailable for the current account or event.')
  expect(f.requests).toHaveLength(0); expect(screen.queryByRole('img')).not.toBeInTheDocument()
})
