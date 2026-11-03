// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest'
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { WorldRooms } from '../ui/WorldRooms'
import { WorldNearby } from '../../world-nearby/WorldNearby'
import { harness, initial, uid } from './fixtures/harness'
const owned: ReturnType<typeof harness>[] = []
afterEach(() => { cleanup(); owned.splice(0).forEach(h => h.dispose()) })
async function setup() { const h = harness(); owned.push(h); await h.live(); return h }
describe('FE18 accessible room and authorized nearby UI', () => {
  it('shows capacity/availability and explicit switch confirmation, never fabricated occupancy', async () => {
    const h = await setup(); render(<MemoryRouter><WorldRooms controller={h.controller} /></MemoryRouter>)
    await screen.findByRole('button', { name: 'Switch to SECOND ROOM' }); expect(screen.getAllByText('Capacity 50 · Available')[0]).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Switch to SECOND ROOM' })); await screen.findByText(/Switching to SECOND ROOM/)
    expect(screen.getAllByText('MAIN STAGE')[0]).toBeTruthy(); await act(async () => { h.reject() }); await screen.findByRole('alert'); expect(screen.getByText(/confirmed room remains unchanged/)).toBeTruthy()
    expect(document.querySelector('img,canvas')).toBeNull()
  })
  it('projects delta removal in both authorized sink and DOM, then clears on same-identity policy cleanup', async () => {
    const h = await setup(); render(<MemoryRouter><WorldNearby controller={h.controller} /></MemoryRouter>)
    const others = screen.getByRole('region', { name: 'Other people in this world' }); expect(within(others).getByRole('link', { name: 'Ghost' })).toBeTruthy()
    act(() => h.receive({ protocolVersion: '1.0.0', type: 'delta', streamId: initial.streamId, roomEpoch: initial.roomEpoch, streamSeq: 1, serverTick: 2, ackInputSeq: 0, upserts: [], removes: [uid(2)] }))
    expect(screen.queryByRole('link', { name: 'Ghost' })).toBeNull(); expect(h.projected.at(-1)).not.toContain(uid(2))
    act(() => { h.auth.privacy.changePolicy(h.auth.privacy.capture(), 'block') })
    expect(screen.queryByRole('link', { name: 'Jet' })).toBeNull(); expect(screen.queryByText('Raven')).toBeNull(); expect(screen.getByText(/unavailable until your current world access/)).toBeTruthy()
  })
  it('clears remote names through epoch replacement and exposes self-only empty state', async () => {
    const h = await setup(); render(<MemoryRouter><WorldNearby controller={h.controller} /></MemoryRouter>)
    act(() => h.receive({ ...initial, cause: 'EPOCH_CHANGED', streamId: uid(501), roomEpoch: uid(502), players: [initial.players[0]] }))
    expect(screen.queryByRole('link', { name: 'Ghost' })).toBeNull(); expect(screen.queryByRole('link', { name: 'Jet' })).toBeNull(); expect(screen.getByRole('region', { name: 'You in this world' })).toBeTruthy()
    expect(screen.getAllByText('No other people are currently visible to you.').length).toBeGreaterThan(0)
  })
  it('clears the DOM on suspension rather than retaining stale people', async () => {
    const h = await setup(); render(<MemoryRouter><WorldNearby controller={h.controller} /></MemoryRouter>)
    act(() => h.replication.setVisible(false)); expect(screen.queryByRole('link', { name: 'Ghost' })).toBeNull(); expect(screen.getByText(/The world is paused. The previous list has been cleared/)).toBeTruthy()
  })
})
