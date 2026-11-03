import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { createFeedThreadController } from '../controller'
import { deferred, eventId, id, post, setup, userId } from '../../event-feed/__tests__/fixtures'
const comment = { comment_id: id(71), post_id: post.post_id, author: post.author, text: 'A test reply', created_at: post.created_at, revision: 1 }
const detail = () => ({ post, comments: [comment], next_cursor: null })
const cleanups: (() => void)[] = []
beforeEach(() => vi.stubGlobal('location', { origin: 'https://mcr.example' }))
afterEach(() => { cleanups.splice(0).forEach(f => f()); vi.unstubAllGlobals() })
function fixture(handler: (r: Request) => Response | Promise<Response>) {
  const fetch = vi.fn<typeof globalThis.fetch>(async input => handler(input as Request)), base = setup(fetch), controller = createFeedThreadController({ auth: base.auth, access: base.access })
  cleanups.push(() => { controller.dispose(); base.dispose() }); controller.start()
  return { ...base, controller, fetch }
}
async function opened(s: ReturnType<typeof fixture>) { s.controller.select(post.post_id); await vi.waitFor(() => expect(s.controller.read().detail.phase).toBe('ready')) }
const receipt = (operation: string, key: string, state = 'PENDING') => ({ receipt_id: id(100), event_id: eventId, actor_id: userId, operation_id: operation, idempotency_key: key, state, resource_id: state === 'SUCCEEDED' ? post.post_id : null, resource_revision: state === 'SUCCEEDED' ? 2 : null, failure_reason: null, created_at: '2026-11-03T00:00:00Z', expires_at: '2026-11-05T00:00:00Z', result_status: state === 'SUCCEEDED' ? 200 : null })
it('publishes through canonical domain transport once and clears draft only on decoded success', async () => {
  const pending = deferred<Response>(), writes: Request[] = []
  const s = fixture(r => { if (r.method === 'POST') { writes.push(r); return pending.promise } return Response.json(detail()) })
  s.controller.setPostText('My test-only post')
  const flight = s.controller.submitPost(); const repeated = await s.controller.submitPost()
  expect(repeated).toBe(false); await vi.waitFor(() => expect(writes).toHaveLength(1))
  expect(await writes[0]!.clone().json()).toEqual({ text: 'My test-only post', media_ids: [], links: [] })
  expect(writes[0]!.headers.get('Idempotency-Key')).toMatch(/^[0-9a-f-]{36}$/); expect(writes[0]!.headers.get('X-MCR-Intent')).toBe('browser')
  expect(s.controller.read().draft.text).toBe('My test-only post')
  pending.resolve(Response.json(post)); expect(await flight).toBe(true)
  expect(s.controller.read().draft.text).toBe(''); expect(s.controller.read().mutation.phase).toBe('succeeded')
})
it('retains key and draft on response loss, only reads status, and recovers confirmed receipt', async () => {
  let writes = 0, key = ''
  const s = fixture(r => {
    if (r.method === 'POST') { writes++; key = r.headers.get('Idempotency-Key')!; throw new Error('lost after commit') }
    const url = new URL(r.url)
    expect(url.searchParams.get('operation_id')).toBe('createConcertPost'); expect(url.searchParams.get('idempotency_key')).toBe(key)
    return Response.json(receipt('createConcertPost', key, 'SUCCEEDED'))
  })
  s.controller.setPostText('Retain this draft'); await s.controller.submitPost()
  expect(s.controller.read().mutation).toMatchObject({ phase: 'uncertain', key }); expect(s.controller.read().draft.text).toBe('Retain this draft')
  s.controller.setPostText('Must not replace uncertain command'); expect(s.controller.read().draft.text).toBe('Retain this draft')
  await s.controller.submitPost(); expect(writes).toBe(1)
  await s.controller.checkStatus(); expect(writes).toBe(1); expect(s.controller.read().mutation.phase).toBe('succeeded'); expect(s.controller.read().draft.text).toBe('')
})
it('pending receipt does not claim success and a foreign-actor receipt cannot clear the draft', async () => {
  let key = ''
  const s = fixture(r => {
    if (r.method === 'POST') { key = r.headers.get('Idempotency-Key')!; return Response.json(receipt('createConcertPost', key), { status: 202 }) }
    return Response.json({ ...receipt('createConcertPost', key, 'SUCCEEDED'), actor_id: id(999) })
  })
  s.controller.setPostText('Pending draft'); await s.controller.submitPost(); expect(s.controller.read().mutation.phase).toBe('pending')
  await s.controller.checkStatus(); expect(s.controller.read().mutation.phase).toBe('uncertain'); expect(s.controller.read().draft.text).toBe('Pending draft')
})
it('edit carries original revision, conflict retains text, delete carries query revision', async () => {
  const bodies: unknown[] = [], queries: string[] = []
  const s = fixture(async r => {
    if (r.method === 'PATCH') { bodies.push(await r.json()); return Response.json({ code: 'CONFLICT', message: 'raw private error', details: { reason: 'VERSION_CONFLICT' } }, { status: 409 }) }
    if (r.method === 'DELETE') { queries.push(new URL(r.url).search); return Response.json({ resource_id: post.post_id, revision: post.revision + 1, state: 'DELETED' }) }
    return Response.json(detail())
  })
  await opened(s); expect(s.controller.beginPostEdit()).toBe(true); s.controller.setPostText('Retained edit')
  await s.controller.submitPost(); expect(bodies).toEqual([{ expected_revision: post.revision, text: 'Retained edit', media_ids: post.media_ids, links: post.links }])
  expect(s.controller.read().mutation.failure).toBe('conflict'); expect(s.controller.read().draft.text).toBe('Retained edit')
  await s.controller.deletePost(); expect(queries).toEqual([`?expected_revision=${post.revision}`]); expect(s.controller.read().detail.data).toBeNull(); expect(s.controller.read().comments.data).toBeNull()
})
it('actual comment create/edit/delete and reaction paths use server-authoritative replies', async () => {
  const requests: { method: string; path: string; body: unknown }[] = []
  const s = fixture(async r => {
    if (r.method === 'GET') return Response.json(detail())
    const path = new URL(r.url).pathname; requests.push({ method: r.method, path, body: r.method === 'POST' || r.method === 'PATCH' ? await r.json() : null })
    if (path.endsWith('/reaction')) return Response.json({ post_id: post.post_id, reacted_by_me: !post.reacted_by_me, reaction_count: 5, revision: 2 })
    if (r.method === 'DELETE') return Response.json({ resource_id: comment.comment_id, revision: 2, state: 'DELETED' })
    return Response.json(comment)
  })
  await opened(s); s.controller.setReplyText('A new comment'); await s.controller.submitComment(); await vi.waitFor(() => expect(s.controller.read().detail.phase).toBe('ready'))
  s.controller.beginCommentEdit(comment.comment_id); s.controller.setReplyText('Edited comment'); await s.controller.submitComment(); await vi.waitFor(() => expect(s.controller.read().detail.phase).toBe('ready'))
  await s.controller.deleteComment(comment.comment_id); await vi.waitFor(() => expect(s.controller.read().detail.phase).toBe('ready')); await s.controller.react()
  expect(requests.map(r => r.method)).toEqual(['POST', 'PATCH', 'DELETE', post.reacted_by_me ? 'DELETE' : 'PUT'])
  expect(requests[0]!.body).toEqual({ text: 'A new comment', post_revision: post.revision }); expect(requests[1]!.body).toEqual({ text: 'Edited comment', expected_revision: 1 })
})
it('denied comment pagination clears parent and all cached replies', async () => {
  const s = fixture(r => r.url.includes('/comments') ? Response.json({}, { status: 403 }) : Response.json({ ...detail(), next_cursor: 'more' }))
  await opened(s); await s.controller.moreComments()
  expect(s.controller.read().detail.data).toBeNull(); expect(s.controller.read().comments.data).toBeNull(); expect(s.controller.read().detail.failure).toBe('denied')
})
it('privacy revocation synchronously drops drafts, pending writes and late response', async () => {
  const late = deferred<Response>(), s = fixture(() => late.promise)
  s.controller.setPostText('Private draft'); const flight = s.controller.submitPost(); await vi.waitFor(() => expect(s.fetch).toHaveBeenCalledTimes(1))
  s.auth.privacy.changePolicy(s.auth.privacy.capture(), 'block')
  expect(s.controller.read().draft.text).toBe(''); expect(s.controller.read().mutation.phase).toBe('idle')
  late.resolve(Response.json(post)); await flight; expect(s.controller.read().mutation.phase).toBe('idle')
})
it('read-only and non-owner paths cannot dispatch, oversized drafts fail locally', async () => {
  const s = fixture(() => Response.json({ ...detail(), post: { ...post, author: { ...post.author, user_id: id(888) } } }))
  await opened(s); expect(s.controller.beginPostEdit()).toBe(false); expect(await s.controller.deletePost()).toBe(false)
  s.controller.setPostText('x'.repeat(4001)); expect(await s.controller.submitPost()).toBe(false); expect(s.fetch).toHaveBeenCalledTimes(1)
  s.setAccess({ available: true, readOnly: true }); expect(await s.controller.submitPost()).toBe(false); expect(s.fetch).toHaveBeenCalledTimes(1)
})
it('accepts only scoped READY POST media and clears uploader on privacy invalidation', async () => {
  const base = setup(vi.fn(async () => Response.json(post))), cancel = vi.fn()
  const mediaId = id(901), ready = { media_id: mediaId, state: 'READY' as const, revision: 1, width: 64, height: 64, content_type: 'image/png' as const, purpose: 'POST' as const, read_path: `/api/concert/events/${eventId}/media/${mediaId}/content` }
  let attachment = ready, scope = base.auth.session.read()
  const media = { read: () => ({ phase: 'ready' as const, scope, previewUrl: null, progress: null, attachment, error: null }), subscribe: () => () => undefined, select: async () => undefined, upload: async () => undefined, recover: async () => undefined, retry: async () => undefined, cancel, dispose: () => undefined }
  const controller = createFeedThreadController({ auth: base.auth, access: base.access, media }); cleanups.push(() => { controller.dispose(); base.dispose() }); controller.start()
  attachment = { ...ready, read_path: `/api/concert/events/${id(999)}/media/${mediaId}/content` }
  expect(controller.attachReadyMedia()).toBe(false)
  attachment = ready; scope = { ...base.auth.session.read(), generation: -1 }; expect(controller.attachReadyMedia()).toBe(false)
  scope = base.auth.session.read(); expect(controller.attachReadyMedia()).toBe(true); expect(controller.read().draft.media_ids).toEqual([mediaId])
  controller.setPostText('Draft with attachment'); base.auth.privacy.changePolicy(base.auth.privacy.capture(), 'block')
  expect(controller.read().draft.media_ids).toEqual([]); expect(controller.read().draft.text).toBe(''); expect(cancel).toHaveBeenCalled()
})
it('retains an uncertain key across same-identity read-only changes', async () => {
  let writes = 0
  const s = fixture(() => { writes++; throw new Error('lost') })
  s.controller.setPostText('Preserve through closure'); await s.controller.submitPost()
  const key = s.controller.read().mutation.key
  s.setAccess({ available: true, readOnly: true })
  expect(s.controller.read().draft.text).toBe('Preserve through closure'); expect(s.controller.read().mutation).toMatchObject({ phase: 'uncertain', key })
  expect(await s.controller.submitPost()).toBe(false); expect(writes).toBe(1)
})

it('destination denial remains evicted from a delayed thread refresh', async () => {
  const target = { target_type: 'PARTY' as const, target_id: id(80) }, linked = { ...detail(), post: { ...post, links: [target] } }, late = deferred<Response>(); let calls = 0
  const s = fixture(() => ++calls === 1 ? Response.json(linked) : late.promise)
  await opened(s); expect(s.controller.read().detail.data?.post.links).toEqual([target])
  const flight = s.controller.loadPost(); await vi.waitFor(() => expect(calls).toBe(2)); s.controller.removeLink(target)
  late.resolve(Response.json(linked)); await flight
  expect(s.controller.read().detail.data?.post.links).toEqual([])
})
