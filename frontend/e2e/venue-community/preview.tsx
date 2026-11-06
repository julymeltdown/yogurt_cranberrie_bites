import { useState } from 'react'
import { createRoot } from 'react-dom/client'
import { MeetingLeaf } from '../../src/app/venue-community-entry/view'
import { fixtureIds, meetingFixture } from './fixture'
import './preview.css'

// Exact reviewed static derivative of the user's selected source pack. Not C06/synthetic art.
const portrait = (frame: number) => <span className="fixture-approved-portrait" style={{ backgroundPosition: `${frame * 20}% center` }} />
const fixture = meetingFixture([portrait(0), portrait(3)], true)
fixture.patch({ invitation: { id: fixtureIds.invitation, version: '1', status: 'pending', expiresAtMs: fixture.time() + 600_000, allowedActions: ['decline', 'accept'] }, invitationActions: ['decline', 'accept'] })
await fixture.owner.open(fixture.inviteIntent)
function FixtureApp() {
  const [, redraw] = useState(0)
  return <main><aside className="fixture-label">CONTROLLED FIXTURE · fictional permissions and clock · no live meeting guidance</aside>
    <nav aria-label="Fixture states"><button onClick={() => fixture.owner.surface('invitation')}>Invitation</button>
      <button onClick={() => fixture.owner.surface('conversation')}>Conversation</button>
      <button onClick={() => { fixture.setWriteResult({ state: 'unknown' }); redraw(x => x + 1) }}>Uncertain next action</button>
      <button onClick={() => fixture.invalidate()}>Revoke context</button></nav>
    <MeetingLeaf owner={fixture.owner} />
  </main>
}
const root = createRoot(document.getElementById('root')!)
root.render(<FixtureApp />)
declare global { interface Window { __VENUE_MEETING_FIXTURE__: {
  state(): { writes: number; statuses: number; reads: number; phase: string; time: number; subscriptions: ReturnType<typeof fixture.subscriptions>; focuses: number }
  conversation(): void; revoke(): void; dispose(): void
} } }
window.__VENUE_MEETING_FIXTURE__ = {
  state: () => ({ writes: fixture.writes.length, statuses: fixture.statuses.length, reads: fixture.reads.length,
    phase: fixture.owner.read().phase, focuses: fixture.focuses.length, time: fixture.time(), subscriptions: fixture.subscriptions() }),
  conversation() { fixture.patch({ invitation: { id: fixtureIds.invitation, version: '2', status: 'accepted', expiresAtMs: fixture.time() + 500_000, allowedActions: ['cancel'] }, invitationActions: ['cancel'],
    sharedPlaceMessageId: 'fixture-1', conversation: { id: 'fixture-conversation', earlierCursor: null }, messages: [
      { id: 'fixture-1', authorId: fixtureIds.actor, body: 'I’m near Main Gate. Meet here?', timeLabel: '8m ago' },
      { id: 'fixture-2', authorId: fixtureIds.friend, body: 'On my way.', timeLabel: '2m ago' },
    ] }); void fixture.owner.refresh().then(() => fixture.owner.surface('conversation', false)) },
  revoke: fixture.invalidate,
  dispose() { root.unmount(); fixture.dispose() },
}
