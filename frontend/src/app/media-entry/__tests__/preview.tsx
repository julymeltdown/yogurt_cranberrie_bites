import { useEffect, useState, useSyncExternalStore } from 'react'
import { createRoot } from 'react-dom/client'
import { FeedImage } from '../../../features/event-feed/FeedImage'
import { createSessionController } from '../../../features/auth/model/session'
import { createMediaOwner, type MediaLease } from '../index'
const eventId = '00000000-0000-4000-8000-000000000002'
const postId = '00000000-0000-4000-8000-000000000003', giftId = '00000000-0000-4000-8000-000000000004'
// Deliberate qualification-only synthetic identity and accessible parent-row fixture.
const auth = createSessionController({ eventId })
auth.session.replace(0, { userId: '00000000-0000-4000-8000-000000000001', eventId, accessToken: 'synthetic-media-lease' })
const created: { url: string; bytes: number }[] = [], revoked: string[] = []
const owner = createMediaOwner({ auth, scope: auth.privacy.capture(), maxEntries: 2, previews: {
  create(blob) { const url = URL.createObjectURL(blob); created.push({ url, bytes: blob.size }); return url },
  revoke(url) { URL.revokeObjectURL(url); revoked.push(url) },
} })
const createPost = (mediaId: string) => owner.acquire({ mediaId, purpose: 'POST' })
function GiftPhoto() {
  const [lease, setLease] = useState<MediaLease | null>(null)
  useEffect(() => { const lease = owner.acquire({ mediaId: giftId, purpose: 'GIVEAWAY' }); setLease(lease); void lease.load(); return lease.release }, [])
  return lease ? <Photo lease={lease} /> : <p>Loading giveaway photo…</p>
}
function Photo({ lease }: { lease: MediaLease }) {
  const state = useSyncExternalStore(lease.subscribe, lease.read, lease.read)
  return state.phase === 'ready' && state.previewUrl ? <img src={state.previewUrl} alt="Authorized synthetic GIVEAWAY photo" width={state.media!.width} height={state.media!.height} /> : <p role="status">{state.phase}</p>
}
function Preview() {
  const [route, setRoute] = useState<'none' | 'post' | 'gift'>('none')
  return <main><h1>Scoped media lease qualification</h1><p>Synthetic authorized parent rows and photos. No production sign-in or readiness claim.</p>
    <button onClick={() => setRoute('post')}>Show post</button> <button onClick={() => setRoute('gift')}>Show gift</button> <button onClick={() => setRoute('none')}>Leave row</button>
    <button onClick={() => auth.privacy.changePolicy(auth.privacy.capture(), 'block')}>Revoke policy</button>
    <button onClick={() => auth.session.replace(auth.session.read().generation, null)}>End identity</button>
    {route === 'post' && <section><h2>Accessible POST row</h2><FeedImage createPreview={createPost} privacy={auth.privacy} mediaId={postId} eventId={eventId} alt="Authorized synthetic POST photo" /></section>}
    {route === 'gift' && <section><h2>Accessible GIVEAWAY row</h2><GiftPhoto /></section>}
  </main>
}
const root = createRoot(document.getElementById('root')!); root.render(<Preview />)
const api = { stats: () => ({ pool: owner.inspect(), created: [...created], revoked: [...revoked] }), dispose: () => { root.unmount(); owner.dispose(); auth.dispose() } }
declare global { interface Window { __MEDIA_LEASE__: typeof api } }
window.__MEDIA_LEASE__ = api
