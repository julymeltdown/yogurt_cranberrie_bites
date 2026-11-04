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

it('retains only authorized natural-ratio metadata across observer exit while removing the image consumer', async () => {
  const h = harness(), state: ReturnType<MediaPreviewPort['read']> = { phase: 'ready', scope: h.scope, previewUrl: 'blob:explicit', media: { ...metadata(), purpose: 'POST', width: 197, height: 67 }, error: null }
  const port: MediaPreviewPort = { read: () => state, subscribe: () => () => undefined, load: vi.fn(async () => undefined), cancel: vi.fn(), dispose: vi.fn() }
  const factory = vi.fn(() => port)
  render(<RootPostImage auth={h.auth} scope={h.scope} mediaId={mediaId} eventId={eventId} alt="Natural photo" createPreview={factory} />)
  const frame = document.querySelector<HTMLElement>('.root-media-view')!
  expect(frame.style.getPropertyValue('--root-media-natural-ratio')).toBe('')
  revealAll(); await screen.findByRole('img'); expect(frame.style.getPropertyValue('--root-media-natural-ratio')).toBe('197 / 67')
  act(() => near(frame, false)); expect(screen.queryByRole('img')).not.toBeInTheDocument(); expect(port.dispose).toHaveBeenCalledTimes(1)
  expect(frame.style.getPropertyValue('--root-media-natural-ratio')).toBe('197 / 67'); expect(frame).toHaveAttribute('data-root-media-layout', 'authorized')
  expect(h.fetch).not.toHaveBeenCalled(); expect(frame.querySelector('[src]')).toBeNull()
  act(() => near(frame)); await screen.findByRole('img'); expect(frame.style.getPropertyValue('--root-media-natural-ratio')).toBe('197 / 67')
})
it('clears retained natural geometry on policy withdrawal outside the observer margin', async () => {
  const h = harness(), state: ReturnType<MediaPreviewPort['read']> = { phase: 'ready', scope: h.scope, previewUrl: 'blob:explicit', media: { ...metadata(), purpose: 'POST', width: 197, height: 46 }, error: null }
  const port: MediaPreviewPort = { read: () => state, subscribe: () => () => undefined, load: async () => undefined, cancel: vi.fn(), dispose: vi.fn() }
  render(<RootPostImage auth={h.auth} scope={h.scope} mediaId={mediaId} eventId={eventId} alt="Private dimensions" createPreview={() => port} />)
  revealAll(); await screen.findByRole('img'); const frame = document.querySelector<HTMLElement>('.root-media-view')!
  act(() => near(frame, false)); expect(frame.style.getPropertyValue('--root-media-natural-ratio')).toBe('197 / 46')
  act(() => { h.privacy.changePolicy(h.scope, 'block') })
  expect(frame.style.getPropertyValue('--root-media-natural-ratio')).toBe(''); expect(frame).not.toHaveAttribute('data-root-media-layout'); expect(screen.queryByRole('img')).not.toBeInTheDocument()
})
it('never carries another media identity geometry into a replacement row', async () => {
  const h = harness(), state: ReturnType<MediaPreviewPort['read']> = { phase: 'ready', scope: h.scope, previewUrl: 'blob:explicit', media: { ...metadata(), purpose: 'POST', width: 197, height: 67 }, error: null }
  const port: MediaPreviewPort = { read: () => state, subscribe: () => () => undefined, load: async () => undefined, cancel() {}, dispose() {} }, factory = () => port
  const view = (selected: string) => <RootPostImage auth={h.auth} scope={h.scope} mediaId={selected} eventId={eventId} alt="Replacement dimensions" createPreview={factory} />
  const r = render(view(mediaId)); revealAll(); await screen.findByRole('img'); const frame = document.querySelector<HTMLElement>('.root-media-view')!
  act(() => near(frame, false)); r.rerender(view(id(4)))
  expect(frame.style.getPropertyValue('--root-media-natural-ratio')).toBe(''); expect(frame).not.toHaveAttribute('data-root-media-layout')
})

function mutablePreview(scope: ReturnType<typeof setup>['scope'], width = 197, height = 67) {
  // Preview state intentionally has only SessionScope fields. Full privacy remains
  // fenced by the row's PrivacyScope and its original privacy.isCurrent check.
  let state: ReturnType<MediaPreviewPort['read']> = { phase: 'ready',
    scope: { userId: scope.userId, eventId: scope.eventId, generation: scope.generation },
    media: { ...metadata(), purpose: 'POST', width, height }, previewUrl: 'blob:explicit', error: null }
  const listeners = new Set<() => void>()
  const port: MediaPreviewPort = { read: () => state, subscribe: listener => { listeners.add(listener); return () => { listeners.delete(listener) } },
    load: vi.fn(async () => undefined), cancel: vi.fn(), dispose: vi.fn() }
  return { port, update(next: Partial<typeof state>) { state = { ...state, ...next }; for (const listener of listeners) listener() } }
}
it('clears a replaced preview source outside the observer margin without acquiring or reading its replacement', async () => {
  const h = harness(), first = mutablePreview(h.scope), second = mutablePreview(h.scope, 120, 240)
  const before = vi.fn(() => first.port), after = vi.fn(() => second.port)
  const view = (factory: (selected: string) => MediaPreviewPort) => <RootPostImage auth={h.auth} scope={h.scope} mediaId={mediaId} eventId={eventId} alt="Source dimensions" createPreview={factory} />
  const r = render(view(before)); revealAll(); await screen.findByRole('img')
  const frame = document.querySelector<HTMLElement>('.root-media-view')!
  act(() => near(frame, false)); expect(frame.style.getPropertyValue('--root-media-natural-ratio')).toBe('197 / 67')
  r.rerender(view(after)); expect(frame.style.getPropertyValue('--root-media-natural-ratio')).toBe('')
  expect(frame).not.toHaveAttribute('data-root-media-layout'); expect(after).not.toHaveBeenCalled()
  expect(first.port.dispose).toHaveBeenCalledTimes(1); expect(second.port.load).not.toHaveBeenCalled()
  act(() => first.update({ media: { ...metadata(), purpose: 'POST', width: 999, height: 1 } }))
  expect(frame.style.getPropertyValue('--root-media-natural-ratio')).toBe('')
  act(() => near(frame)); await screen.findByRole('img'); expect(frame.style.getPropertyValue('--root-media-natural-ratio')).toBe('120 / 240')
  expect(second.port.load).toHaveBeenCalledTimes(1); expect(h.fetch).not.toHaveBeenCalled(); expect(created).not.toHaveBeenCalled(); expect(revoked).not.toHaveBeenCalled()
})
it.each(['failed', 'cancelled'] as const)('clears natural geometry on %s and authorizes new dimensions only after a retry', async phase => {
  const h = harness(), first = mutablePreview(h.scope), second = mutablePreview(h.scope, 120, 240)
  const factory = vi.fn().mockReturnValueOnce(first.port).mockReturnValue(second.port)
  render(<RootPostImage auth={h.auth} scope={h.scope} mediaId={mediaId} eventId={eventId} alt="Retry dimensions" createPreview={factory} />)
  revealAll(); await screen.findByRole('img'); const frame = document.querySelector<HTMLElement>('.root-media-view')!
  expect(frame.style.getPropertyValue('--root-media-natural-ratio')).toBe('197 / 67')
  act(() => first.update({ phase, media: null, previewUrl: null }))
  expect(screen.queryByRole('img')).not.toBeInTheDocument(); expect(frame.style.getPropertyValue('--root-media-natural-ratio')).toBe('')
  expect(frame).not.toHaveAttribute('data-root-media-layout')
  if (phase === 'failed') fireEvent.click(screen.getByRole('button', { name: 'Retry image read' }))
  else { act(() => near(frame, false)); act(() => near(frame)) }
  await screen.findByRole('img'); expect(frame.style.getPropertyValue('--root-media-natural-ratio')).toBe('120 / 240')
  expect(first.port.dispose).toHaveBeenCalledTimes(1); expect(factory).toHaveBeenCalledTimes(2)
  expect(h.fetch).not.toHaveBeenCalled(); expect(created).not.toHaveBeenCalled(); expect(revoked).not.toHaveBeenCalled()
})
it.each([[0, 67], [197, -1], [Number.NaN, 67], [197, Number.POSITIVE_INFINITY], [1.5, 67]])('does not retain invalid numeric geometry %s × %s', async (width, height) => {
  const h = harness(), preview = mutablePreview(h.scope, width, height)
  render(<RootPostImage auth={h.auth} scope={h.scope} mediaId={mediaId} eventId={eventId} alt="Invalid dimensions" createPreview={() => preview.port} />)
  revealAll(); await screen.findByRole('img'); const frame = document.querySelector<HTMLElement>('.root-media-view')!
  expect(frame.style.getPropertyValue('--root-media-natural-ratio')).toBe(''); expect(frame).not.toHaveAttribute('data-root-media-layout')
  act(() => near(frame, false)); expect(frame.style.getPropertyValue('--root-media-natural-ratio')).toBe('')
})
it('mounts safely with a disposed privacy owner and never acquires media or geometry', () => {
  const h = harness(), factory = vi.fn(() => mutablePreview(h.scope).port)
  act(() => h.privacy.dispose())
  render(<RootPostImage auth={h.auth} scope={h.scope} mediaId={mediaId} eventId={eventId} alt="Ended access" createPreview={factory} />)
  revealAll(); expect(screen.getByText('Image access unavailable.')).toBeInTheDocument()
  expect(factory).not.toHaveBeenCalled(); expect(h.fetch).not.toHaveBeenCalled()
  expect(document.querySelector('.root-media-view')).not.toHaveAttribute('data-root-media-layout')
})
