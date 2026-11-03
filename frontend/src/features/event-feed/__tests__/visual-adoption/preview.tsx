import { createRoot } from 'react-dom/client'
import { projectCapabilities } from '../../../../app/capabilities'
import { MemoryRouter, Route, Routes, useNavigate } from 'react-router-dom'
import { EventFeed } from '../../EventFeed'
import { setup, post, id, notice } from '../fixtures'
import { PartyDetail, PartyDiscovery } from '../../../party-discovery/ui/PartyDiscovery'
import { harness } from '../../../party-discovery/__tests__/fixtures/harness'
import { activity, ids, party, errorResponse } from '../../../party-discovery/__tests__/fixtures/data'
import { GiftDetail, GiftDiscovery } from '../../../gift-discovery/ui/Gifts'
import { fixture, giveaway, uid } from '../../../gift-discovery/__tests__/fixtures/data'
import '../../../../shared/styles/base.css'

// Explicitly synthetic populations. No product imagery, live service or physical guidance.
const q = new URLSearchParams(location.search), kind = q.get('kind') ?? 'feed', mode = q.get('mode') ?? 'loaded'
const paused = new Promise<Response>(() => {})
const failure = () => errorResponse(503, 'DEPENDENCY_UNAVAILABLE')
const gifts = [giveaway({ title: 'Fixture fan badges' }), giveaway({ giveaway_id: uid(40), title: 'Fixture ribbon keepsakes', state: 'PAUSED', created_at: '2026-11-03T00:50:00Z' }),
  giveaway({ giveaway_id: uid(39), title: 'Fixture lyric zines', available: 0, created_at: '2026-11-03T00:40:00Z' })]
const parties = [party({ title: 'Fixture encore crew' }), party({ party_id: ids.next, title: 'Fixture seated-section crew', member_count: 5, created_at: '2026-11-01T00:00:00Z' })]
const feed = setup(async input => {
  if (mode === 'pending') return paused
  if (mode === 'error') return failure()
  const url = new URL((input as Request).url)
  if (url.pathname.endsWith('/notices')) return Response.json({ items: mode === 'empty' ? [] : [notice], next_cursor: null })
  const rows = [0, 1, 2].map((n) => ({ ...post, post_id: id(90 - n), text: ['Fixture: who else is waiting for the encore?', 'Fixture: sharing our favourite moment from tonight.', 'Fixture: looking forward to the next song.'][n],
    author: { ...post.author, display_name: ['Alex · fixture', 'Sam · fixture', 'Jules · fixture'][n] }, created_at: `2026-11-03T0${3 - n}:00:00Z`, media_ids: [], links: [], comment_count: n + 1, reaction_count: 3 + n }))
  return Response.json({ items: mode === 'empty' || url.searchParams.get('filter') !== 'ALL' ? [] : rows, next_cursor: null })
})
const p = harness(request => {
  if (mode === 'pending') return paused
  if (mode === 'error') return failure()
  const path = new URL(request.url).pathname
  return Response.json(path.endsWith('/me/parties') ? activity() : path.endsWith('/parties') ? { items: mode === 'empty' ? [] : parties, next_cursor: null } : parties[0])
})
const g = fixture({ handler: request => {
  if (mode === 'pending') return paused
  if (mode === 'error') return failure()
  return Response.json(new URL(request.url).pathname.endsWith('/giveaways') ? { items: mode === 'empty' ? [] : gifts, next_cursor: null } : gifts[0])
} })
if (mode === 'readonly') {
  feed.setAccess({ available: true, readOnly: true })
  p.update({ ...p.access()!, phase: 'READ_ONLY', capabilities: projectCapabilities({ ...p.access()!.capabilities, mutations: { available: false, reason: 'EVENT_READ_ONLY' } }) })
  g.publish({ ...g.context()!, eventPhase: 'READ_ONLY', capabilities: projectCapabilities({ ...g.context()!.capabilities, mutations: { available: false, reason: 'EVENT_READ_ONLY' } }) })
}
if (mode === 'unavailable') { feed.setAccess({ available: false, readOnly: true }); p.update(null); g.publish(null) }
if (kind === 'feed') feed.controller.start()
function Feed() {
  const navigate = useNavigate()
  return <EventFeed controller={feed.controller} onPost={value => { void navigate(`/feed/${value}`) }} onCompose={() => { void navigate('/feed/new') }} onLink={() => undefined} />
}
function Preview() {
  return <><aside className="qualification">SYNTHETIC UIV03 FIXTURE · {mode}<br />Fictional people and places. No live service or product art.</aside>
    <Routes><Route path="/feed" element={<Feed />} /><Route path="/feed/new" element={<p>Fixture compose destination reached.</p>} /><Route path="/feed/:id" element={<p>Fixture thread destination reached.</p>} />
      <Route path="/meet" element={<PartyDiscovery controller={p.controller} />} /><Route path="/meet/parties/:id" element={<PartyDetail controller={p.controller} partyId={ids.party} />} /><Route path="/meet/people" element={<p>Fixture people destination reached.</p>} />
      <Route path="/gifts" element={<GiftDiscovery controller={g.controller} />} /><Route path="/gifts/:id" element={<GiftDetail controller={g.controller} giveawayId={giveaway().giveaway_id} />} /><Route path="/gifts/mine" element={<p>Fixture own-gifts destination reached.</p>} />
    </Routes></>
}
const style = document.createElement('style'); style.textContent = 'body{margin:0;padding:0;background:#d7cbb4}.qualification{padding:8px;font:11px/1.3 Arial,sans-serif;color:#1d1a17}'; document.head.append(style)
const root = createRoot(document.getElementById('root')!)
root.render(<MemoryRouter initialEntries={[kind === 'party' ? '/meet' : kind === 'gift' ? '/gifts' : '/feed']}><Preview /></MemoryRouter>)
window.__UIV03__ = { dispose() { root.unmount(); feed.dispose(); p.dispose(); g.dispose(); style.remove() } }
declare global { interface Window { __UIV03__: { dispose(): void } } }
