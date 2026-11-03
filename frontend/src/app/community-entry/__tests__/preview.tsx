import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { MemoryRouter, Route, Routes, useNavigate } from 'react-router-dom'
import { CommunityEntryProvider, communityEntryPanels } from '../index'
import { fixture, giftId, reservationId, ids } from './fixture'

const query = new URLSearchParams(location.search), f = fixture({ owner: query.has('owner'), storage: sessionStorage })
const paths = { party: '/meet/parties/new', giveaway: '/gifts/new', manageParty: `/meet/parties/${ids.party}/manage`,
  rally: `/meet/parties/${ids.party}/rally`, manage: `/gifts/${giftId}/manage`, reserve: `/gifts/${giftId}/reservation`, pickup: `/gifts/pickups/${reservationId}` }
function Fixture() {
  const navigate = useNavigate()
  return <main style={{ fontFamily: 'system-ui, sans-serif', maxWidth: 720, margin: 'auto', overflowWrap: 'anywhere' }}>
    <p><strong>SYNTHETIC ADAPTER FIXTURE</strong><br />Fictional people, places, time and policy. No real venue or pickup guidance.</p>
    <nav aria-label="Fixture routes" style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>{Object.entries(paths).map(([name, path]) =>
      <button style={{ minHeight: 44, minWidth: 44 }} key={name} onClick={() => navigate(path)}>{name}</button>)}</nav>
    <Routes>{Object.entries(communityEntryPanels).map(([path, element]) => <Route key={path} path={path} element={element} />)}</Routes>
  </main>
}
const root = createRoot(document.getElementById('root')!)
root.render(<StrictMode><MemoryRouter initialEntries={[paths[query.get('start') as keyof typeof paths] ?? paths.party]}>
  <CommunityEntryProvider auth={f.auth} release={f.release}><Fixture /></CommunityEntryProvider>
</MemoryRouter></StrictMode>)
declare global { interface Window { __COMMUNITY_ENTRY__: { lost(): void; revoke(): void; clock(): void;
  state(): { writes: number; reads: number; journal: unknown; subscriptions: ReturnType<typeof f.subscriptions> }; dispose(): void } } }
window.__COMMUNITY_ENTRY__ = {
  lost: f.loseResponse, revoke: () => { f.auth.privacy.changePolicy(f.scope, 'block') }, clock: () => f.clock(false),
  state: () => ({ writes: f.requests.filter(r => r.method !== 'GET').length, reads: f.requests.filter(r => r.method === 'GET').length,
    journal: f.journal.read(f.scope.userId!, f.scope.eventId!), subscriptions: f.subscriptions() }),
  dispose() { root.unmount(); f.dispose() },
}
