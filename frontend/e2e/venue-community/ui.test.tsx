// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, expect, it } from 'vitest'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { MeetingLeaf } from '../../src/app/venue-community-entry/view'
import { VenueCommunityFeatures, meetingConsumer } from '../../src/app/venue-community-entry/provider'
import { VenueInvitationPanel } from '../../src/app/venue-community-entry/routes'
import type { MeetingConsumerPort } from '../../src/features/paradise-city-map/ports'
import { fixtureIds, meetingFixture } from './fixture'

const fixtures: ReturnType<typeof meetingFixture>[] = []
const fixture = () => { const f = meetingFixture(); fixtures.push(f); return f }
afterEach(() => { cleanup(); for (const f of fixtures.splice(0)) f.dispose() })

it('renders a composer without an invitation; an explicit submit sends once', async () => {
  const f = fixture(); await f.owner.open(f.intent); render(<MeetingLeaf owner={f.owner} />)
  expect(screen.getByRole('heading', { name: 'Meet at Main Gate' })).toBeTruthy()
  fireEvent.change(screen.getByLabelText('Message'), { target: { value: 'At the gate now' } })
  fireEvent.click(screen.getByRole('button', { name: 'Send message' }))
  await waitFor(() => expect(f.writes).toHaveLength(1))
  await screen.findByText('At the gate now'); expect(f.writes[0]?.command.invitationId).toBeNull()
})
it('renders the authorized friend conversation without a fake landmark', async () => {
  const f = fixture(), { landmark: _association, ...noPlace } = f.intent
  void _association
  f.patch({ selection: noPlace, landmarkLabel: null, landmarkDescription: null, invitationActions: [] })
  await f.owner.open(noPlace); render(<MeetingLeaf owner={f.owner} />)
  expect(screen.getByRole('heading', { name: 'Message inkparade' })).toBeTruthy()
  expect(screen.getByRole('textbox', { name: 'Message' })).toBeTruthy()
  expect(screen.queryByText('Main Gate')).toBeNull(); expect(screen.queryByRole('button', { name: 'Invitation' })).toBeNull()
  expect(f.writes).toHaveLength(0)
})
it('shows server recipient actions and explicit accept updates the view', async () => {
  const f = fixture(); f.patch({ invitation: { id: fixtureIds.invitation, version: '1', status: 'pending', expiresAtMs: f.time() + 500_000, allowedActions: ['accept', 'decline'] }, invitationActions: ['accept', 'decline'] })
  await f.owner.open(f.inviteIntent); render(<MeetingLeaf owner={f.owner} />)
  expect(screen.getByRole('button', { name: 'Decline' })).toBeTruthy(); expect(f.writes).toHaveLength(0)
  fireEvent.click(screen.getByRole('button', { name: 'Accept' })); await screen.findByText('Meeting accepted')
  expect(f.writes[0]?.command.kind).toBe('accept')
})
it('keeps unknown send visible with only an original-status recovery action', async () => {
  const f = fixture(); f.setWriteResult({ state: 'unknown' }); await f.owner.open(f.intent); render(<MeetingLeaf owner={f.owner} />)
  fireEvent.change(screen.getByLabelText('Message'), { target: { value: 'Hello' } }); fireEvent.click(screen.getByRole('button', { name: 'Send message' }))
  await screen.findByRole('button', { name: 'Check original action' })
  expect((screen.getByRole('button', { name: 'Send message' }) as HTMLButtonElement).disabled).toBe(true)
  fireEvent.click(screen.getByRole('button', { name: 'Check original action' })); await screen.findByText('Hello')
  expect(f.writes).toHaveLength(1); expect(f.statuses).toHaveLength(1)
})
it('scrubs rendered private text when the same privacy owner blocks access', async () => {
  const f = fixture(); await f.owner.open(f.intent); render(<MeetingLeaf owner={f.owner} />)
  fireEvent.change(screen.getByLabelText('Message'), { target: { value: 'Private draft' } })
  act(() => { f.auth.privacy.changePolicy(f.scope, 'block') })
  expect(screen.queryByDisplayValue('Private draft')).toBeNull(); expect(screen.queryByText('inkparade')).toBeNull()
  expect(screen.getByText(/unavailable until current participant/)).toBeTruthy()
})
it('provides a stable snapshot through the sole primary consumer type', async () => {
  const f = fixture(), consumer = meetingConsumer(f.owner)
  expect(consumer.read()).toBeNull(); await consumer.open(f.intent)
  const snapshot = consumer.read(); expect(snapshot).toBe(consumer.read()); expect(snapshot?.active).toBe(true)
  f.owner.setDraft('Retain'); expect(consumer.read()).not.toBe(snapshot)
  expect(consumer.close()).toBe(true); expect(consumer.read()?.active).toBe(false)
})
it('a missing shell provider is truthfully unavailable and never constructs an owner', () => {
  render(<MemoryRouter initialEntries={[`/meet/invitations/${fixtureIds.invitation}`]}><Routes>
    <Route path="/meet/invitations/:invitationId" element={<VenueInvitationPanel />} />
  </Routes></MemoryRouter>)
  expect(screen.getByText(/unavailable until current participant/)).toBeTruthy()
})
it('the retained host binds one consumer and releases it with its original subscriptions', async () => {
  const f = fixture(), bindings: (MeetingConsumerPort | null)[] = []
  const bindConsumer = (consumer: MeetingConsumerPort | null) => { bindings.push(consumer) }
  const rendered = render(<VenueCommunityFeatures auth={f.auth} release={f.release} newCommandKey={() => fixtureIds.invitation} bindConsumer={bindConsumer}><p>Host</p></VenueCommunityFeatures>)
  expect(bindings).toHaveLength(1); await act(async () => { await bindings[0]!.open(f.intent) })
  rendered.unmount(); expect(bindings.at(-1)).toBeNull()
  // The separate fixture owner still has one subscription; the host-owned one is gone.
  expect(f.subscriptions()).toEqual({ clock: 1, invalidation: 1 })
})

it.each([true, false])('keeps current place outside unassociated messages and embeds only an associated message: %s', async associated => {
  const f = fixture(); f.patch({ invitation: { id: fixtureIds.invitation, version: '1', status: 'accepted', expiresAtMs: f.time() + 50000, allowedActions: ['cancel'] },
    messages: [{ id: 'visible-message', authorId: fixtureIds.actor, body: 'Meet after the show.', timeLabel: 'Now' }],
    sharedPlaceMessageId: associated ? 'visible-message' : null })
  await f.owner.open(f.intent); const result = render(<MeetingLeaf owner={f.owner} />)
  expect(result.container.querySelector('.venue-meeting-message-with-place .venue-meeting-place') !== null).toBe(associated)
  expect(result.container.querySelectorAll('.venue-meeting-conversation .venue-meeting-place')).toHaveLength(1)
})
