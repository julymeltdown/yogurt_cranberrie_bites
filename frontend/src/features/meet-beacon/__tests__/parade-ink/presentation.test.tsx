// @vitest-environment jsdom
import { act, cleanup, render, screen, waitFor } from '@testing-library/react'
import { afterEach, expect, it } from 'vitest'
import { MeetingBeaconDetail } from '../../ui/MeetingBeacon'
import { beaconId, harness } from './harness'

const owned: Awaited<ReturnType<typeof harness>>[] = []
afterEach(() => { cleanup(); owned.splice(0).forEach(h => h.dispose()) })
it('renders only qualified private guidance and clears it when audience authority is withdrawn', async () => {
  const h = await harness(); owned.push(h); await h.auth.checkAdmission(); await h.controller.select(beaconId)
  render(<MeetingBeaconDetail controller={h.controller} />)
  await screen.findByText(h.beacon.meeting.instructions)
  expect(screen.getByText('Approved party members only')).toBeTruthy()
  act(() => h.setAuthority(null))
  await waitFor(() => expect(screen.queryByText(h.beacon.meeting.instructions)).toBeNull())
  expect(screen.getByText(/Previous private instructions and photos have been cleared/)).toBeTruthy()
})

it('never presents an expired beacon as current meeting guidance', async () => {
  const h = await harness(); owned.push(h); await h.auth.checkAdmission(); await h.controller.select(beaconId)
  act(() => h.setTime(Date.parse(h.beacon.meeting.expires_at) + 1))
  render(<MeetingBeaconDetail controller={h.controller} />)
  expect(screen.queryByText(h.beacon.meeting.instructions)).toBeNull()
  expect(screen.getByText(/Avatar position is virtual and does not confirm arrival/)).toBeTruthy()
})
