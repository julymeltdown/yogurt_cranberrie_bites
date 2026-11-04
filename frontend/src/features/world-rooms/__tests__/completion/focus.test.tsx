// @vitest-environment jsdom
import { afterEach, expect, it } from 'vitest'
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { WorldRooms } from '../../ui/WorldRooms'
import { WorldNearby } from '../../../world-nearby/WorldNearby'
import { harness, uid } from '../fixtures/harness'
const owned: ReturnType<typeof harness>[] = []
afterEach(() => { cleanup(); owned.splice(0).forEach(h => h.dispose()) })
async function setup() {
  const h = harness(); owned.push(h); await h.live()
  render(<MemoryRouter><WorldRooms controller={h.controller} /></MemoryRouter>)
  return h
}
it.each(['admit', 'reject'] as const)('moves focus from the completed switch to its actual %s outcome', async result => {
  const h = await setup(), button = await screen.findByRole('button', { name: 'Switch to SECOND ROOM' })
  button.focus(); fireEvent.click(button)
  await act(async () => { await h.waitForSwitch(uid(12)); h[result]() })
  const feedback = screen.getByRole('region', { name: 'Room switch status' })
  expect(document.activeElement).toBe(feedback)
  if (result === 'admit') {
    expect(feedback.textContent).toContain('You are now in SECOND ROOM.')
    expect(screen.getByText('SECOND ROOM', { selector: 'h3' }).closest('li')?.getAttribute('aria-current')).toBe('true')
  } else expect(feedback.textContent).toContain('confirmed room remains unchanged')
})
it('does not steal focus when the user moves to another control while admission is pending', async () => {
  const h = await setup(), button = await screen.findByRole('button', { name: 'Switch to SECOND ROOM' })
  button.focus(); fireEvent.click(button)
  const refresh = screen.getByRole('button', { name: 'Refresh rooms' }); refresh.focus()
  await act(async () => { await h.waitForSwitch(uid(12)); h.admit() })
  expect(document.activeElement).toBe(refresh)
})
it('names the authorized roster as a list, retaining the self/other distinction', async () => {
  const h = harness(); owned.push(h); await h.live()
  render(<MemoryRouter><WorldNearby controller={h.controller} /></MemoryRouter>)
  expect(screen.getByRole('list', { name: 'People authorized in your current virtual room' }).textContent).toContain('Ghost')
  expect(screen.getByRole('region', { name: 'You in this world' }).textContent).toContain('You')
})

it('restores a useful list focus when filtering removes the focused person', async () => {
  const h = harness(); owned.push(h); await h.live()
  render(<MemoryRouter><WorldNearby controller={h.controller} /></MemoryRouter>)
  screen.getByRole('link', { name: 'Ghost' }).focus()
  const b = h.replication.read().baseline!
  act(() => h.receive({ protocolVersion: '1.0.0', type: 'delta', streamId: b.streamId, roomEpoch: b.roomEpoch, streamSeq: 1, serverTick: 2, ackInputSeq: 0, upserts: [], removes: [uid(2)] }))
  expect(screen.queryByRole('link', { name: 'Ghost' })).toBeNull()
  expect(document.activeElement).toBe(screen.getByRole('region', { name: 'Other people in this world' }))
})
it('preserves another control focus when filtering changes the list', async () => {
  const h = harness(); owned.push(h); await h.live()
  render(<MemoryRouter><WorldNearby controller={h.controller} /></MemoryRouter>)
  screen.getByRole('link', { name: 'Ghost' }).focus()
  const rooms = screen.getByRole('link', { name: 'Choose a room' }); rooms.focus()
  const b = h.replication.read().baseline!
  act(() => h.receive({ protocolVersion: '1.0.0', type: 'delta', streamId: b.streamId, roomEpoch: b.roomEpoch, streamSeq: 1, serverTick: 2, ackInputSeq: 0, upserts: [], removes: [uid(2)] }))
  expect(document.activeElement).toBe(rooms)
})
