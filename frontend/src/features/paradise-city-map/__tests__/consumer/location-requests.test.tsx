// @vitest-environment jsdom
import { afterEach, beforeAll, expect, it, vi } from 'vitest'
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { ParadiseCityMap } from '../../ui/ParadiseCityMap'
import { harness } from './harness'
import type { VenueCapabilities } from '../../ports'
const owners: ReturnType<typeof harness>[] = []
beforeAll(() => { globalThis.ResizeObserver = class { observe() {} unobserve() {} disconnect() {} } })
afterEach(() => { cleanup(); for (const h of owners.splice(0)) h.dispose() })
function setup() {
  const h = harness(); owners.push(h)
  const decide = vi.fn<NonNullable<VenueCapabilities['decideLocationRequest']>>(async () => ({ state: 'applied' }))
  h.capabilities.decideLocationRequest = decide
  const incoming = { id: 'request-in', requesterId: 'friend-test', subjectId: 'actor-test', revision: 8, expiresAtMs: h.snapshot.serverNowMs! + 10000 }
  const outgoing = { id: 'request-out', requesterId: 'actor-test', subjectId: 'friend-test', revision: 9, expiresAtMs: h.snapshot.serverNowMs! + 10000 }
  h.publish({ ...h.snapshot, locationRequests: { incoming: [incoming], outgoing: [outgoing] } })
  render(<ParadiseCityMap controller={h.controller} renderPortrait={() => null} />)
  fireEvent.click(screen.getByRole('tab', { name: 'FRIENDS' }))
  return { h, decide, incoming, outgoing }
}
it('shows current incoming requests and sends a single explicit approval', async () => {
  const { h, decide } = setup(); expect(screen.getByText('inkparade asked for your location.')).toBeTruthy(); expect(decide).not.toHaveBeenCalled()
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'APPROVE' })) })
  expect(decide).toHaveBeenCalledTimes(1); expect(decide.mock.calls[0]?.slice(0, 2)).toEqual(['request-in', 'APPROVE']); expect(decide.mock.calls[0]?.[2].scope).toEqual(h.authority.scope)
})
it('allows denial of incoming and cancellation of outgoing, never the reverse', async () => {
  const { h, decide } = setup()
  await act(async () => { await h.controller.decideLocationRequest('request-in', 'CANCEL'); await h.controller.decideLocationRequest('request-out', 'APPROVE') }); expect(decide).not.toHaveBeenCalled()
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'DENY' })); await Promise.resolve() }); expect(decide.mock.calls[0]?.slice(0, 2)).toEqual(['request-in', 'DENY'])
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'CANCEL REQUEST' })) }); expect(decide.mock.calls[1]?.slice(0, 2)).toEqual(['request-out', 'CANCEL'])
})
it('removes expired and departed-person requests and cannot decide them', async () => {
  const { h, decide } = setup()
  act(() => h.publish({ ...h.snapshot, serverNowMs: h.snapshot.serverNowMs! + 10001 }))
  expect(screen.queryByRole('region', { name: 'Location requests' })).toBeNull()
  await act(async () => { await h.controller.decideLocationRequest('request-in', 'APPROVE') }); expect(decide).not.toHaveBeenCalled()
  act(() => h.publish({ ...h.snapshot, friends: [] })); expect(screen.queryByText('inkparade asked for your location.')).toBeNull()
})
it('retains an unknown decision key and recovers status without deciding again', async () => {
  const { h, decide } = setup(); decide.mockResolvedValue({ state: 'unknown' })
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'APPROVE' })) }); const original = decide.mock.calls[0]![2].key
  expect(screen.getByRole('button', { name: 'DENY' })).toHaveProperty('disabled', true)
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Check action status' })) })
  expect(decide).toHaveBeenCalledTimes(1); expect(h.calls.at(-1)?.input).toBe(original); expect(h.calls.at(-1)?.name).toBe('resolve')
})
it('withdraws request identities and buttons synchronously on privacy loss', () => {
  const { h } = setup(); act(() => h.setCurrent(false))
  expect(screen.queryByRole('button', { name: 'APPROVE' })).toBeNull(); expect(screen.queryByText('inkparade asked for your location.')).toBeNull()
})
