// DEV-only qualification: real components/owner, synthetic authorized HTTP, no production readiness grant.
import { useState } from 'react'
import { createRoot } from 'react-dom/client'
import { MemoryRouter } from 'react-router-dom'
import { RootMediaProvider } from '../../../../app/media-entry/provider'
import { RootPostImage } from '../../../../app/media-entry/RootMediaView'
import { RootProfilePostImage } from '../../../../app/media-entry/RootProfilePostImage'
import { parse, type Page, type Post } from '../../../event-feed/contract'
import { MeHub } from '../../ui/MePages'
import { activity } from '../reference-target/data'
import { participant, setup, userId, eventId } from '../fixtures'
import '../reference-target/preview.css'
import '../../../../shared/styles/base.css'
if (!import.meta.env.DEV) throw Error('Synthetic Profile media preview is development-only')
const mode = new URLSearchParams(location.search).get('mode') ?? 'loaded'
const mediaIds = ['00000000-0000-4000-8000-000000000071', '00000000-0000-4000-8000-000000000072']
const readCounts = { feed: 0, media: 0 }
let photo: ArrayBuffer | null = null
// Existing non-character art is only a clearly synthetic protected POST photo payload.
async function imageBytes() {
  if (!photo) photo = await (await fetch('/world/illustrated/parade-ground.png')).arrayBuffer()
  return photo
}
const h = setup(async input => {
  const request = input as Request, path = new URL(request.url).pathname
  if (path.endsWith('/feed')) {
    readCounts.feed++
    const source = activity(h.auth.privacy.capture(), userId)
    return Response.json({ ...source.page, items: source.page.items.map((post, i) => ({ ...post, media_ids: i === 0 ? [mediaIds[0]!, '00000000-0000-4000-8000-000000000073'] : [mediaIds[1]!] })) })
  }
  if (path.includes('/media/')) {
    readCounts.media++
    if (mode !== 'loaded') return new Response(null, { status: 503 })
    const bytes = await imageBytes(), view = new DataView(bytes), mediaId = path.split('/').at(path.endsWith('/content') ? -2 : -1)
    if (!mediaIds.includes(mediaId ?? '')) return new Response(null, { status: 404 })
    if (path.endsWith('/content')) return new Response(bytes.slice(0), { headers: { 'Content-Type': 'image/png' } })
    return Response.json({ media_id: mediaId, state: 'READY', revision: 1, width: view.getUint32(16), height: view.getUint32(20), content_type: 'image/png', purpose: 'POST', read_path: `/api/concert/events/${eventId}/media/${mediaId}/content` })
  }
  if (path.endsWith('/me')) return Response.json(participant())
  return new Response(null, { status: 404 })
})
await h.controller.load()
const scope = h.auth.privacy.capture()
let page: Page<Post> | null = null, show: (stage: 'feed' | 'profile') => void = () => undefined, ready = false
function Preview() {
  const [stage, setStage] = useState<'feed' | 'profile'>('profile'); show = setStage
  return <RootMediaProvider auth={h.auth} scope={scope}><MemoryRouter>
    {stage === 'feed' && page ? <section className="fixture-feed">{page.items.map(post => <RootPostImage key={post.post_id} auth={h.auth} scope={scope} mediaId={post.media_ids[0]!} eventId={eventId} alt="Synthetic authorized POST photo" />)}</section>
      : <MeHub controller={h.controller} auth={h.auth} {...(page ? { activity: { scope, subjectUserId: userId, page } } : {})}
        renderPostMedia={post => <RootProfilePostImage auth={h.auth} scope={scope} post={post} />} />}
  </MemoryRouter></RootMediaProvider>
}
const root = createRoot(document.getElementById('root')!); root.render(<Preview />)
async function waitFor(test: () => boolean) {
  const end = performance.now() + 8000
  while (!test()) { if (performance.now() > end) throw Error('Synthetic Feed stage did not settle'); await new Promise(resolve => setTimeout(resolve, 15)) }
}
await waitFor(() => document.querySelector('.me-hub') !== null); ready = true
const api = {
  state: () => ({ ready, readCounts: { ...readCounts }, expectedImages: mode === 'loaded' ? 2 : 0, mode }),
  async prepareFeed() {
    if (mode === 'cold') throw Error('Cold Profile cannot prepare Feed')
    const response = await h.auth.privacy.run(scope, async ({ signal }) => h.auth.request({ path: `/api/concert/events/${eventId}/feed`, method: 'GET', access: 'protected', errorFamily: 'domain', domainOperation: 'listConcertFeed', signal,
      decode: value => parse<Page<Post>>('Domain_feed_FeedPage', value) }))
    page = response.data; show('feed')
    await waitFor(() => mode === 'loaded' ? document.querySelectorAll('.fixture-feed img').length === 2 && [...document.querySelectorAll<HTMLImageElement>('.fixture-feed img')].every(image => image.complete && image.naturalWidth > 0)
      : [...document.querySelectorAll('.fixture-feed p')].filter(e => e.textContent?.startsWith('Image unavailable.')).length === 2)
  },
  showProfile() { show('profile') },
  revoke() { h.auth.privacy.changePolicy(scope, 'consent-revoked') },
  dispose() { root.unmount(); h.dispose() },
}
Object.assign(window, { __PROFILE_ACTIVITY_MEDIA__: api })
declare global { interface Window { __PROFILE_ACTIVITY_MEDIA__: typeof api } }
