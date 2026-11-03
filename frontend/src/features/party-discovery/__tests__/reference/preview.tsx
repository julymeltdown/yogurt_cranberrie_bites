import { createRoot } from 'react-dom/client'
import { BrowserRouter, Link, Route, Routes, useNavigate, useParams } from 'react-router-dom'
import { NavigationMemory } from '../../../../app/navigation'
import { PartyDetail, PartyDiscovery } from '../../ui/PartyDiscovery'
import { harness } from '../fixtures/harness'
import { activity, application, errorResponse, ids, party } from '../fixtures/data'
import type { Party } from '../../model/contract'
import '../../../../shared/styles/base.css'
import './preview.css'

// Frozen contract fixtures only. No art, physical venue authority or real-service qualification.
const parties: readonly Party[] = Object.freeze([
  party({ title: 'Area A Queue Group', ticket_section: 'Area A', capacity: 20, member_count: 12, meeting_starts_at: '2026-11-08T08:30:00Z' }),
  party({ party_id: ids.next, title: 'After Show Hangout', ticket_section: 'Any section', capacity: 12, member_count: 8, meeting_starts_at: '2026-11-08T12:30:00Z', meeting_ends_at: '2026-11-08T13:30:00Z' }),
  party({ party_id: '00000000-0000-4000-8000-000000000012', title: 'Photo Walk (Parade ver.)', ticket_section: null, capacity: 8, member_count: 5, meeting_starts_at: '2026-11-08T07:00:00Z' }),
])
let currentMode = 'ready', currentApplication = false
const h = harness(r => {
  const path = new URL(r.url).pathname
  if (currentMode === 'denied') return errorResponse(403, 'PERMISSION_DENIED')
  if (r.method === 'POST') { currentApplication = true; return Response.json(application()) }
  if (path.endsWith('/me/parties')) return Response.json({ ...activity(), applications: { items: currentApplication ? [application()] : [], next_cursor: null } })
  if (path.endsWith('/parties')) return Response.json({ items: currentMode === 'empty' ? [] : currentMode === 'full' ? parties.map(p => ({ ...p, member_count: p.capacity })) : parties, next_cursor: null })
  return Response.json(parties.find(p => path.endsWith(p.party_id)) ?? parties[0])
})
function Detail() { const { partyId = '' } = useParams(); return <PartyDetail controller={h.controller} partyId={partyId} /> }
function Frame() {
  const navigate = useNavigate()
  return <div className="reference-fixture"><header className="reference-title"><h1>MEET</h1><button aria-label="Close MEET" onClick={() => void navigate('/world')}>×</button></header>
    <main className="reference-content"><Routes>
      <Route path="*" element={<PartyDiscovery controller={h.controller} presentation={{ canCreate: true }} />} />
      <Route path="/meet/parties/:partyId" element={<Detail />} />
      <Route path="/meet/people" element={<section><h2>People route fixture</h2><p>This fixture does not simulate introductions.</p><Link to="/meet">Back to parties</Link></section>} />
      <Route path="/meet/parties/new" element={<section><h2>Create party route fixture</h2><p>This fixture does not simulate creation success.</p><Link to="/meet">Back to parties</Link></section>} />
      <Route path="/world" element={<Link to="/meet">Open MEET</Link>} />
    </Routes></main><aside className="reference-caption">FROZEN FIXTURE · NO LIVE SERVICE · PORTRAITS UNAVAILABLE</aside></div>
}
const root = createRoot(document.getElementById('root')!)
root.render(<BrowserRouter><NavigationMemory><Frame /></NavigationMemory></BrowserRouter>)
window.__MEET_REFERENCE__ = {
  requests: () => h.requests.map(r => ({ method: r.method, path: new URL(r.url).pathname })),
  async mode(value: string) { currentMode = value; await h.controller.loadList() },
  revoke() { h.auth.privacy.changePolicy(h.auth.privacy.capture(), 'block') },
  dispose() { root.unmount(); h.dispose() },
}
declare global { interface Window { __MEET_REFERENCE__: { requests(): { method: string; path: string }[]; mode(value: string): Promise<void>; revoke(): void; dispose(): void } } }
