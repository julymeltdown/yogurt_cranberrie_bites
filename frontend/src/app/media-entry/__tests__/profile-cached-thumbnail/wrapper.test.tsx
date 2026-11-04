// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { act, cleanup, fireEvent, render } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import type { RootMediaBinding } from '../../provider'
import { RootProfilePostImage } from '../../RootProfilePostImage'
import { activity } from '../../../../features/me/__tests__/reference-target/data'
import { id, mediaId, setup } from '../fixtures'

const holder = vi.hoisted(() => ({ binding: null as RootMediaBinding | null }))
vi.mock('../../provider', async original => ({ ...await original<typeof import('../../provider')>(), useRootMediaOwner: () => holder.binding }))
const cleanups: (() => void)[] = []
afterEach(() => { cleanup(); cleanups.splice(0).forEach(dispose => dispose()); holder.binding = null })
async function fixture(ready = true) {
  const h = setup(); cleanups.push(h.cleanup)
  holder.binding = { auth: h.auth, scope: h.scope, owner: h.owner }
  if (ready) { const reader = h.owner.acquire({ mediaId, purpose: 'POST' }); await reader.load(); reader.release() }
  const source = activity(h.scope, h.scope.userId!).page.items[0]!, post = { ...source, media_ids: [mediaId] }
  return { ...h, post }
}
it('shows one inert cached image at bounded square geometry without another request', async () => {
  const h = await fixture(), view = render(<RootProfilePostImage auth={h.auth} scope={h.scope} post={h.post} />)
  const image = view.container.querySelector('img')!
  expect(image).toHaveAttribute('src', 'blob:media-owner-1'); expect(image).toHaveAttribute('alt', '')
  expect(image.style.objectFit).toBe('cover'); expect(image.style.maxInlineSize).toBe('52px')
  expect(view.container.querySelector('button,a,[tabindex]')).toBeNull(); expect(h.fetch).toHaveBeenCalledTimes(2)
  expect(h.owner.inspect()).toMatchObject({ consumers: 1, activeEntries: 1 })
  view.unmount(); expect(h.owner.inspect().consumers).toBe(0); expect(h.previews.revoke).not.toHaveBeenCalled()
})
it('keeps the generic icon when cold, without creating an entry or loading', async () => {
  const h = await fixture(false), view = render(<RootProfilePostImage auth={h.auth} scope={h.scope} post={h.post} />)
  expect(view.container.querySelector('img')).toBeNull(); expect(view.container.querySelector('.concert-icon')).not.toBeNull()
  expect(h.owner.inspect()).toMatchObject({ entries: 0, consumers: 0 }); expect(h.fetch).not.toHaveBeenCalled()
})
it('uses only the first media ID and never probes a cached second ID', async () => {
  const h = await fixture(), acquire = vi.spyOn(h.owner, 'acquireCached')
  const view = render(<RootProfilePostImage auth={h.auth} scope={h.scope} post={{ ...h.post, media_ids: [id(4), mediaId] }} />)
  expect(acquire).toHaveBeenCalledExactlyOnceWith({ mediaId: id(4), purpose: 'POST' })
  expect(view.container.querySelector('img')).toBeNull(); expect(h.fetch).toHaveBeenCalledTimes(2)
})
it('falls back without a media ID and when the exact auth or full scope does not match', async () => {
  const h = await fixture(), other = setup(); cleanups.push(other.cleanup)
  const view = render(<RootProfilePostImage auth={other.auth} scope={h.scope} post={h.post} />)
  expect(view.container.querySelector('img')).toBeNull()
  view.rerender(<RootProfilePostImage auth={h.auth} scope={{ ...h.scope, policyGeneration: h.scope.policyGeneration + 1 }} post={h.post} />)
  expect(view.container.querySelector('img')).toBeNull()
  view.rerender(<RootProfilePostImage auth={h.auth} scope={h.scope} post={{ ...h.post, media_ids: [] }} />)
  expect(view.container.querySelector('img')).toBeNull(); expect(h.owner.inspect().consumers).toBe(0); expect(h.fetch).toHaveBeenCalledTimes(2)
})
it('clears the old image immediately when a new post media ID is unavailable', async () => {
  const h = await fixture(), view = render(<RootProfilePostImage auth={h.auth} scope={h.scope} post={h.post} />)
  expect(view.container.querySelector('img')).not.toBeNull()
  view.rerender(<RootProfilePostImage auth={h.auth} scope={h.scope} post={{ ...h.post, media_ids: [id(4)] }} />)
  expect(view.container.querySelector('img')).toBeNull(); expect(h.owner.inspect().consumers).toBe(0); expect(h.fetch).toHaveBeenCalledTimes(2)
})
it('replaces decode failure with the ordinary icon and no retry action or request', async () => {
  const h = await fixture(), view = render(<RootProfilePostImage auth={h.auth} scope={h.scope} post={h.post} />)
  fireEvent.error(view.container.querySelector('img')!)
  expect(view.container.querySelector('img')).toBeNull(); expect(view.container.querySelector('.concert-icon')).not.toBeNull()
  expect(view.container.querySelector('button')).toBeNull(); expect(h.fetch).toHaveBeenCalledTimes(2)
})
it('removes private URL on policy change without a parent rerender', async () => {
  const h = await fixture(), view = render(<RootProfilePostImage auth={h.auth} scope={h.scope} post={h.post} />)
  expect(view.container.querySelector('img')).not.toBeNull()
  act(() => { h.privacy.changePolicy(h.scope, 'consent-revoked') })
  expect(view.container.querySelector('img')).toBeNull(); expect(h.previews.revoke).toHaveBeenCalledTimes(1)
  expect(h.owner.inspect()).toMatchObject({ disposed: true, consumers: 0 }); expect(h.fetch).toHaveBeenCalledTimes(2)
})
