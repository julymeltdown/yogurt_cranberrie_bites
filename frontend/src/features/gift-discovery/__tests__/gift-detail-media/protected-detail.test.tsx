// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import type { ReactNode } from 'react'
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { MemoryRouter, Route, Routes, useParams } from 'react-router-dom'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { GiftDetail, GiftDiscovery, type GiftImageRequest } from '../../ui/Gifts'
import { RootGiftImage } from '../../../../app/media-entry/RootMediaView'
import { RootMediaProvider } from '../../../../app/media-entry/provider'
import { bytes } from '../../../../app/media-entry/__tests__/fixtures'
import { deferred, eventId, fixture, giveaway, uid } from '../fixtures/data'

// Existing contract data and shared reader test bytes only: no product media mapping.
const mediaId = uid(52), giftId = giveaway().giveaway_id
const stops: (() => void)[] = []
let created = vi.fn((_blob: Blob | MediaSource) => 'blob:unused'), revoked = vi.fn((_url: string) => undefined)
beforeEach(() => {
  vi.stubGlobal('location', { origin: 'https://mcr.example' })
  vi.stubGlobal('IntersectionObserver', undefined)
  created = vi.fn((_blob: Blob | MediaSource) => `blob:gift-detail-${created.mock.calls.length}`)
  revoked = vi.fn((_url: string) => undefined)
  class FixtureURL extends URL { static createObjectURL = created; static revokeObjectURL = revoked }
  vi.stubGlobal('URL', FixtureURL)
})
afterEach(() => { cleanup(); stops.splice(0).reverse().forEach(stop => stop()); vi.unstubAllGlobals() })
const renderImage = (request: GiftImageRequest) => <RootGiftImage {...request} />
function harness(options: { image?: string | null; media?: (request: Request) => Response | Promise<Response>; detail?: (request: Request) => Response | Promise<Response> } = {}) {
  const gift = giveaway({ image_media_id: options.image === undefined ? mediaId : options.image })
  const h = fixture({ handler: request => {
    const path = new URL(request.url).pathname
    if (path.includes('/media/')) return options.media?.(request) ?? (path.endsWith('/content')
      ? new Response(bytes(), { headers: { 'Content-Type': 'image/png' } })
      : Response.json({ media_id: mediaId, state: 'READY', revision: 1, width: 1, height: 1, content_type: 'image/png', purpose: 'GIVEAWAY',
        read_path: `/api/concert/events/${eventId}/media/${mediaId}/content` }))
    if (path.endsWith('/me/giveaways')) return Response.json({ giving: { items: [], next_cursor: null }, receiving: { items: [], next_cursor: null } })
    if (path.endsWith('/giveaways')) return Response.json({ items: [gift], next_cursor: null })
    return options.detail?.(request) ?? Response.json(gift)
  } })
  stops.push(h.dispose)
  return { ...h, gift, mediaRequests: () => h.requests.filter(r => new URL(r.url).pathname.includes('/media/')) }
}
function detail(h: ReturnType<typeof harness>, port: ((request: GiftImageRequest) => ReactNode) = renderImage, id = giftId) {
  return <MemoryRouter><RootMediaProvider auth={h.auth} scope={h.auth.privacy.capture()}>
    <GiftDetail controller={h.controller} giveawayId={id} {...(port ? { renderGiftImage: port } : {})} />
  </RootMediaProvider></MemoryRouter>
}
it('renders explicit no-photo for null without asking the media port or reader', async () => {
  const h = harness({ image: null }), port = vi.fn(renderImage)
  render(detail(h, port))
  expect(await screen.findByText('No photo')).toBeVisible()
  expect(port).not.toHaveBeenCalled(); expect(h.mediaRequests()).toHaveLength(0)
  expect(screen.getByText('1 remaining')).toBeVisible()
  expect(screen.getByRole('link', { name: 'Review reservation' })).toHaveAttribute('href', `/gifts/${giftId}/reservation`)
  expect(screen.getByText('Your current reservation eligibility has not been confirmed.')).toBeVisible()
  expect(h.requests.every(r => r.method === 'GET')).toBe(true)
})
it.each(['absent', 'null-result'] as const)('distinguishes an attached image with %s renderer from no-photo', async mode => {
  const h = harness()
  render(mode === 'absent' ? <MemoryRouter><GiftDetail controller={h.controller} giveawayId={giftId} /></MemoryRouter>
    : detail(h, () => null))
  expect(await screen.findByText('Image access unavailable.')).toBeVisible()
  expect(screen.queryByText('No photo')).toBeNull(); expect(h.mediaRequests()).toHaveLength(0)
})
it('forwards only the authoritative attachment identity and the complete current privacy scope', async () => {
  const h = harness(), port = vi.fn((_request: GiftImageRequest) => <span>Protected image slot</span>)
  render(detail(h, port)); await screen.findByText('Protected image slot')
  expect(port).toHaveBeenCalledWith({ mediaId, giveawayId: giftId, scope: h.auth.privacy.capture(), alt: h.gift.title })
  expect(h.mediaRequests()).toHaveLength(0)
  expect(document.body.textContent).not.toContain('recipient_id')
})
it('uses the real Root provider for deferred protected reads and a blob image', async () => {
  const gate = deferred<Response>(), h = harness({ media: request => new URL(request.url).pathname.endsWith('/content') ? gate.promise
    : Response.json({ media_id: mediaId, state: 'READY', revision: 1, width: 1, height: 1, content_type: 'image/png', purpose: 'GIVEAWAY', read_path: `/api/concert/events/${eventId}/media/${mediaId}/content` }) })
  render(detail(h)); fireEvent.click(await screen.findByRole('button', { name: 'Load image' }))
  await waitFor(() => expect(h.mediaRequests()).toHaveLength(2))
  expect(screen.getByText('Loading image…')).toBeInTheDocument(); expect(screen.queryByRole('img')).toBeNull()
  gate.resolve(new Response(bytes(), { headers: { 'Content-Type': 'image/png' } }))
  expect(await screen.findByRole('img', { name: h.gift.title })).toHaveAttribute('src', 'blob:gift-detail-1')
  expect(h.mediaRequests().every(r => r.headers.get('Authorization') === 'Bearer synthetic-memory-token')).toBe(true)
})
it('preserves Root denial and retry without replacing a denied attachment with art or no-photo', async () => {
  const h = harness({ media: () => new Response(null, { status: 403 }) })
  render(detail(h)); fireEvent.click(await screen.findByRole('button', { name: 'Load image' }))
  expect(await screen.findByText('Image unavailable.')).toBeVisible()
  expect(screen.queryByRole('img')).toBeNull(); expect(screen.queryByText('No photo')).toBeNull()
  expect(screen.getByRole('heading', { name: h.gift.title })).toBeVisible()
  expect(h.mediaRequests()).toHaveLength(1)
  fireEvent.click(screen.getByRole('button', { name: 'Retry image read' }))
  await waitFor(() => expect(h.mediaRequests()).toHaveLength(2)); expect(created).not.toHaveBeenCalled()
})
it.each(['block', 'sign-out', 'event-change'] as const)('withdraws an already displayed protected photo on %s', async cause => {
  const h = harness(); render(detail(h)); fireEvent.click(await screen.findByRole('button', { name: 'Load image' }))
  const image = await screen.findByRole('img'), url = image.getAttribute('src')
  act(() => {
    if (cause === 'block') h.auth.privacy.changePolicy(h.auth.privacy.capture(), 'block')
    else h.auth.session.replace(h.auth.session.read().generation, cause === 'sign-out' ? null
      : { userId: h.auth.session.read().userId!, eventId: uid(2), accessToken: 'synthetic-next-event' })
  })
  expect(screen.queryByRole('img')).toBeNull(); expect(screen.queryByRole('group', { name: 'Giveaway photo' })).toBeNull()
  expect(revoked).toHaveBeenCalledExactlyOnceWith(url); expect(h.mediaRequests()).toHaveLength(2)
})
it('aborts a pending protected read and fences late bytes after privacy withdrawal', async () => {
  const gate = deferred<Response>(), h = harness({ media: request => new URL(request.url).pathname.endsWith('/content') ? gate.promise
    : Response.json({ media_id: mediaId, state: 'READY', revision: 1, width: 1, height: 1, content_type: 'image/png', purpose: 'GIVEAWAY', read_path: `/api/concert/events/${eventId}/media/${mediaId}/content` }) })
  render(detail(h)); fireEvent.click(await screen.findByRole('button', { name: 'Load image' }))
  await waitFor(() => expect(h.mediaRequests()).toHaveLength(2))
  act(() => { h.auth.privacy.changePolicy(h.auth.privacy.capture(), 'block') })
  expect(h.mediaRequests()[1]!.signal.aborted).toBe(true)
  await act(async () => { gate.resolve(new Response(bytes(), { headers: { 'Content-Type': 'image/png' } })); await gate.promise })
  expect(created).not.toHaveBeenCalled(); expect(screen.queryByRole('img')).toBeNull()
})
it('never reuses the previous giveaway photo while a different route detail is pending', async () => {
  const gate = deferred<Response>(), nextId = uid(99), h = harness({ detail: request => new URL(request.url).pathname.endsWith(nextId) ? gate.promise : Response.json(giveaway({ image_media_id: mediaId })) })
  const view = render(detail(h)); fireEvent.click(await screen.findByRole('button', { name: 'Load image' })); await screen.findByRole('img')
  view.rerender(detail(h, renderImage, nextId))
  expect(screen.queryByRole('img')).toBeNull(); expect(screen.queryByRole('group', { name: 'Giveaway photo' })).toBeNull()
  await act(async () => { gate.resolve(Response.json(giveaway({ giveaway_id: nextId, title: 'Next fixture item', image_media_id: null }))); await gate.promise })
  expect(await screen.findByText('No photo')).toBeVisible(); expect(screen.getByRole('heading', { name: 'Next fixture item' })).toBeVisible()
  expect(h.mediaRequests()).toHaveLength(2)
})
it('shares the protected cache through list to detail to list navigation without extra media requests', async () => {
  const h = harness()
  function DetailRoute() { return <GiftDetail controller={h.controller} giveawayId={useParams().giveawayId!} renderGiftImage={renderImage} /> }
  render(<MemoryRouter initialEntries={['/gifts']}><RootMediaProvider auth={h.auth} scope={h.auth.privacy.capture()}><Routes>
    <Route path="/gifts" element={<GiftDiscovery controller={h.controller} renderGiftImage={renderImage} />} />
    <Route path="/gifts/:giveawayId" element={<DetailRoute />} />
  </Routes></RootMediaProvider></MemoryRouter>)
  fireEvent.click(await screen.findByRole('button', { name: 'Load image' })); const image = await screen.findByRole('img'), url = image.getAttribute('src')
  fireEvent.click(screen.getByRole('link', { name: h.gift.title })); fireEvent.click(await screen.findByRole('button', { name: 'Load image' }))
  expect(await within(screen.getByRole('group', { name: 'Giveaway photo' })).findByRole('img')).toHaveAttribute('src', url)
  fireEvent.click(screen.getByRole('link', { name: '← All giveaways' })); fireEvent.click(await screen.findByRole('button', { name: 'Load image' }))
  expect(await screen.findByRole('img')).toHaveAttribute('src', url)
  expect(h.mediaRequests()).toHaveLength(2); expect(revoked).not.toHaveBeenCalled()
  expect(h.requests.every(r => r.method === 'GET')).toBe(true)
})
