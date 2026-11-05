/** PI02/03 synthetic default-App fixture derived from app/feed-entry/__tests__/fixture.ts
 * at f025bdb6. Explicit additions: canonical admission, selected-art text-only/long
 * rows, real decoded in-memory command responses and declared state modes.
 * No production module imports this file; mutations do not certify live persistence. */
import selected from '../../../../../../contracts/fixtures/assets/selected-static/avatar-manifest.json'
import postInput from '../../../../../../contracts/fixtures/domain/public-post.json'
import { authResponse, capabilities, eventId, userId, fixture, isolatedCookiePolicy } from '../../../auth/__tests__/fixtures/data'
import { enrollmentPaths } from '../../../auth/model/enrollment-contract'
import { canonicalJson } from '../../../auth/model/contract'
import { createSessionController } from '../../../auth/model/session'
import { parse, type Comment, type Notice, type Post, type Receipt } from '../../contract'
import { createFeedSocialPorts } from '../../../../app/feed-entry/production'

export type FixtureMode = 'loaded' | 'no-readiness' | 'denied-media' | 'thread' | 'notice' | 'long' | 'empty' | 'error' | 'read-only' | 'pending'
export const inputRevision = 'eb6e81bd7e5fbeb15613a45e00f1fe896e49f0da'
export const fixedNow = Date.parse('2026-11-04T12:00:00Z')
export const selectedPin = Object.freeze({ manifest_id: selected.manifest_id, manifest_version: selected.manifest_version, content_hash: selected.content_hash })
export const id = (tail: number) => `00000000-0000-4000-8000-${String(tail).padStart(12, '0')}`
export const mediaInputs = Object.freeze({
  [id(75)]: { path: '/reference-feed-media-v1/reference-card-1.png', sha256: 'a28db3162d4fe822b17d3228ad82866d4eeb0e2524fb606d469ccd925fd7385d', width: 197, height: 67 },
  [id(76)]: { path: '/reference-feed-media-v1/reference-card-2.png', sha256: '2c5863585bce002be0ea0b057f4927d67c6f99e97cc7f5edee6ba9c47589ffdd', width: 197, height: 46 },
})
export const rows = parse<{ items: readonly Post[]; next_cursor: null }>('Domain_feed_FeedPage', { items: [
  { ...postInput, event_id: eventId, post_id: id(92), author: { ...postInput.author, user_id: userId, display_name: 'Encore fan', avatar: { revision: 1, config: selected.presets[2]!.config } }, text: 'SEE YOU FOR THE ENCORE!\nAnyone swapping favourite songs?', created_at: new Date(fixedNow - 7_200_000).toISOString(), links: [], media_ids: [id(75)], reaction_count: 42, comment_count: 7 },
  { ...postInput, event_id: eventId, post_id: id(91), author: { ...postInput.author, display_name: 'Paper parade', avatar: { revision: 1, config: selected.presets[3]!.config } }, text: 'Trading memories from tonight.\nWhich moment are you keeping?', created_at: new Date(fixedNow - 14_400_000).toISOString(), links: [], media_ids: [id(76)], reaction_count: 27, comment_count: 5 },
], next_cursor: null }).items
// Existing declared test prose only. Bind IDs/authors to this fixture's canonical
// synthetic scope; do not import test runners or alter production inputs.
export const shapeProvenance = Object.freeze({
  revision: 'a413e69f6864d3a5578fb3149507921e96e9c5b5',
  comment: { path: 'frontend/src/features/feed-thread/__tests__/controller.test.ts', blob: '3e2215fc78f79558cb1de81162afe938e48d4a79', adaptation: 'Bind post/author/time to first selected-art synthetic row; retain reply text and revision.' },
  notice: { path: 'frontend/src/features/event-feed/__tests__/fixtures.ts', blob: 'f3e55881c846509f7c4cfd8eb7970c50273afec9', adaptation: 'Bind event_id to canonical synthetic Auth event; retain notice fields.' },
})
export const reply = parse<Comment>('Domain_feed_CommentPublic', { comment_id: id(71), post_id: rows[0]!.post_id,
  author: rows[0]!.author, text: 'A test reply', created_at: rows[0]!.created_at, revision: 1 })
export const notice = parse<Notice>('Domain_feed_PublishedNotice', { notice_id: id(60), event_id: eventId,
  title: 'Fixture notice', text: 'Test-only published notice.', source_uri: 'https://example.com/notice',
  confirmed_at: '2026-11-01T01:00:00Z', created_at: '2026-11-01T01:00:00Z', state: 'PUBLISHED', revision: 1 })
export const fixtureRoute = (mode: FixtureMode) => mode.startsWith('thread') ? `/feed/${rows[0]!.post_id}` : mode === 'notice' ? `/notices/${notice.notice_id}` : '/feed'

async function sha(value: BufferSource) {
  return [...new Uint8Array(await crypto.subtle.digest('SHA-256', value))].map(byte => byte.toString(16).padStart(2, '0')).join('')
}
/** Synthetic C03 remains coherent under the canonical decoder, including theme hashes. */
async function bootstrap() {
  const source = fixture(), avatar = { revision: 1, config: selected.presets[2]!.config }
  const themes = await Promise.all(source.theme_manifests.map(async theme => {
    const { content_hash: _oldHash, ...body } = theme
    const next = { ...body, avatar_manifest_id: selected.manifest_id, avatar_manifest_version: selected.manifest_version, avatar_content_hash: selected.content_hash }
    return { ...next, content_hash: `sha256:${await sha(new TextEncoder().encode(canonicalJson(next)))}` }
  }))
  return { ...source, avatar, participant: { ...source.participant, profile: { ...source.participant.profile, avatar } },
    avatar_manifest: selected, theme_manifests: themes,
    capabilities: { ...source.capabilities, feed: { available: true, reason: null }, mutations: { available: true, reason: null } } }
}

/** Explicit synthetic transport only; production.ts never imports this module. */
export async function createFeedFixture(mode: FixtureMode) {
  const originalBody = await bootstrap(), login = authResponse(), originalFetch = globalThis.fetch.bind(globalThis)
  const body = mode === 'read-only' ? { ...originalBody, event: { ...originalBody.event, phase: 'READ_ONLY' }, capabilities: { ...originalBody.capabilities, mutations: { available: false, reason: 'EVENT_READ_ONLY' } } } : originalBody
  let posts: Post[] = mode === 'empty' ? [] : rows.map(row => ({ ...row,
    ...(mode === 'long' ? { text: row.text + '\n' + 'A source-pinned long post remains readable across mobile widths. '.repeat(12), author: { ...row.author, display_name: 'Long authorized fixture display name' } } : {}) }))
  // Text-only anatomy is an explicit synthetic adaptation of the selected row.
  if (mode !== 'empty') posts.push(parse<Post>('Domain_feed_PostPublic', { ...rows[0]!, post_id: id(90), text: 'Anyone at the plaza after the show?',
    author: { ...rows[0]!.author, display_name: 'cemeterykids', avatar: { revision: 1, config: selected.presets[0]!.config } },
    media_ids: [], created_at: new Date(fixedNow - 18_000_000).toISOString(), reaction_count: 13, comment_count: 4 }))
  let comments: Comment[] = [reply], nextId = 900
  const receipts = new Map<string, Receipt>()
  const calls: { method: string; path: string; status: number }[] = []
  const denied = () => Response.json({ code: 'PERMISSION_DENIED', message: 'Synthetic media access denied', details: { reason: 'PERMISSION_DENIED' } }, { status: 403, headers: { 'Cache-Control': 'no-store' } })
  const unavailable = () => Response.json({ code: 'UNAVAILABLE', message: 'Synthetic dependency unavailable', details: { reason: 'DEPENDENCY_UNAVAILABLE' } }, { status: 503 })
  let disposed = false
  const transport: typeof fetch = async (input, init) => {
    const request = input instanceof Request ? input : new Request(input, init)
    const path = new URL(request.url).pathname
    const done = (response: Response) => { calls.push({ method: request.method, path, status: response.status }); return response }
    if (disposed || request.signal.aborted) throw new DOMException('Aborted', 'AbortError')
    if (path === '/api/auth/capabilities') return done(Response.json(capabilities()))
    if (path === '/api/auth/login' && request.method === 'POST') return done(Response.json(login))
    if (path === '/api/auth/logout' && request.method === 'POST') return done(new Response(null, { status: 204 }))
    if (request.headers.get('authorization') !== `Bearer ${login.accessToken}`) return done(denied())
    const base = `/api/concert/events/${eventId}`
    if (path === `${base}/bootstrap`) return done(Response.json(body))
    if (path === enrollmentPaths.admission && request.method === 'GET') return done(Response.json({
      decision: 'ADMITTED', policyRevision: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
      acceptedRevision: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa', serverTime: new Date(fixedNow).toISOString(),
    }))
    const post = posts.find(row => path === `${base}/posts/${row.post_id}`)
    const parent = posts.find(row => path === `${base}/posts/${row.post_id}/comments`)
    const comment = comments.find(row => path === `${base}/comments/${row.comment_id}`)
    const conflict = () => done(Response.json({ code: 'CONFLICT', message: 'Synthetic revision conflict', details: { reason: 'REVISION_CONFLICT' } }, { status: 409 }))
    if (request.method !== 'GET') {
      const data = request.method === 'DELETE' || request.method === 'PUT' ? {} : await request.json() as Record<string, unknown>
      if (request.method === 'POST' && parent) {
        if (data.post_revision !== parent.revision) return conflict()
        const created = parse<Comment>('Domain_feed_CommentPublic', { ...reply, comment_id: id(nextId++), post_id: parent.post_id,
          text: data.text, author: rows[0]!.author, created_at: new Date(fixedNow).toISOString() })
        comments.push(created)
        posts = posts.map(row => row.post_id === parent.post_id ? { ...row, comment_count: row.comment_count + 1, revision: row.revision + 1 } : row)
        if (mode === 'pending') {
          const key = request.headers.get('Idempotency-Key')!
          const receipt: Receipt = { receipt_id: id(nextId++), event_id: eventId, actor_id: login.userId, operation_id: 'createConcertComment',
            idempotency_key: key, state: 'PENDING', resource_id: created.comment_id, resource_revision: null, failure_reason: null,
            created_at: new Date(fixedNow).toISOString(), expires_at: new Date(fixedNow + 86_400_000).toISOString(), result_status: null }
          receipts.set(key, { ...receipt, state: 'SUCCEEDED', resource_revision: 1, result_status: 200 })
          return done(Response.json(receipt, { status: 202 }))
        }
        return done(Response.json(created))
      }
      if (request.method === 'POST' && path === `${base}/posts`) {
        const created = parse<Post>('Domain_feed_PostPublic', { ...rows[0]!, post_id: id(nextId++), text: data.text, media_ids: data.media_ids,
          links: data.links, created_at: new Date(fixedNow).toISOString(), reaction_count: 0, comment_count: 0, revision: 1 })
        posts.unshift(created)
        return done(Response.json(created))
      }
      if (post && request.method === 'PATCH') {
        if (data.expected_revision !== post.revision) return conflict()
        const edited = parse<Post>('Domain_feed_PostPublic', { ...post, text: data.text, media_ids: data.media_ids, links: data.links, revision: post.revision + 1,
          edited_at: new Date(fixedNow).toISOString() })
        posts = posts.map(row => row.post_id === edited.post_id ? edited : row)
        return done(Response.json(edited))
      }
      if (post && request.method === 'DELETE') {
        if (Number(new URL(request.url).searchParams.get('expected_revision')) !== post.revision) return conflict()
        posts = posts.filter(row => row.post_id !== post.post_id)
        return done(Response.json({ resource_id: post.post_id, revision: post.revision + 1, state: 'DELETED' }))
      }
      if (comment && request.method === 'PATCH') {
        if (data.expected_revision !== comment.revision) return conflict()
        const edited = parse<Comment>('Domain_feed_CommentPublic', { ...comment, text: data.text, revision: comment.revision + 1 })
        comments = comments.map(row => row.comment_id === comment.comment_id ? edited : row)
        return done(Response.json(edited))
      }
      if (comment && request.method === 'DELETE') {
        if (Number(new URL(request.url).searchParams.get('expected_revision')) !== comment.revision) return conflict()
        comments = comments.filter(row => row.comment_id !== comment.comment_id)
        posts = posts.map(row => row.post_id === comment.post_id ? { ...row, comment_count: Math.max(0, row.comment_count - 1), revision: row.revision + 1 } : row)
        return done(Response.json({ resource_id: comment.comment_id, revision: comment.revision + 1, state: 'DELETED' }))
      }
      const reaction = posts.find(row => path === `${base}/posts/${row.post_id}/reaction`)
      if (reaction && ['PUT', 'DELETE'].includes(request.method)) {
        const reacted = request.method === 'PUT', count = reaction.reaction_count + (reacted ? 1 : -1)
        posts = posts.map(row => row.post_id === reaction.post_id ? { ...row, reacted_by_me: reacted, reaction_count: count, revision: row.revision + 1 } : row)
        return done(Response.json({ post_id: reaction.post_id, reacted_by_me: reacted, reaction_count: count, revision: reaction.revision + 1 }))
      }
      return done(unavailable())
    }
    if (path === `${base}/commands`) {
      const receipt = receipts.get(new URL(request.url).searchParams.get('idempotency_key') ?? '')
      return done(receipt ? Response.json(receipt) : unavailable())
    }
    if (path === `${base}/feed`) {
      if (mode === 'error') return done(unavailable())
      const filter = new URL(request.url).searchParams.get('filter')
      return done(Response.json({ items: !filter || filter === 'ALL' ? posts : filter === 'WITH_MEDIA' ? posts.filter(row => row.media_ids.length > 0) : [], next_cursor: null }))
    }
    if (post) return done(Response.json({ post: post,
      comments: comments.filter(row => row.post_id === post.post_id), next_cursor: null }))
    if (mode === 'notice' && path === `${base}/notices/${notice.notice_id}`) return done(Response.json(notice))
    if (path === `${base}/notices`) return done(Response.json({ items: mode === 'notice' ? [notice] : [], next_cursor: null }))
    const match = /^\/media\/([^/]+)(\/content)?$/.exec(path.slice(base.length))
    const media = match?.[1] ? mediaInputs[match[1]] : undefined
    if (path.startsWith(base) && media && match) {
      if (mode === 'denied-media') return done(denied())
      if (!match[2]) return done(Response.json({ media_id: match[1], state: 'READY', revision: 1, width: media.width, height: media.height,
        content_type: 'image/png', purpose: 'POST', read_path: `${base}/media/${match[1]}/content` }))
      const response = await originalFetch(media.path, { signal: request.signal, cache: 'no-store' })
      if (!response.ok) throw new Error('Approved fixture media missing')
      const bytes = await response.arrayBuffer()
      if (await sha(bytes) !== media.sha256) throw new Error('Approved fixture media hash mismatch')
      return done(new Response(bytes, { headers: { 'Content-Type': 'image/png', 'Cache-Control': 'no-store' } }))
    }
    return done(unavailable())
  }
  const auth = createSessionController({ eventId, fetch: transport, qualifiedRefresh: false, cookiePolicy: isolatedCookiePolicy() })
  const listeners = new Set<() => void>()
  let available = true
  const social = createFeedSocialPorts(mode === 'no-readiness' ? {} : { readiness: context => ({
    read: () => ({ scope: context.scope, bootstrap: context.bootstrap, services: { feed: available } }),
    subscribe(listener) { listeners.add(listener); return () => { listeners.delete(listener) } },
  }) })
  try {
    auth.start()
    await new Promise<void>((resolve, reject) => {
      let stop = () => {}
      const timer = setTimeout(() => { stop(); reject(new Error('Synthetic Auth unavailable')) }, 5000)
      const check = () => {
        if (auth.read().capabilities === 'loading') return
        clearTimeout(timer); stop()
        if (auth.read().emailEnabled) resolve(); else reject(new Error('Synthetic Auth unavailable'))
      }
      stop = auth.subscribe(check); check()
    })
    if (!await auth.login('feed-qualification@example.invalid', 'synthetic-password')) throw new Error('Synthetic login failed')
  } catch (error) { auth.dispose(); throw error }
  return { auth, social, eventId, calls,
    setReady(value: boolean) { available = value; [...listeners].forEach(listener => listener()) },
    invalidate() { void auth.logout() },
    dispose() { if (disposed) return; disposed = true; listeners.clear(); auth.dispose() },
  }
}
