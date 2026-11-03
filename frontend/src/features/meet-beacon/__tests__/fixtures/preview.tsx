import { useState, useSyncExternalStore } from 'react'
import { createRoot } from 'react-dom/client'
import { ExplicitArrivalControls, MeetingBeaconDetail, MeetingBeaconForm } from '../../index'
import { harness, beaconId, partyId, sampleBeacon } from './harness'

const h = await harness()
await h.controller.select(beaconId)
let reports = 0
function Preview() {
  const state = useSyncExternalStore(h.controller.subscribe, h.controller.read), [moves, setMoves] = useState(0)
  const b = state.beacon
  return <main style={{ maxWidth: 680, margin: 'auto', fontFamily: 'system-ui, sans-serif' }}>
    <p style={{ padding: 16 }}><strong>SYNTHETIC TEST FIXTURE ONLY</strong><br />Fictional venue, audience, time, and authority. No real meeting or arrival acceptance.</p>
    <MeetingBeaconDetail controller={h.controller} ownerActions />
    <ExplicitArrivalControls beacon={b} phase={state.phase === 'current' ? 'current' : state.phase === 'moved' ? 'moved' : 'unavailable'}
      currentScope={h.auth.privacy.capture()} now={h.now} pending={false}
      authorization={b ? { scope: h.auth.privacy.capture(), partyId, beaconId: b.beacon_id, beaconRevision: b.revision, meetingRevision: 8, expiresAt: b.meeting.expires_at } : null}
      onIntent={() => { reports++ }} />
    <section className="meeting-beacon"><button onClick={() => setMoves(moves + 1)}>Simulate virtual movement (fixture only)</button><p>Virtual fixture moves: {moves}</p></section>
    <MeetingBeaconForm controller={h.controller} />
  </main>
}
const root = createRoot(document.getElementById('root')!); root.render(<Preview />)
declare global { interface Window { __FE20__: {
  state(): { readonly reports: number; readonly requests: number; readonly writes: number; readonly phase: string }
  mode(value: 'moved' | 'expired' | 'revoked' | 'denied' | 'lost' | 'privacy' | 'ok'): Promise<void>
  dispose(): void
} } }
window.__FE20__ = {
  state: () => ({ reports, requests: h.requests.length, writes: h.requests.filter(r => r.method !== 'GET').length, phase: h.controller.read().phase }),
  async mode(value) {
    if (value === 'moved') { h.controller.invalidate({ beaconId, revision: 2 }); h.setBeacon({ ...sampleBeacon(), revision: 2 }); await h.controller.refresh() }
    if (value === 'expired') h.setTime(Date.parse(h.beacon.meeting.expires_at))
    if (value === 'revoked') h.controller.invalidate({ beaconId, revision: h.beacon.revision + 1, state: 'REVOKED' })
    if (value === 'denied') { h.setMode('denied'); await h.controller.refresh() }
    if (value === 'lost') h.setMode('lost')
    if (value === 'privacy') h.auth.privacy.changePolicy(h.auth.privacy.capture(), 'block')
    if (value === 'ok') { h.setTime(Date.parse('2026-11-07T08:00:00Z')); h.setMode('ok'); h.setBeacon(sampleBeacon()); h.controller.select(null); await h.controller.select(beaconId) }
  },
  dispose() { root.unmount(); h.dispose() },
}
