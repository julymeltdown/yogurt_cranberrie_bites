// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { StrictMode } from 'react'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { RootMediaProvider, useRootMediaOwner } from '../../provider'
import { RootGiftImage, RootPostImage } from '../../RootMediaView'
import type { MediaOwner } from '../../index'
import type { MediaPreviewPort } from '../../../../features/event-feed'
import { bytes, deferred, eventId, id, mediaId, metadata, setup } from '../fixtures'
const stops: (() => void)[] = [], observers: { callback: IntersectionObserverCallback; elements: Set<Element> }[] = []
let created = vi.fn((_blob: Blob | MediaSource) => 'blob:unused'), revoked = vi.fn((_url: string) => undefined)
beforeEach(() => {
  vi.stubGlobal('location', { origin: 'https://mcr.example' })
  created = vi.fn((_blob: Blob | MediaSource) => `blob:root-${created.mock.calls.length}`); revoked = vi.fn((_url: string) => undefined)
  class FixtureURL extends URL { static createObjectURL = created; static revokeObjectURL = revoked }
  vi.stubGlobal('URL', FixtureURL)
  vi.stubGlobal('IntersectionObserver', class {
    value: typeof observers[number]
    constructor(callback: IntersectionObserverCallback) { this.value = { callback, elements: new Set() }; observers.push(this.value) }
    observe(element: Element) { this.value.elements.add(element) }
    disconnect() { this.value.elements.clear() }
  })
})
afterEach(() => { cleanup(); stops.splice(0).reverse().forEach(stop => stop()); observers.length = 0; vi.unstubAllGlobals() })
function harness(...args: Parameters<typeof setup>) { const h = setup(...args); stops.push(h.cleanup); return h }
function near(element: Element, isIntersecting = true) {
  for (const observer of observers) if (observer.elements.has(element)) observer.callback([{ target: element, isIntersecting } as IntersectionObserverEntry], {} as IntersectionObserver)
}
function revealAll() { act(() => { document.querySelectorAll('.root-media-view').forEach(element => near(element)) }) }
function Probe({ seen }: { seen: MediaOwner[] }) { const binding = useRootMediaOwner(); if (binding && !seen.includes(binding.owner)) seen.push(binding.owner); return null }
it('StrictMode keeps one live owner per full scope value; rows release on exit and remount warm without reads', async () => {
  const h = harness(), seen: MediaOwner[] = []
  const view = (visible = true) => <StrictMode><RootMediaProvider auth={h.auth} scope={{ ...h.scope }}><Probe seen={seen} />{visible && <RootPostImage auth={h.auth} scope={{ ...h.scope }} mediaId={mediaId} eventId={eventId} alt="Post photo" />}</RootMediaProvider></StrictMode>
  const r = render(view()); expect(h.fetch).not.toHaveBeenCalled(); revealAll()
  const image = await screen.findByRole('img', { name: 'Post photo' }), url = image.getAttribute('src')
  const owner = seen.at(-1)!; expect(owner.inspect()).toMatchObject({ entries: 1, consumers: 1, maxEntries: 8, maxConsumers: 32 })
  r.rerender(view()); expect(seen.at(-1)).toBe(owner); expect(h.fetch).toHaveBeenCalledTimes(2)
  act(() => near(document.querySelector('.root-media-view')!, false)); expect(screen.queryByRole('img')).not.toBeInTheDocument()
  expect(owner.inspect()).toMatchObject({ entries: 1, consumers: 0 }); expect(revoked).not.toHaveBeenCalled()
  r.rerender(view(false)); r.rerender(view()); revealAll()
  expect(await screen.findByRole('img')).toHaveAttribute('src', url); expect(h.fetch).toHaveBeenCalledTimes(2)
  r.unmount(); expect(owner.inspect().disposed).toBe(true); expect(revoked).toHaveBeenCalledExactlyOnceWith(url)
  expect(seen.every(owner => owner.inspect().disposed)).toBe(true)
})
it('manual no-observer fallback does not prefetch below-fold rows', async () => {
  vi.stubGlobal('IntersectionObserver', undefined)
  const h = harness()
  render(<RootMediaProvider auth={h.auth} scope={h.scope}>{[3, 4, 5].map(n => <RootPostImage key={n} auth={h.auth} scope={h.scope} mediaId={id(n)} eventId={eventId} alt={`Photo ${n}`} />)}</RootMediaProvider>)
  expect(screen.getAllByRole('button', { name: 'Load image' })).toHaveLength(3); expect(h.fetch).not.toHaveBeenCalled()
  fireEvent.click(screen.getAllByRole('button', { name: 'Load image' })[0]!); await screen.findByRole('img', { name: 'Photo 3' })
  expect(h.fetch).toHaveBeenCalledTimes(2); expect(screen.getAllByRole('button', { name: 'Load image' })).toHaveLength(2)
})
it('explicit adapters remain usable without a default provider, defer until visible and dispose only their consumer', async () => {
  const h = harness(), state: ReturnType<MediaPreviewPort['read']> = { phase: 'ready', scope: h.scope, previewUrl: 'blob:explicit', media: { ...metadata(), purpose: 'POST' }, error: null }
  const port: MediaPreviewPort = { read: () => state, subscribe: () => () => undefined, load: vi.fn(async () => undefined), cancel: vi.fn(), dispose: vi.fn() }
  const factory = vi.fn(() => port)
  render(<RootPostImage auth={h.auth} scope={h.scope} mediaId={mediaId} eventId={eventId} alt="Explicit photo" createPreview={factory} />)
  expect(factory).not.toHaveBeenCalled(); revealAll(); expect(await screen.findByRole('img')).toHaveAttribute('src', 'blob:explicit')
  act(() => near(document.querySelector('.root-media-view')!, false)); expect(port.dispose).toHaveBeenCalledTimes(1); expect(h.fetch).not.toHaveBeenCalled()
})
it.each(['purpose', 'path', 'scope', 'url'] as const)('retains exact POST rendering guard for an explicit adapter with invalid %s', async kind => {
  const h = harness(), state: ReturnType<MediaPreviewPort['read']> = { phase: 'ready', scope: kind === 'scope' ? { ...h.scope, generation: 9 } : h.scope,
    previewUrl: kind === 'url' ? `/api/concert/events/${eventId}/media/${mediaId}/content` : 'blob:fixture',
    media: { media_id: mediaId, purpose: kind === 'purpose' ? 'GIVEAWAY' : 'POST', read_path: kind === 'path' ? 'https://other.example/image' : metadata().read_path, width: 1, height: 1 }, error: null }
  const factory = vi.fn(() => ({ read: () => state, subscribe: () => () => undefined, load: async () => undefined, cancel() {}, dispose() {} }))
  render(<RootPostImage auth={h.auth} scope={h.scope} mediaId={mediaId} eventId={eventId} alt="Invalid photo" createPreview={factory} />); revealAll()
  await waitFor(() => expect(factory).toHaveBeenCalledTimes(1)); expect(screen.queryByRole('img')).not.toBeInTheDocument()
})
it('uses GIVEAWAY validation and rejects stale GiftImageRequest scopes without a POST renderer', async () => {
  const h = harness({}, r => new URL(r.url).pathname.endsWith('/content') ? new Response(bytes(), { headers: { 'Content-Type': 'image/png' } }) : Response.json(metadata(mediaId, 'GIVEAWAY')))
  const r = render(<RootMediaProvider auth={h.auth} scope={h.scope}><RootGiftImage mediaId={mediaId} giveawayId={id(9)} scope={h.scope} alt="Gift photo" /></RootMediaProvider>); revealAll()
  expect(await screen.findByRole('img', { name: 'Gift photo' })).toHaveAttribute('src', expect.stringMatching(/^blob:/))
  r.rerender(<RootMediaProvider auth={h.auth} scope={h.scope}><RootGiftImage mediaId={mediaId} giveawayId={id(9)} scope={{ ...h.scope, policyGeneration: 9 }} alt="Gift photo" /></RootMediaProvider>)
  expect(screen.queryByRole('img')).not.toBeInTheDocument(); expect(h.fetch).toHaveBeenCalledTimes(2)
})
it('policy withdrawal removes DOM and all retained URLs, and old scopes cannot revive an owner', async () => {
  const h = harness(), seen: MediaOwner[] = []
  render(<RootMediaProvider auth={h.auth} scope={h.scope}><Probe seen={seen} /><RootPostImage auth={h.auth} scope={h.scope} mediaId={mediaId} eventId={eventId} alt="Private photo" /></RootMediaProvider>); revealAll(); await screen.findByRole('img')
  act(() => { h.privacy.changePolicy(h.scope, 'block'); expect(revoked).toHaveBeenCalledTimes(1); expect(seen.at(-1)!.inspect().entries).toBe(0) })
  expect(screen.queryByRole('img')).not.toBeInTheDocument(); expect(h.fetch).toHaveBeenCalledTimes(2)
})
it('out-of-view release aborts a pending read and rejects late protected bytes', async () => {
  const gate = deferred<Response>(), h = harness({}, r => new URL(r.url).pathname.endsWith('/content') ? gate.promise : Response.json(metadata()))
  render(<RootMediaProvider auth={h.auth} scope={h.scope}><RootPostImage auth={h.auth} scope={h.scope} mediaId={mediaId} eventId={eventId} alt="Late photo" /></RootMediaProvider>); revealAll()
  await waitFor(() => expect(h.fetch).toHaveBeenCalledTimes(2)); act(() => near(document.querySelector('.root-media-view')!, false))
  gate.resolve(new Response(bytes(), { headers: { 'Content-Type': 'image/png' } })); await act(async () => { await gate.promise })
  expect(created).not.toHaveBeenCalled(); expect(screen.queryByRole('img')).not.toBeInTheDocument()
})
it('bounds active reads and facades without throwing or hiding row text; retry succeeds after exit frees a slot', async () => {
  const h = harness(), seen: MediaOwner[] = []
  render(<RootMediaProvider auth={h.auth} scope={h.scope}><Probe seen={seen} />{Array.from({ length: 34 }, (_, n) => <article key={n}><p>Authorized row {n}</p><RootPostImage auth={h.auth} scope={h.scope} mediaId={id(10 + n)} eventId={eventId} alt={`Photo ${n}`} /></article>)}</RootMediaProvider>); revealAll()
  await waitFor(() => expect(screen.getAllByRole('img')).toHaveLength(8)); expect(h.fetch).toHaveBeenCalledTimes(16)
  expect(screen.getAllByRole('button', { name: 'Retry image read' })).toHaveLength(26); expect(screen.getByText('Authorized row 33')).toBeVisible()
  expect(seen.at(-1)!.inspect()).toMatchObject({ entries: 8, consumers: 32 })
  act(() => near(document.querySelector('.root-media-view')!, false)); fireEvent.click(screen.getAllByRole('button', { name: 'Retry image read' })[0]!)
  await screen.findByRole('img', { name: 'Photo 8' }); expect(h.fetch).toHaveBeenCalledTimes(18); expect(seen.at(-1)!.inspect().entries).toBe(8)
})
