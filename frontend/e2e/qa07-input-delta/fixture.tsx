// QA07-only adapters around published production UI. All data/authority is fictional.
import { createRoot } from 'react-dom/client'
import { BrowserRouter, Route, Routes, useParams } from 'react-router-dom'
import { NavigationMemory, usePanelBack } from '../../src/app/navigation'
import { GiftDiscovery, GiftDetail } from '../../src/features/gift-discovery/ui/Gifts'
import { fixture as giftsFixture, giveaway } from '../../src/features/gift-discovery/__tests__/fixtures/data'
import { setup, post, id } from '../../src/features/event-feed/__tests__/fixtures'
import { createFeedThreadController } from '../../src/features/feed-thread/controller'
import { admission as syntheticAdmission } from '../../src/features/auth/__tests__/enrollment-consent/fixtures'
import { FeedThread } from '../../src/features/feed-thread/FeedThread'
import { PartyDiscovery, PartyDetail } from '../../src/features/party-discovery/ui/PartyDiscovery'
import { harness as meetHarness } from '../../src/features/party-discovery/__tests__/fixtures/harness'
import { activity, party } from '../../src/features/party-discovery/__tests__/fixtures/data'
import '../../src/shared/styles/base.css'
import '../../src/features/party-discovery/__tests__/reference/preview.css'
if (!import.meta.env.DEV) throw Error('QA07 fixture requires development mode')
const kind = new URLSearchParams(location.search).get('surface') ?? 'gifts'
const root = createRoot(document.getElementById('root')!)
// Explicit synthetic positive admission, decoded by the actual canonical Auth gate.
// This is not real enrollment or a bypass of session/admission validation.
function admission(request: Request, token: string | null) {
  if (new URL(request.url).pathname !== '/api/auth/enrollment/admission') return null
  if (request.method !== 'GET' || !token || request.headers.get('Authorization') !== 'Bearer ' + token) throw Error('Unexpected synthetic admission request or principal')
  return Response.json(syntheticAdmission())
}
if (kind === 'gifts') {
  const f = giftsFixture({ handler: request => {
    const canonical = admission(request, f.auth.session.read().accessToken); if (canonical) return canonical
    const path = new URL(request.url).pathname
    if (request.method !== 'GET') throw Error('Unexpected synthetic Gifts mutation')
    return Response.json(path.endsWith('/me/giveaways') ? { giving: { items: [], next_cursor: null }, receiving: { items: [], next_cursor: null } }
      : path.endsWith('/giveaways') ? { items: [giveaway()], next_cursor: null } : giveaway())
  } })
  function Detail() { const { giftId = '' } = useParams(); return <GiftDetail controller={f.controller} giveawayId={giftId} /> }
  function ReservationEntry() { const back = usePanelBack(); return <section aria-label="Reservation route fixture"><h2>Reservation entry fixture</h2><p>This only records navigation. No reservation or pickup is simulated.</p><button onClick={back}>Return to gifts</button></section> }
  history.replaceState(null, '', '/gifts')
  root.render(<BrowserRouter><NavigationMemory><main><p>QA07 synthetic Gifts input fixture; no real stock or venue.</p><Routes>
    <Route path="/gifts" element={<GiftDiscovery controller={f.controller} places={[{ id: giveaway().public_place_id, label: 'Synthetic public reference' }]} />} />
    <Route path="/gifts/:giftId" element={<Detail />} /><Route path="/gifts/:giftId/reservation" element={<ReservationEntry />} />
  </Routes></main></NavigationMemory></BrowserRouter>)
  Object.assign(window, { __QA07_DELTA__: { read: () => ({ view: f.controller.read(), admission: f.auth.read().admission, requests: f.requests.map(r => ({ method: r.method, path: new URL(r.url).pathname })) }), dispose() { root.unmount(); f.dispose() } } })
} else if (kind === 'meet') {
  const current = party({ title: 'Area A Queue Group', ticket_section: 'Area A', capacity: 20, member_count: 12, meeting_starts_at: '2026-11-08T08:30:00Z' })
  const h = meetHarness(request => {
    const canonical = admission(request, h.auth.session.read().accessToken); if (canonical) return canonical
    if (request.method !== 'GET') throw Error('Unexpected synthetic Meet mutation')
    const path = new URL(request.url).pathname
    return Response.json(path.endsWith('/me/parties') ? activity() : path.endsWith('/parties') ? { items: [current], next_cursor: null } : current)
  })
  function Detail() { const { partyId = '' } = useParams(); return <PartyDetail controller={h.controller} partyId={partyId} /> }
  history.replaceState(null, '', '/meet')
  root.render(<BrowserRouter><NavigationMemory><div className="reference-fixture"><header className="reference-title"><h1>MEET</h1></header><main className="reference-content"><Routes>
    <Route path="/meet" element={<PartyDiscovery controller={h.controller} presentation={{ canCreate: true }} />} />
    <Route path="/meet/parties/:partyId" element={<Detail />} />
    <Route path="/meet/parties/new" element={<section><h2>Create party route fixture</h2><p>This fixture does not simulate creation success.</p></section>} />
  </Routes></main><aside className="reference-caption">QA07 SYNTHETIC ADMISSION AND PARTY · NO LIVE SERVICE · PORTRAITS UNAVAILABLE</aside></div></NavigationMemory></BrowserRouter>)
  Object.assign(window, { __MEET_REFERENCE__: { admission: () => h.auth.read().admission, requests: () => h.requests.map(r => ({ method: r.method, path: new URL(r.url).pathname })), dispose() { root.unmount(); h.dispose() } } })
} else {
  let currentPost = structuredClone(post)
  let comments = [{ comment_id: id(71), post_id: post.post_id, author: post.author, text: 'Synthetic editable reply', created_at: post.created_at, revision: 1 }]
  const requests: { method: string; path: string }[] = [], reports: string[] = []
  const f = setup(async input => {
    const request = input as Request, path = new URL(request.url).pathname; requests.push({ method: request.method, path })
    const canonical = admission(request, f.auth.session.read().accessToken); if (canonical) return canonical
    if (request.method === 'GET') return Response.json({ post: currentPost, comments, next_cursor: null })
    if (path.endsWith('/reaction')) { currentPost = { ...currentPost, reacted_by_me: !currentPost.reacted_by_me, reaction_count: currentPost.reacted_by_me ? currentPost.reaction_count - 1 : currentPost.reaction_count + 1, revision: currentPost.revision + 1 }; return Response.json({ post_id: currentPost.post_id, reacted_by_me: currentPost.reacted_by_me, reaction_count: currentPost.reaction_count, revision: currentPost.revision }) }
    if (request.method === 'POST' && path.endsWith('/comments')) { const body = await request.json() as { text: string }; const comment = { ...comments[0]!, comment_id: id(72), text: body.text }; comments = [...comments, comment]; return Response.json(comment) }
    throw Error('Unexpected synthetic Thread command')
  })
  f.controller.dispose()
  const controller = createFeedThreadController({ auth: f.auth, access: f.access }); controller.start(); controller.select(post.post_id)
  root.render(<main><p>QA07 synthetic Thread input fixture; no real post or moderation.</p><FeedThread controller={controller} onLink={() => undefined} onEditPost={() => undefined} onReport={target => { reports.push(target.target_id) }} /></main>)
  Object.assign(window, { __QA07_DELTA__: { read: () => ({ view: controller.read(), admission: f.auth.read().admission, requests, reports }), dispose() { root.unmount(); controller.dispose(); f.dispose() } } })
}
