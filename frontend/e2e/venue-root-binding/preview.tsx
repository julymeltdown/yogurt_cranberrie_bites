// Controlled integration harness, not the default WorldShell or a live venue.
import { useEffect } from 'react'
import { createRoot } from 'react-dom/client'
import { Link, MemoryRouter, Outlet, Route, Routes, useLocation, useNavigate } from 'react-router-dom'
import { VenueRootFeatures } from '../../src/app/venue-root'
import { ParadiseCityMapPanel } from '../../src/app/venue-entry/ParadiseCityMapPanel'
import { VenueCommunityOutlet } from '../../src/app/venue-community-entry'
import { useMeetingLeafOwner } from '../../src/app/venue-community-entry/provider'
import { fixture } from './fixture'

const f = await fixture(), owners = new Set(), clock = f.clock
let owner: ReturnType<typeof useMeetingLeafOwner> = null
const borrowed = f.input
function Probe() {
  const value = useMeetingLeafOwner(), navigate = useNavigate(), location = useLocation()
  owner = value
  if (value) owners.add(value)
  useEffect(() => { f.navigate(() => navigate('/map')) }, [navigate])
  return <output aria-label="Fixture route">{location.pathname}</output>
}
function Layout() {
  return <div><p>CONTROLLED FIXTURE · social permissions and meeting details are synthetic.</p>
    <p>Borrowed canvas below is a shell-lifetime sentinel, not a Phaser/live World qualification.</p>
    <canvas width="8" height="8" data-borrowed-world-fixture="" aria-label="Borrowed shell canvas sentinel" />
    <VenueRootFeatures borrowed={borrowed}><Probe /><nav><Link to="/map">Map route</Link> <Link to="/meeting">Meeting route</Link></nav><Outlet /></VenueRootFeatures>
  </div>
}
createRoot(document.getElementById('root')!).render(<MemoryRouter initialEntries={['/map']}><Routes><Route element={<Layout />}>
  <Route path="/map" element={<ParadiseCityMapPanel />} /><Route path="/meeting" element={<VenueCommunityOutlet />} />
</Route></Routes></MemoryRouter>)
Object.assign(window, { venueRootFixture: {
  openMessage: () => owner?.open(f.intent()), openInvite: () => owner?.open(f.intent('INVITE')),
  draft: (value: string) => owner?.setDraft(value), revoke: f.revoke, abort: () => f.lifetime.abort(),
  inspect: () => ({ owners: owners.size, clockSame: f.clock === clock, borrowedSame: borrowed === f.input,
    counts: f.counts(), current: owner?.read().phase ?? null }), dispose: f.dispose,
} })
