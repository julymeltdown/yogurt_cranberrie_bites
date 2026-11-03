// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { MeetingBeaconDetail, MeetingBeaconForm } from '../ui/MeetingBeacon'
import { ExplicitArrivalControls, type ExplicitArrivalControlsProps } from '../ui/ExplicitArrivalControls'
import { beaconId, harness, partyId, sampleBeacon } from './fixtures/harness'

const owners: Awaited<ReturnType<typeof harness>>[] = []
async function setup() { const h = await harness(); owners.push(h); return h }
beforeEach(() => vi.stubGlobal('location', { origin: 'https://mcr.example.invalid' }))
afterEach(() => { cleanup(); owners.splice(0).forEach(h => h.dispose()); vi.unstubAllGlobals() })
it('authors an explicit audience/TTL form and sends a single physical acknowledgement', async () => {
  const h = await setup(); render(<MeetingBeaconForm controller={h.controller} />)
  expect(screen.getByLabelText('Reviewed place')).toHaveTextContent('Synthetic reviewed landmark')
  expect(screen.getByLabelText('Reviewed place')).not.toHaveTextContent('Synthetic virtual stage')
  fireEvent.change(screen.getByLabelText('Reviewed place'), { target: { value: h.items[1]!.place_id } })
  fireEvent.change(screen.getByLabelText('Who can see this?'), { target: { value: 'synthetic-party' } })
  fireEvent.change(screen.getByLabelText('How to find me'), { target: { value: 'SYNTHETIC marker' } })
  fireEvent.change(screen.getByLabelText('Identification'), { target: { value: 'SYNTHETIC scarf' } })
  fireEvent.change(screen.getByLabelText('Visible for (seconds)'), { target: { value: '60' } })
  fireEvent.click(screen.getByRole('checkbox'))
  expect(h.controller.read().draft.acknowledged).toBe(true)
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Share check-in' })) })
  expect(screen.getByText('Change confirmed by the server.')).toBeVisible()
  expect(h.requests.filter(r => r.method === 'POST')).toHaveLength(1)
})
it('clears obsolete text at expiry and renders a moved revision as superseding earlier guidance', async () => {
  const h = await setup(); await h.controller.select(beaconId); render(<MeetingBeaconDetail controller={h.controller} />)
  expect(screen.getByText(h.beacon.meeting.identification)).toBeVisible()
  expect(screen.getByText('Approved party members only')).toBeVisible()
  expect(screen.getByText('Beacon 1 · Place 3 · Map 1')).toBeVisible()
  act(() => h.controller.invalidate({ beaconId, revision: 2 }))
  expect(screen.queryByText(h.beacon.meeting.identification)).not.toBeInTheDocument()
  h.setBeacon({ ...h.beacon, revision: 2 })
  await act(() => h.controller.refresh())
  expect(screen.getByRole('heading', { name: 'Meeting moved or changed' })).toBeVisible()
  fireEvent.click(screen.getByRole('button', { name: 'I have reviewed this revision' }))
  expect(screen.getByRole('heading', { name: 'Current meeting' })).toBeVisible()
  act(() => h.setTime(Date.parse(h.beacon.meeting.expires_at)))
  expect(screen.getByRole('heading', { name: 'Meeting expired' })).toBeVisible()
  expect(screen.queryByText(h.beacon.meeting.identification)).not.toBeInTheDocument()
})
it('requires a deliberate revoke confirmation and removes the current details', async () => {
  const h = await setup(); await h.controller.select(beaconId); render(<MeetingBeaconDetail controller={h.controller} ownerActions />)
  fireEvent.click(screen.getByRole('button', { name: 'Revoke meeting…' })); expect(h.requests.filter(r => r.method === 'DELETE')).toHaveLength(0)
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Confirm revocation' })) })
  expect(screen.getByRole('heading', { name: 'Meeting revoked' })).toBeVisible()
  expect(screen.queryByText(h.beacon.meeting.identification)).not.toBeInTheDocument()
})
it('clears controlled private form state on canonical policy revocation', async () => {
  const h = await setup(); render(<MeetingBeaconForm controller={h.controller} />)
  fireEvent.change(screen.getByLabelText('Identification'), { target: { value: 'SYNTHETIC private scarf' } })
  act(() => { h.auth.privacy.changePolicy(h.auth.privacy.capture(), 'consent-revoked') })
  expect(screen.getByLabelText('Identification')).toHaveValue('')
  expect(screen.getByRole('button', { name: 'Share check-in' })).toBeDisabled()
})
it('reports arrival only after explicit acknowledgement; movement/re-render never sends an intent', async () => {
  const h = await setup(), b = sampleBeacon(), onIntent = vi.fn()
  let time = h.time!
  const props: ExplicitArrivalControlsProps = { beacon: b, phase: 'current', currentScope: h.auth.privacy.capture(), now: () => time,
    authorization: { scope: h.auth.privacy.capture(), partyId, beaconId, beaconRevision: b.revision, meetingRevision: 8, expiresAt: b.meeting.expires_at }, pending: false, onIntent }
  const view = render(<ExplicitArrivalControls {...props} />)
  expect(screen.getByRole('button', { name: 'I am here' })).toBeDisabled(); expect(onIntent).not.toHaveBeenCalled()
  view.rerender(<ExplicitArrivalControls {...props} />); expect(onIntent).not.toHaveBeenCalled()
  fireEvent.click(screen.getByRole('checkbox')); fireEvent.click(screen.getByRole('button', { name: 'I am here' }))
  expect(onIntent).toHaveBeenCalledExactlyOnceWith({ state: 'HERE', meeting_revision: 8, physical_presence_acknowledged: true })
  expect(screen.getByRole('button', { name: 'I am here' })).toBeDisabled()
  fireEvent.click(screen.getByRole('checkbox'))
  const next = { ...props, authorization: { ...props.authorization!, meetingRevision: 9 } }
  view.rerender(<ExplicitArrivalControls {...next} />)
  expect(screen.getByRole('checkbox')).not.toBeChecked(); expect(screen.getByRole('button', { name: 'I am here' })).toBeDisabled()
  fireEvent.click(screen.getByRole('checkbox')); time = Date.parse(b.meeting.expires_at)
  fireEvent.click(screen.getByRole('button', { name: 'I am here' })); expect(onIntent).toHaveBeenCalledTimes(1) // Click-time expiry fence.
})
it.each(['moved', 'expired', 'revoked', 'stale', 'unavailable'] as const)('disables arrival for %s guidance and absent authority', async phase => {
  const h = await setup(); render(<ExplicitArrivalControls beacon={sampleBeacon()} phase={phase} currentScope={h.auth.privacy.capture()}
    authorization={null} now={h.now} pending={false} onIntent={() => { throw new Error('Unexpected arrival') }} />)
  expect(screen.getByRole('checkbox')).toBeDisabled(); expect(screen.getByRole('button', { name: 'On my way' })).toBeDisabled()
})
