import { createRoot } from 'react-dom/client'
import { EventFeed } from '../../EventFeed'
import { FeedImage, type MediaPreviewPort } from '../../FeedImage'
import { ConcertWindow } from '../../../../shared/ui/concert-window'
import { post, setup, id, notice } from '../fixtures'
import '../../../../shared/styles/base.css'
import './preview.css'

const mode = new URLSearchParams(location.search).get('mode') ?? 'loaded'
const calls: string[] = [], actions: string[] = [], objectUrls = new Set<string>()
const rows = [
  { ...post, post_id: id(92), author: { ...post.author, display_name: 'Encore fan · fixture' }, text: 'SEE YOU FOR THE ENCORE!\nAnyone swapping favourite songs?', created_at: new Date(Date.now() - 7_200_000).toISOString(), links: [], media_ids: [id(75)], reaction_count: 42, comment_count: 7 },
  { ...post, post_id: id(91), author: { ...post.author, display_name: 'Paper parade · fixture' }, text: 'Trading memories from tonight.\nWhich moment are you keeping?', created_at: new Date(Date.now() - 14_400_000).toISOString(), links: [], media_ids: [id(76)], reaction_count: 27, comment_count: 5 },
]
const s = setup(async input => {
  const request = input as Request, url = new URL(request.url); calls.push(url.pathname + url.search)
  if (mode === 'pending') return new Promise<Response>(() => {})
  if (mode === 'error') return Response.json({ code: 'UNAVAILABLE', message: 'synthetic failure', details: { reason: 'DEPENDENCY_UNAVAILABLE' } }, { status: 503 })
  if (url.pathname.endsWith(`/notices/${notice.notice_id}`)) return Response.json(notice)
  if (url.pathname.endsWith('/notices')) return Response.json({ items: mode === 'empty' ? [] : [notice], next_cursor: null })
  const filter = url.searchParams.get('filter')
  return Response.json({ items: mode === 'empty' || (filter !== 'ALL' && filter !== 'WITH_MEDIA') ? [] : rows, next_cursor: null })
})
if (mode === 'readonly') s.setAccess({ available: true, readOnly: true })
if (mode === 'unavailable') s.setAccess({ available: false, readOnly: true })

// Synthetic media authorization port; actual FeedImage checks scope, purpose, ID and path.
// Fixture bytes are existing venue art, not photographs owned by the synthetic authors.
function createPreview(mediaId: string): MediaPreviewPort {
  const listeners = new Set<() => void>(), scope = s.auth.session.read()
  let state: ReturnType<MediaPreviewPort['read']> = { phase: 'idle', scope, previewUrl: null, media: null, error: null }
  let generation = 0
  const publish = (next: typeof state) => { state = next; listeners.forEach(fn => fn()) }
  return { read: () => state, subscribe(fn) { listeners.add(fn); return () => { listeners.delete(fn) } },
    async load() {
      const run = ++generation; publish({ ...state, phase: 'loading' })
      const path = `/api/concert/events/${post.event_id}/media/${mediaId}/content`
      const response = await fetch(path), blob = await response.blob()
      if (run !== generation) return
      const previewUrl = URL.createObjectURL(blob); objectUrls.add(previewUrl)
      publish({ phase: 'ready', scope, previewUrl, error: null, media: { media_id: mediaId, purpose: 'POST', read_path: path, width: 1152, height: 896 } })
    },
    cancel() { generation++; if (state.previewUrl) { URL.revokeObjectURL(state.previewUrl); objectUrls.delete(state.previewUrl) }; publish({ ...state, phase: 'cancelled', previewUrl: null, media: null }) },
    dispose() { this.cancel(); listeners.clear() },
  }
}
const root = createRoot(document.getElementById('root')!)
function go(value: string) { actions.push(value); document.getElementById('destination')!.textContent = value }
root.render(<>
  <main className="reference-frame"><ConcertWindow title="FEED" headingLevel={1} onClose={() => go('Fixture close destination')}>
    <EventFeed controller={s.controller} onPost={value => go(`Fixture thread ${value}`)} onCompose={() => go('Fixture compose destination')} onLink={link => go(`Fixture link ${link.target_type}`)}
      renderMedia={(mediaId, item) => <FeedImage mediaId={mediaId} eventId={item.event_id} privacy={s.auth.privacy} createPreview={createPreview} alt="Synthetic post attachment: supplied venue illustration" />} />
  </ConcertWindow></main>
  <aside className="reference-qualification">SYNTHETIC FE-REF-FEED · {mode}. Fixture identities/counts; venue illustration bytes reused as test media. Portrait mapping is caller-owned.</aside>
  <p id="destination" role="status" />
</>)
s.controller.start()
window.__REFERENCE_FEED__ = {
  calls, actions,
  invalidate() { s.auth.session.replace(s.auth.session.read().generation, null) },
  resources() { return objectUrls.size },
  dispose() { root.unmount(); s.dispose() },
}
declare global { interface Window { __REFERENCE_FEED__: { calls: string[]; actions: string[]; invalidate(): void; resources(): number; dispose(): void } } }
