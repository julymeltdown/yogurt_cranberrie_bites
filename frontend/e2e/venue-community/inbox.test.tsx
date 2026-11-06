// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { VenueCommunityFeatures } from '../../src/app/venue-community-entry/provider'
import { VenueConversationPanel, VenueInvitationPanel } from '../../src/app/venue-community-entry/routes'
import { fixtureIds, invitationView, meetingFixture } from './fixture'

const fixtures: ReturnType<typeof meetingFixture>[] = []
afterEach(() => { cleanup(); for (const f of fixtures.splice(0)) f.dispose() })

const DAY_MS = 24 * 60 * 60 * 1000
function inboxFixture() {
  const f = meetingFixture(); fixtures.push(f)
  const now = f.time(), target = f.inviteIntent, presentation = f.presentation
  f.setInbox({ INCOMING: [
    { invitation: invitationView(fixtureIds.invitation, 'pending', now + DAY_MS, ['accept', 'decline']), target, presentation },
    { invitation: invitationView(fixtureIds.invitation2, 'pending', now + DAY_MS, ['accept', 'decline']), target, presentation },
    { invitation: invitationView(fixtureIds.invitation3, 'accepted', now + DAY_MS, []), target, presentation },
    { invitation: invitationView(fixtureIds.invitation4, 'expired', now - 1, []), target, presentation },
  ], OUTGOING: [
    { invitation: invitationView(fixtureIds.invitation5, 'pending', now + DAY_MS, ['cancel']), target, presentation },
    { invitation: invitationView(fixtureIds.invitation6, 'accepted', now + DAY_MS, ['cancel']), target, presentation },
  ] })
  return f
}
function renderRoute(f: ReturnType<typeof meetingFixture>, path = `/meet/invitations/${fixtureIds.invitation}`) {
  let key = 0
  return render(<MemoryRouter initialEntries={[path]}><Routes>
    <Route path="/meet/invitations/:invitationId" element={<VenueInvitationPanel />} />
    <Route path="/meet/invitations/:invitationId/conversation" element={<VenueConversationPanel />} />
  </Routes></MemoryRouter>, { wrapper: ({ children }) => <VenueCommunityFeatures auth={f.auth} release={f.release}
    newCommandKey={() => `00000000-0000-4000-8000-${String(++key).padStart(12, '0')}`}>{children}</VenueCommunityFeatures> })
}
const openList = async (f: ReturnType<typeof meetingFixture>) => {
  const rendered = renderRoute(f)
  fireEvent.click(await screen.findByRole('button', { name: 'All invitations' }))
  await screen.findByRole('heading', { name: 'Invitations' })
  await waitFor(() => expect(f.inboxReads.length).toBeGreaterThan(0))
  await screen.findByText('Party invitation policy')
  return rendered
}

describe('invitation list screen', () => {
  it('opens the inbox from the granted detail route without issuing a mutating request', async () => {
    const f = inboxFixture(); await openList(f)
    expect(f.inboxReads[0]?.direction).toBe('INCOMING')
    expect(f.writes).toHaveLength(0)
    expect(screen.getByText('Invitee consent: only the invited friend can accept or decline.')).toBeTruthy()
    expect(screen.getByText('8: the party is full at 8 people.', { exact: false })).toBeTruthy()
    expect(screen.getByText('24-hour expiry from the server-issued invitation time.')).toBeTruthy()
  })
  it('shows pending, accepted, declined, cancelled and expired rows from producer state', async () => {
    const f = inboxFixture(); const rendered = await openList(f)
    expect(rendered.container.querySelectorAll('[data-display-status="pending"]')).toHaveLength(2)
    expect(rendered.container.querySelectorAll('[data-display-status="accepted"]')).toHaveLength(1)
    expect(rendered.container.querySelector('[data-display-status="expired"]')).toBeTruthy()
    expect(screen.getByText('Expired after 24 hours')).toBeTruthy()
    fireEvent.click(screen.getByRole('tab', { name: 'Outgoing' }))
    await waitFor(() => expect(f.inboxReads.at(-1)?.direction).toBe('OUTGOING'))
    expect(rendered.container.querySelectorAll('[data-display-status="pending"]')).toHaveLength(1)
    expect(rendered.container.querySelectorAll('[data-display-status="accepted"]')).toHaveLength(1)
  })
  it('never relabels a terminal producer state as expired', async () => {
    const f = inboxFixture(), past = f.time() - 1, target = f.inviteIntent, presentation = f.presentation
    f.setInbox({ INCOMING: [
      { invitation: invitationView(fixtureIds.invitation, 'accepted', past, ['cancel']), target, presentation },
      { invitation: invitationView(fixtureIds.invitation2, 'declined', past, []), target, presentation },
      { invitation: invitationView(fixtureIds.invitation3, 'cancelled', past, []), target, presentation },
      { invitation: invitationView(fixtureIds.invitation4, 'pending', past, ['accept']), target, presentation },
    ] })
    const rendered = await openList(f)
    expect(rendered.container.querySelector(`[data-invitation-id="${fixtureIds.invitation}"]`)?.getAttribute('data-display-status')).toBe('accepted')
    expect(rendered.container.querySelector(`[data-invitation-id="${fixtureIds.invitation2}"]`)?.getAttribute('data-display-status')).toBe('declined')
    expect(rendered.container.querySelector(`[data-invitation-id="${fixtureIds.invitation3}"]`)?.getAttribute('data-display-status')).toBe('cancelled')
    expect(rendered.container.querySelector(`[data-invitation-id="${fixtureIds.invitation4}"]`)?.getAttribute('data-display-status')).toBe('expired')
    expect(rendered.container.querySelector(`[data-invitation-id="${fixtureIds.invitation}"]`)?.textContent).toContain('Expiry no longer applies.')
  })
  it('accepts and declines only through the explicit row actions', async () => {
    const f = inboxFixture(); await openList(f)
    fireEvent.click(screen.getAllByRole('button', { name: 'Accept' })[0]!)
    await waitFor(() => expect(f.writes).toHaveLength(1))
    expect(f.writes[0]?.command.kind).toBe('accept')
    await screen.findByText('Action applied.')
    fireEvent.click(screen.getByRole('button', { name: 'Decline' }))
    await waitFor(() => expect(f.writes).toHaveLength(2))
    expect(f.writes[1]?.command.kind).toBe('decline')
    expect(screen.getAllByText('Action applied.').length).toBeGreaterThan(0)
  })
  it('renders full capacity when the adapter supplies it and never disables server rights itself', async () => {
    const f = inboxFixture(); f.setInboxCapacity({ limit: 8, joined: 8 }); await openList(f)
    expect(screen.getByText('8 of 8 party places · Full')).toBeTruthy()
    expect(screen.getAllByRole('button', { name: 'Accept' })).toHaveLength(2)
  })
  it('surfaces a server denial truthfully and keeps the producer status', async () => {
    const f = inboxFixture(); await openList(f)
    f.setWriteResult({ state: 'rejected', reason: 'denied' })
    fireEvent.click(screen.getAllByRole('button', { name: 'Accept' })[0]!)
    await waitFor(() => expect(f.writes).toHaveLength(1))
    await screen.findByText('The server denied this action. Current permissions and invitation status were refreshed.')
    expect(document.querySelector(`[data-invitation-id="${fixtureIds.invitation}"]`)?.getAttribute('data-display-status')).toBe('pending')
  })
  it('surfaces a denied host revoke on the outgoing row', async () => {
    const f = inboxFixture(); await openList(f)
    fireEvent.click(screen.getByRole('tab', { name: 'Outgoing' }))
    await waitFor(() => expect(f.inboxReads.at(-1)?.direction).toBe('OUTGOING'))
    f.setWriteResult({ state: 'rejected', reason: 'denied' })
    const row = document.querySelector(`[data-invitation-id="${fixtureIds.invitation6}"]`)
    expect(row).toBeTruthy()
    fireEvent.click(within(row as HTMLElement).getByRole('button', { name: 'Cancel invitation' }))
    await waitFor(() => expect(f.writes).toHaveLength(1))
    await screen.findByText('The server denied this action. Current permissions and invitation status were refreshed.')
    expect(document.querySelector(`[data-invitation-id="${fixtureIds.invitation6}"]`)?.getAttribute('data-display-status')).toBe('accepted')
  })
  it('shows an unavailable row when the producer omits the authorized participant', async () => {
    const f = inboxFixture()
    f.setInbox({ INCOMING: [{ invitation: invitationView(fixtureIds.invitation, 'pending', f.time() + 600_000, ['accept']), target: null, presentation: null }] })
    await openList(f)
    expect(screen.getByText('Participant unavailable')).toBeTruthy()
    expect(screen.queryByRole('button', { name: 'Accept' })).toBeNull()
  })
  it('fails closed when the inbox read is denied', async () => {
    const f = inboxFixture(); f.denyInboxReads()
    renderRoute(f)
    fireEvent.click(await screen.findByRole('button', { name: 'All invitations' }))
    await screen.findByText('Invitations are unavailable. The server did not authorize the current read.')
    expect(f.writes).toHaveLength(0)
  })
})

describe('invitation detail and conversation screens', () => {
  it('opens the invitation detail with GET reads only', async () => {
    const f = inboxFixture(); renderRoute(f, `/meet/invitations/${fixtureIds.invitation}`)
    await screen.findByRole('heading', { name: /Meet at Main Gate/ })
    expect(f.writes).toHaveLength(0); expect(f.reads.length).toBeGreaterThan(0)
  })
  it('opens an authorized friend conversation with no invitation and no write', async () => {
    const f = inboxFixture(); f.patch({ canSend: true, invitation: null, invitationActions: [] })
    const { landmark: _association, ...noPlace } = f.intent
    void _association
    f.patch({ selection: noPlace, landmarkLabel: null, landmarkDescription: null })
    renderRoute(f, `/meet/invitations/${fixtureIds.invitation}/conversation`)
    await screen.findByRole('heading', { name: 'Meet at Main Gate' })
    expect(f.writes).toHaveLength(0)
    expect(screen.getByText(/Messaging is authorized by your current friendship/)).toBeTruthy()
  })
  it('never treats an invitation id as conversation permission', async () => {
    const f = inboxFixture(); f.patch({ canSend: false, invitationActions: [],
      invitation: invitationView(fixtureIds.invitation, 'accepted', f.time() + 600_000, ['cancel']) })
    renderRoute(f, `/meet/invitations/${fixtureIds.invitation}/conversation`)
    await screen.findByText(/Sending is currently unavailable for this context/)
    expect(screen.queryByRole('textbox', { name: 'Message' })).toBeNull()
    expect(screen.queryByRole('button', { name: 'Send message' })).toBeNull()
    expect(f.writes).toHaveLength(0)
  })
})
