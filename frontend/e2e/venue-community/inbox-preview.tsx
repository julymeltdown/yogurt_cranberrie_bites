import { useEffect, useSyncExternalStore } from 'react'
import { createRoot } from 'react-dom/client'
import { MemoryRouter } from 'react-router-dom'
import { InvitationInbox } from '../../src/features/venue-invitations/Inbox'
import { fixtureIds, invitationView, meetingFixture } from './fixture'
import './preview.css'

const portrait = (frame: number) => <span className="fixture-approved-portrait" style={{ backgroundPosition: `${frame * 20}% center` }} />
const fixture = meetingFixture([portrait(0), portrait(3)])
const now = fixture.time(), target = fixture.inviteIntent, presentation = fixture.presentation
const DAY_MS = 24 * 60 * 60 * 1000
fixture.setInbox({ INCOMING: [
  { invitation: invitationView(fixtureIds.invitation, 'pending', now + DAY_MS, ['accept', 'decline']), target, presentation },
  { invitation: invitationView(fixtureIds.invitation2, 'pending', now + DAY_MS, ['accept', 'decline']), target, presentation },
  { invitation: invitationView(fixtureIds.invitation3, 'accepted', now - 1, []), target, presentation },
  { invitation: invitationView(fixtureIds.invitation8, 'declined', now - 1, []), target, presentation },
  { invitation: invitationView(fixtureIds.invitation4, 'expired', now - 1, []), target, presentation },
], OUTGOING: [
  { invitation: invitationView(fixtureIds.invitation5, 'pending', now + DAY_MS, ['cancel']), target, presentation },
  { invitation: invitationView(fixtureIds.invitation6, 'accepted', now + DAY_MS, ['cancel']), target, presentation },
  { invitation: invitationView(fixtureIds.invitation7, 'cancelled', now - 1, []), target, presentation },
] })
function App() {
  const state = useSyncExternalStore(fixture.owner.subscribe, fixture.owner.read, fixture.owner.read)
  useEffect(() => { void fixture.owner.openInbox('INCOMING') }, [])
  return <main><aside className="fixture-label">CONTROLLED FIXTURE · fictional permissions and clock · no live meeting guidance</aside>
    <InvitationInbox owner={fixture.owner} state={state}
      onDirection={direction => { void fixture.owner.openInbox(direction) }}
      onRefresh={() => { void fixture.owner.refreshInbox() }} /></main>
}
const root = createRoot(document.getElementById('root')!)
root.render(<MemoryRouter><App /></MemoryRouter>)
declare global { interface Window { __VENUE_INBOX_FIXTURE__: {
  state(): { writes: number; inboxReads: number; phase: string; direction: string; notice: string | null;
    statuses: string[]; capacity: { limit: number; joined: number } | null }
  open(direction: 'INCOMING' | 'OUTGOING'): Promise<boolean>
  setFull(): void
  deny(): Promise<boolean>
  dispose(): void
} } }
window.__VENUE_INBOX_FIXTURE__ = {
  state() { const current = fixture.owner.read(); return { writes: fixture.writes.length, inboxReads: fixture.inboxReads.length,
    phase: current.inbox.phase, direction: current.inbox.direction, notice: current.inbox.notice,
    statuses: current.inbox.items.map(item => item.invitation.status), capacity: current.inbox.capacity } },
  open(direction) { return fixture.owner.openInbox(direction) },
  setFull() { fixture.setInboxCapacity({ limit: 8, joined: 8 }); void fixture.owner.refreshInbox() },
  async deny() {
    fixture.setWriteResult({ state: 'rejected', reason: 'denied' })
    await fixture.owner.openInbox('OUTGOING')
    return fixture.owner.decideInbox(fixtureIds.invitation6, 'cancel')
  },
  dispose() { root.unmount(); fixture.dispose() },
}
