import { createRoot } from 'react-dom/client'
import { useState } from 'react'
import { FeedThread } from '../../../feed-thread/FeedThread'
import { createFeedThreadController } from '../../../feed-thread/controller'
import { EventFeed } from '../../EventFeed'
import { FeedImage, type MediaPreviewPort } from '../../FeedImage'
import { ConcertWindow } from '../../../../shared/ui/concert-window'
import { post, setup, id, notice } from '../fixtures'
import '../../../../shared/styles/base.css'
import './preview.css'

const mode = new URLSearchParams(location.search).get('mode') ?? 'loaded'
const calls: string[] = [], actions: string[] = [], objectUrls = new Set<string>()
const rows = [
  { ...post, post_id: id(92), author: { ...post.author, display_name: 'Encore fan' }, text: 'SEE YOU FOR THE ENCORE!\nAnyone swapping favourite songs?', created_at: new Date(Date.now() - 7_200_000).toISOString(), links: [], media_ids: [id(75)], reaction_count: 42, comment_count: 7 },
  { ...post, post_id: id(91), author: { ...post.author, display_name: 'Paper parade' }, text: 'Trading memories from tonight.\nWhich moment are you keeping?', created_at: new Date(Date.now() - 14_400_000).toISOString(), links: [], media_ids: [id(76)], reaction_count: 27, comment_count: 5 },
]
const s = setup(async input => {
  const request = input as Request, url = new URL(request.url); calls.push(url.pathname + url.search)
  if (mode === 'pending') return new Promise<Response>(() => {})
  if (mode === 'error') return Response.json({ code: 'UNAVAILABLE', message: 'synthetic failure', details: { reason: 'DEPENDENCY_UNAVAILABLE' } }, { status: 503 })
  if (url.pathname.endsWith(`/notices/${notice.notice_id}`)) return Response.json(notice)
  if (url.pathname.endsWith('/notices')) return Response.json({ items: mode === 'empty' ? [] : [notice], next_cursor: null })
  if (url.pathname.includes('/posts/')) {
    const item = rows.find(row => url.pathname.endsWith('/' + row.post_id))
    return item ? Response.json({ post: item, comments: [], next_cursor: null }) : Response.json({}, { status: 404 })
  }
  const filter = url.searchParams.get('filter')
  return Response.json({ items: mode === 'empty' || (filter !== 'ALL' && filter !== 'WITH_MEDIA') ? [] : rows, next_cursor: null })
})
if (mode === 'readonly') s.setAccess({ available: true, readOnly: true })
if (mode === 'unavailable') s.setAccess({ available: false, readOnly: true })

// FIXTURE ONLY. Root supplies exact approved native crops; no product media reader is replaced.
// Each native crop is the complete fixture input; it is not an original post photograph.
const thread = createFeedThreadController({ auth: s.auth, access: s.access })
const inputs: Record<string, { path: string; sha256: string; width: number; height: number }> = {
  [id(75)]: { path: '/reference-feed-media-v1/reference-card-1.png', sha256: 'a28db3162d4fe822b17d3228ad82866d4eeb0e2524fb606d469ccd925fd7385d', width: 197, height: 67 },
  [id(76)]: { path: '/reference-feed-media-v1/reference-card-2.png', sha256: '2c5863585bce002be0ea0b057f4927d67c6f99e97cc7f5edee6ba9c47589ffdd', width: 197, height: 46 },
}
function createPreview(mediaId: string): MediaPreviewPort {
  const listeners = new Set<() => void>(), scope = s.auth.session.read()
  let state: ReturnType<MediaPreviewPort['read']> = { phase: 'idle', scope, previewUrl: null, media: null, error: null }
  let generation = 0
  const publish = (next: typeof state) => { state = next; listeners.forEach(fn => fn()) }
  return { read: () => state, subscribe(fn) { listeners.add(fn); return () => { listeners.delete(fn) } },
    async load() {
      const run = ++generation; publish({ ...state, phase: 'loading' })
      const path = `/api/concert/events/${post.event_id}/media/${mediaId}/content`
      try {
        const input = inputs[mediaId]
        if (!input) throw new Error('Unqualified fixture media')
        const response = await fetch(input.path)
        if (!response.ok) throw new Error('Fixture image unavailable')
        const bytes = await response.arrayBuffer()
        const digest = [...new Uint8Array(await crypto.subtle.digest('SHA-256', bytes))].map(value => value.toString(16).padStart(2, '0')).join('')
        if (digest !== input.sha256) throw new Error('Fixture media hash mismatch')
        const blob = new Blob([bytes], { type: 'image/png' }), decoded = await createImageBitmap(blob)
        const width = decoded.width, height = decoded.height; decoded.close()
        if (width !== input.width || height !== input.height) throw new Error('Fixture media dimensions mismatch')
        if (run !== generation) return
        if (state.previewUrl) { URL.revokeObjectURL(state.previewUrl); objectUrls.delete(state.previewUrl) }
        const previewUrl = URL.createObjectURL(blob); objectUrls.add(previewUrl)
        publish({ phase: 'ready', scope, previewUrl, error: null, media: { media_id: mediaId, purpose: 'POST', read_path: path, width, height } })
      } catch {
        if (run === generation) publish({ ...state, phase: 'failed', error: 'Fixture image unavailable' })
      }
    },
    cancel() { generation++; if (state.previewUrl) { URL.revokeObjectURL(state.previewUrl); objectUrls.delete(state.previewUrl) }; publish({ ...state, phase: 'cancelled', previewUrl: null, media: null }) },
    dispose() { this.cancel(); listeners.clear() },
  }
}
const root = createRoot(document.getElementById('root')!)
function go(value: string) { actions.push(value); document.getElementById('destination')!.textContent = value }
function App() {
  const [page, setPage] = useState<'feed' | 'thread'>('feed')
  const open = (value: string) => { if (thread.select(value)) { go(`Fixture thread ${value}`); setPage('thread') } }
  const media = (mediaId: string, item: typeof post) => <FeedImage mediaId={mediaId} eventId={item.event_id} privacy={s.auth.privacy} createPreview={createPreview} alt={mediaId === id(75) ? 'Concert silhouette illustration' : 'Band sticker collage'} />
  return <>
    <main className="reference-frame reference-native-frame"><ConcertWindow title={page === 'feed' ? 'FEED' : 'THREAD'} headingLevel={1} onClose={() => page === 'thread' ? setPage('feed') : go('Fixture close destination')}>
      {page === 'feed' ? <EventFeed controller={s.controller} onPost={open} onCompose={() => go('Fixture compose destination')} onLink={link => go(`Fixture link ${link.target_type}`)} renderMedia={media} />
        : <FeedThread controller={thread} renderMedia={media} onLink={link => go(`Fixture link ${link.target_type}`)} onEditPost={() => go('Fixture edit destination')} onReport={() => go('Fixture report destination')} />}
    </ConcertWindow></main>
    <p id="destination" role="status" />
  </>
}
root.render(<App />)
s.controller.start(); thread.start()
window.__REFERENCE_MEDIA_FEED__ = {
  calls, actions,
  qualification: { mode, inputCommit: '8f6fe07f389f73f09d9fc61a6012b49846b24ce6', inputs, syntheticIdentitiesAndCounts: true, completeNativeCrops: true, productionMedia: false },
  invalidate() { s.auth.session.replace(s.auth.session.read().generation, null) },
  resources() { return objectUrls.size },
  dispose() { root.unmount(); thread.dispose(); s.dispose() },
}
declare global { interface Window { __REFERENCE_MEDIA_FEED__: { calls: string[]; actions: string[]; qualification: unknown; invalidate(): void; resources(): number; dispose(): void } } }
