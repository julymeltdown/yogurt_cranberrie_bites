import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { createSocialOwner } from '../owner'
import { socialRoute } from '../routes'
import { deferred, event, fixture, id, post, response, user } from './fixtures'
const stops: (() => void)[] = []
const setup = (handler?: Parameters<typeof fixture>[0]) => { const f = fixture(handler), owner = createSocialOwner(f.options); stops.push(() => { owner.dispose(); f.dispose() }); return { ...f, owner } }
beforeEach(() => vi.stubGlobal('location', { origin: 'https://mcr.example' }))
afterEach(() => { stops.splice(0).forEach(f => f()); vi.unstubAllGlobals() })
it('creates no replacement Auth/cache/world or network work before entering a panel', () => {
  const f = setup(); expect(f.owner.auth).toBe(f.auth); expect(f.requests).toHaveLength(0); expect(f.stats.mounts).toBe(0)
  expect(f.owner.thread.read().selectedId).toBeNull(); expect(f.owner.inbox.read().count.total).toBeNull()
})
it.each(['/feed', '/feed/new', `/feed/${post.post_id}`, `/feed/${post.post_id}/edit`, '/meet/people', '/meet/intro/edit', `/people/${user}`, '/me/age', '/notifications', '/me/requests'])('recognizes only the frozen social destination %s', path => { expect(socialRoute(path)).not.toBeNull() })
it.each(['/people', '/intro/abc', '/notifications?actor=someone', '//evil.example/feed', '/feed/%2e%2e', '/feed/not-a-uuid', '/admin', '/me/age/verify'])('rejects unknown/unsafe destination %s', path => { expect(socialRoute(path)).toBeNull() })
it('retains exact controller identities and a safe composer draft across panels', async () => {
  const f = setup(), thread = f.owner.thread, inbox = f.owner.inbox
  f.owner.enter('/feed/new'); thread.setPostText('Keep this draft'); f.owner.enter('/notifications')
  await vi.waitFor(() => expect(inbox.read().count.total).toBe(37)); f.owner.enter('/meet/people'); f.owner.enter('/feed/new')
  expect(f.owner.thread).toBe(thread); expect(f.owner.inbox).toBe(inbox); expect(thread.read().draft.text).toBe('Keep this draft')
  expect(inbox.read().unreadShown).toBe(1); expect(f.owner.canNavigate('/world')).toBe(true)
})
it('does not relabel old bootstrap/service decisions with a newer policy lease', () => {
  const f = setup(); f.setLease({ ...f.options.availability.read()!, scope: { ...f.scope, policyGeneration: f.scope.policyGeneration + 1 } })
  f.owner.enter('/feed'); expect(f.owner.feed.read().available).toBe(false); expect(f.requests).toHaveLength(0)
  f.setLease({ scope: f.scope, bootstrap: { ...f.bootstrap }, services: { feed: true } }); expect(f.owner.available('feed')).toBe(false)
})
it('absent producer and optional requests module fail closed without dispatch', () => {
  const f = setup(); f.setLease(null); f.owner.enter('/feed'); f.owner.enter('/me/requests')
  expect(f.requests).toHaveLength(0); expect(f.owner.requests()).toBeNull(); expect(f.owner.available('inbox')).toBe(false)
})
it.each(['block', 'consent-revoked', 'suspended'] as const)('synchronously withdraws all children on %s metadata', async reason => {
  const f = setup(); f.owner.enter('/feed/new'); f.owner.thread.setPostText('Private draft'); f.owner.enter('/notifications')
  await vi.waitFor(() => expect(f.owner.inbox.read().count.total).toBe(37))
  expect(f.owner.notifyPolicy({ scope: f.scope, reason })).toBe(true)
  expect(f.owner.isDisposed()).toBe(true); expect(f.owner.read().phase).toBe('withdrawn'); expect(f.owner.thread.read().draft.text).toBe(''); expect(f.owner.inbox.read().count.total).toBeNull()
  expect(f.listenerCount()).toBe(0); expect(f.owner.canNavigate('/world')).toBe(false)
})
it('rejects policy messages with another scope or secret payload and does not ingest them', () => {
  const f = setup(); expect(f.owner.notifyPolicy({ scope: { ...f.scope, userId: id(999) }, reason: 'block' })).toBe(false)
  expect(f.owner.notifyPolicy({ scope: f.scope, reason: 'block', payload: '@secret' } as never)).toBe(false); expect(f.owner.isDisposed()).toBe(false)
})
it('old scope cannot complete an age verification callback or create another owner', async () => {
  const f = setup(); f.auth.session.replace(1, { userId: id(999), eventId: event, accessToken: 'NEW_SCOPE' })
  expect(await f.owner.completeVerification(f.scope, { verification_id: id(1), state_token: 'do-not-log', provider_response_token: 'do-not-log' })).toBe(false)
  expect(() => createSocialOwner(f.options)).toThrow(); expect(f.requests).toHaveLength(0)
})
it('unknown post outcome blocks panel/back/target changes and preserves original command+draft', async () => {
  const f = setup(r => { if (r.method === 'POST') throw new TypeError('lost result'); return response(r) })
  f.owner.enter('/feed/new'); f.owner.thread.setPostText('Uncertain write'); await f.owner.thread.submitPost()
  const key = f.owner.thread.read().mutation.key; expect(key).toBeTruthy(); expect(f.owner.canNavigate('/world')).toBe(false)
  expect(f.owner.enter('/notifications')).toBe(false); expect(f.owner.navigate('/meet/people')).toBe(false)
  expect(f.owner.thread.read()).toMatchObject({ draft: { text: 'Uncertain write' }, mutation: { key, phase: 'uncertain' } })
  expect(f.owner.read()).toMatchObject({ path: '/feed/new', notice: 'command' }); expect(f.requests.filter(r => r.method === 'POST')).toHaveLength(1)
})
it('keeps a reply bound to its post when another direct link would discard it', async () => {
  const f = setup(); f.owner.enter(`/feed/${post.post_id}`); await vi.waitFor(() => expect(f.owner.thread.read().detail.phase).toBe('ready'))
  f.owner.thread.setReplyText('Retain this reply'); expect(f.owner.enter(`/feed/${id(999)}`)).toBe(false)
  expect(f.owner.thread.read().reply.text).toBe('Retain this reply'); expect(f.owner.read().notice).toBe('draft')
})
it('freshly authorizes a related post before scoped navigation; destination then rereads', async () => {
  const f = setup(); const link = { target_type: 'POST' as const, target_id: post.post_id }
  expect(await f.owner.openResource(link)).toBe(true); expect(f.navigations).toEqual([`/feed/${post.post_id}`])
  f.owner.enter(`/feed/${post.post_id}`); await vi.waitFor(() => expect(f.requests.filter(r => new URL(r.url).pathname.endsWith(`/posts/${post.post_id}`))).toHaveLength(2))
})
it('a delayed authorized target cannot navigate after panel change or privacy teardown', async () => {
  const late = deferred<Response>(); const f = setup(() => late.promise)
  f.owner.enter('/feed/new'); const opening = f.owner.openResource({ target_type: 'POST', target_id: post.post_id })
  await vi.waitFor(() => expect(f.requests).toHaveLength(1)); f.owner.enter('/me/age'); expect(await opening).toBe(false)
  late.resolve(Response.json({ post, comments: [], next_cursor: null })); expect(f.navigations).toEqual([])
})
it('denied related target is evicted from both cached feed and thread projections', async () => {
  let denied = false
  const link = { target_type: 'PARTY' as const, target_id: id(600) }, linked = { ...post, links: [link] }
  const f = setup(r => {
    const path = new URL(r.url).pathname
    if (path.endsWith('/feed')) return Response.json({ items: [linked], next_cursor: null })
    if (path.includes('/posts/')) return Response.json({ post: linked, comments: [], next_cursor: null })
    if (path.includes('/parties/')) { denied = true; return Response.json({ code: 'PERMISSION_DENIED', message: 'no cached secret', details: { reason: 'PERMISSION_DENIED' } }, { status: 403 }) }
    return response(r)
  })
  f.owner.enter('/feed'); await vi.waitFor(() => expect(f.owner.feed.read().feed.phase).toBe('ready'))
  f.owner.enter(`/feed/${post.post_id}`); await vi.waitFor(() => expect(f.owner.thread.read().detail.phase).toBe('ready'))
  expect(await f.owner.openResource(link)).toBe(false); expect(denied).toBe(true)
  expect(f.owner.feed.read().feed.data?.items[0]?.links).toEqual([]); expect(f.owner.thread.read().detail.data?.post.links).toEqual([])
})

it.each(['intent', 'entry'] as const)('later %s navigation cancels a delayed inbox target before it can override the route', async mode => {
  const late = deferred<Response>()
  const f = setup(r => new URL(r.url).pathname.includes('/posts/') ? late.promise : response(r))
  f.owner.enter('/notifications'); await vi.waitFor(() => expect(f.owner.inbox.read().phase).toBe('ready'))
  const opening = f.owner.inbox.open(id(90))
  await vi.waitFor(() => expect(f.owner.inbox.read().target.phase).toBe('checking'))
  if (mode === 'intent') f.owner.navigate('/meet/people'); else f.owner.enter('/meet/people')
  await opening; late.resolve(Response.json({ post, comments: [], next_cursor: null })); await Promise.resolve()
  expect(f.navigations).toEqual(mode === 'intent' ? ['/meet/people'] : [])
  expect(f.owner.inbox.read().target.phase).toBe('idle')
})
