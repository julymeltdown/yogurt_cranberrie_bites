import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { MemoryRouter, Route, Routes, useNavigate } from 'react-router-dom'
import { MeetingBeaconPanel, MeetingCheckInPanel, MeetingEntryFeatures } from '../index'
import { beaconId, entryFixture } from './fixtures'

const f = await entryFixture()
function Fixture() {
  const navigate = useNavigate()
  return <main style={{ fontFamily: 'system-ui, sans-serif', maxWidth: 680, margin: 'auto' }}>
    <section className="meeting-beacon"><p><strong>SYNTHETIC ADAPTER FIXTURE ONLY</strong><br />Fictional venue, time, policy and people. No real meeting guidance.</p>
      <nav aria-label="Fixture panels" className="meeting-actions"><button onClick={() => navigate(`/meet/beacons/${beaconId}`)}>Open beacon fixture</button><button onClick={() => navigate('/world/check-in')}>Open check-in fixture</button></nav></section>
    <Routes><Route path="/world/check-in" element={<MeetingCheckInPanel />} /><Route path="/meet/beacons/:beaconId" element={<MeetingBeaconPanel />} /></Routes>
  </main>
}
const root = createRoot(document.getElementById('root')!)
root.render(<StrictMode><MemoryRouter initialEntries={[`/meet/beacons/${beaconId}`]}><MeetingEntryFeatures auth={f.h.auth} release={f.release}><Fixture /></MeetingEntryFeatures></MemoryRouter></StrictMode>)
declare global { interface Window { __MEETING_ENTRY__: {
  state(): { writes: number; subscriptions: ReturnType<typeof f.subscriptions> }
  clock(available: boolean): void
  mode(value: 'lost' | 'revoke' | 'privacy'): void
  dispose(): void
} } }
window.__MEETING_ENTRY__ = {
  state: () => ({ writes: f.h.requests.filter(r => r.method !== 'GET').length, subscriptions: f.subscriptions() }),
  clock: available => f.setClock(available),
  mode(value) {
    if (value === 'lost') f.h.setMode('lost')
    if (value === 'revoke') f.invalidate({ scope: f.scope, beaconId, revision: 2, state: 'REVOKED' })
    if (value === 'privacy') f.h.auth.privacy.changePolicy(f.scope, 'block')
  },
  dispose() { root.unmount(); f.dispose() },
}
