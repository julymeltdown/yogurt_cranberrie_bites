// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { createSocialOwner } from '../../owner'
import { safeReturnTarget } from '../../../routes'
import { socialRoute } from '../../routes'
import type { SocialOptions } from '../../ports'
import type { ImageUploadPort } from '../../../../features/feed-thread'
import { deferred, fixture, id, ownIntro, post, response, user, event } from '../fixtures'

const stops: (() => void)[] = []
function setup(handler?: Parameters<typeof fixture>[0], extra: Pick<SocialOptions, 'media'> = {}) {
  const f = fixture(handler), owner = createSocialOwner({ ...f.options, ...extra })
  stops.push(() => { owner.dispose(); f.dispose() })
  return { ...f, owner }
}
afterEach(() => stops.splice(0).forEach(stop => stop()))

it.each(['/', '/legal/privacy', '/legal/terms', '/archive', '/login', '/signup', '/verify-email', '/reset-password', '/auth/callback', '/account/deleted', '/admin', '/admin/reports', '/admin/event', `/admin/users/${id(22)}`])(
  'permits the clean registered boundary %s without weakening participant return targets', path => {
    const f = setup()
    expect(safeReturnTarget(path, '')).toBe('')
    expect(socialRoute(path)).toBeNull()
    expect(f.owner.canNavigate(path)).toBe(true)
    expect(f.owner.navigate(path)).toBe(true)
    expect(f.navigations).toEqual([path])
    expect(f.owner.isDisposed()).toBe(false) // Actual router unmount, not a pure permission read, owns teardown.
    expect(f.requests).toHaveLength(0)
  },
)

it.each(['', 'https://example.invalid/', '//example.invalid/admin', '/admin/../world', '/legal/..', '/legal/%2e%2e', '/legal/privacy#private', '/legal/privacy?token=secret', '/admin/reports?actor=other', '/admin/unknown', '/admin/users/not-a-uuid', '/people/not-a-uuid', `/people/${user}?private=1`, '/feed/%2fadmin', '/world?token=secret', '/legal/pri\\vacy', '/legal/pri\nvacy', '/LEGAL/privacy', '/unknown'])('keeps unsafe, unknown and malformed private targets blocked: %s', path => {
  const f = setup()
  expect(f.owner.canNavigate(path)).toBe(false)
  expect(f.owner.navigate(path)).toBe(false)
  expect(f.owner.read().notice).toBe('unavailable')
  expect(f.navigations).toEqual([])
})

it('retains a composer across participant panels but blocks a lifetime exit until draft is restored', () => {
  const f = setup()
  f.owner.enter('/feed/new')
  f.owner.thread.setPostText('PRIVATE unsent post')
  expect(f.owner.navigate('/world')).toBe(true)
  for (const path of ['/legal/privacy', '/admin/reports']) {
    expect(f.owner.navigate(path)).toBe(false)
    expect(f.owner.read().notice).toBe('draft')
    expect(f.owner.thread.read().draft.text).toBe('PRIVATE unsent post')
  }
  f.owner.thread.setPostText('')
  expect(f.owner.canNavigate('/legal/privacy')).toBe(true)
  expect(f.owner.canNavigate('/admin/reports')).toBe(true)
})

it('protects target-specific reply/edit drafts when the shell would be destroyed', async () => {
  const f = setup()
  f.owner.enter(`/feed/${post.post_id}`)
  await vi.waitFor(() => expect(f.owner.thread.read().detail.phase).toBe('ready'))
  f.owner.thread.setReplyText('PRIVATE reply')
  expect(f.owner.navigate('/legal/privacy')).toBe(false)
  expect(f.owner.read().notice).toBe('draft')
  f.owner.thread.setReplyText('')
  expect(f.owner.threadPanel.beginPostEdit()).toBe(true)
  expect(f.owner.navigate('/admin/reports')).toBe(false)
  f.owner.thread.cancelPostEdit()
  expect(f.owner.canNavigate('/admin/reports')).toBe(true)
})

it('compares intro drafts to saved content and permits an exactly restored value', async () => {
  const f = setup()
  f.owner.enter('/discover/intro')
  await vi.waitFor(() => expect(f.owner.intro.read().saved).not.toBeNull())
  const initial = structuredClone(f.owner.intro.read().draft)
  expect(initial.text).toBe(ownIntro().text)
  expect(f.owner.canNavigate('/legal/privacy')).toBe(true)
  f.owner.intro.edit({ ...initial, text: 'PRIVATE changed intro' })
  expect(f.owner.navigate('/legal/privacy')).toBe(false)
  expect(f.owner.read().notice).toBe('draft')
  f.owner.intro.edit(initial)
  expect(f.owner.canNavigate('/legal/privacy')).toBe(true)
  expect(f.owner.canNavigate('/admin/reports')).toBe(true)
})

it('retains sending/uncertain original command identity across a refused boundary exit', async () => {
  const late = deferred<Response>()
  const f = setup(r => r.method === 'POST' ? late.promise : response(r))
  f.owner.enter('/feed/new')
  f.owner.thread.setPostText('PRIVATE uncertain post')
  const sending = f.owner.thread.submitPost()
  await vi.waitFor(() => expect(f.requests.filter(r => r.method === 'POST')).toHaveLength(1))
  const key = f.owner.thread.read().mutation.key
  expect(f.owner.navigate('/legal/privacy')).toBe(false)
  expect(f.owner.read().notice).toBe('command')
  late.resolve(new Response(null, { status: 503 }))
  await sending
  expect(f.owner.thread.read().mutation.phase).toBe('uncertain')
  expect(f.owner.navigate('/admin/reports')).toBe(false)
  expect(f.owner.thread.read().mutation.key).toBe(key)
  expect(f.owner.thread.read().draft.text).toBe('PRIVATE uncertain post')
  await f.owner.thread.submitPost()
  expect(f.requests.filter(r => r.method === 'POST')).toHaveLength(1)
  expect(f.navigations).toEqual([])
})

it('a genuine PENDING receipt keeps its original key and cannot bypass the boundary lock', async () => {
  const f = setup(r => r.method !== 'POST' ? response(r) : Response.json({ receipt_id: id(100), event_id: event, actor_id: user,
    operation_id: 'createConcertPost', idempotency_key: r.headers.get('Idempotency-Key'), state: 'PENDING', resource_id: null,
    resource_revision: null, failure_reason: null, created_at: '2026-11-04T00:00:00Z', expires_at: '2026-11-05T00:00:00Z', result_status: null }, { status: 202 }))
  f.owner.enter('/feed/new'); f.owner.thread.setPostText('Pending post')
  await f.owner.thread.submitPost()
  expect(f.owner.thread.read().mutation.phase).toBe('pending')
  const key = f.owner.thread.read().mutation.key
  expect(f.owner.navigate('/legal/privacy')).toBe(false)
  expect(f.owner.navigate('/admin/reports')).toBe(false)
  expect(f.owner.read().notice).toBe('command')
  await f.owner.thread.submitPost()
  expect(f.owner.thread.read().mutation.key).toBe(key)
  expect(f.requests.filter(r => r.method === 'POST')).toHaveLength(1)
})

it('protects a prepared media selection until explicitly cancelled, then disposes its owner', () => {
  let phase: 'prepared' | 'idle' = 'prepared'
  const dispose = vi.fn(), upload: ImageUploadPort = { read: () => ({ phase, scope: null, previewUrl: 'blob:synthetic-private', progress: null, attachment: null, error: null }),
    subscribe: () => () => {}, select: async () => {}, upload: async () => {}, recover: async () => {}, retry: async () => {}, cancel: () => { phase = 'idle' }, dispose }
  const f = setup(undefined, { media: { create: () => ({ upload, createPreview: () => { throw Error('Unused synthetic preview') }, dispose }) } })
  expect(f.owner.navigate('/legal/privacy')).toBe(false)
  expect(f.owner.read().notice).toBe('draft')
  upload.cancel()
  expect(f.owner.canNavigate('/legal/privacy')).toBe(true)
  f.owner.dispose()
  expect(dispose).toHaveBeenCalledOnce()
})

it.each(['identity', 'event', 'block', 'event-closed'] as const)('does not reopen a boundary or revive late private data after %s cleanup', async reason => {
  const late = deferred<Response>(), f = setup(r => new URL(r.url).pathname.endsWith('/feed') ? late.promise : response(r))
  f.owner.enter('/feed')
  await vi.waitFor(() => expect(f.requests.some(r => new URL(r.url).pathname.endsWith('/feed'))).toBe(true))
  if (reason === 'identity' || reason === 'event') f.auth.session.replace(f.auth.session.read().generation, { userId: reason === 'identity' ? id(999) : user, eventId: reason === 'event' ? id(999) : event, accessToken: 'NEXT_SYNTHETIC' })
  else f.auth.privacy.changePolicy(f.scope, reason)
  expect(f.owner.isDisposed()).toBe(true)
  expect(f.owner.canNavigate('/legal/privacy')).toBe(false)
  expect(f.owner.canNavigate('/admin/reports')).toBe(false)
  late.resolve(Response.json({ items: [post], next_cursor: null }))
  await new Promise(resolve => setTimeout(resolve, 0))
  expect(f.owner.feed.read().feed.data).toBeNull()
  expect(f.listenerCount()).toBe(0)
  expect(f.navigations).toEqual([])
})

it('router-owner disposal clears private children/subscriptions without disposing canonical Auth', async () => {
  const f = setup()
  f.owner.enter('/notifications')
  await vi.waitFor(() => expect(f.owner.inbox.read().count.total).toBe(37))
  f.owner.enter('/discover/intro')
  await vi.waitFor(() => expect(f.owner.intro.read().saved).not.toBeNull())
  expect(f.owner.canNavigate('/legal/privacy')).toBe(true)
  expect(f.listenerCount()).toBeGreaterThan(0)
  f.owner.dispose()
  expect(f.owner.inbox.read().count.total).toBeNull()
  expect(f.owner.intro.read().saved).toBeNull()
  expect(f.listenerCount()).toBe(0)
  expect(f.auth.privacy.isCurrent(f.scope)).toBe(true)
})
