// @vitest-environment jsdom
import { afterEach, beforeAll, expect, it } from 'vitest'
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react'
import { ParadiseCityMap } from '../../ui/ParadiseCityMap'
import { harness, person } from './harness'

const owners: ReturnType<typeof harness>[] = []
beforeAll(() => {
  globalThis.ResizeObserver = class { observe() {} unobserve() {} disconnect() {} }
  if (!globalThis.CSS) Object.defineProperty(globalThis, 'CSS', { value: { escape: (value: string) => value }, configurable: true })
})
afterEach(() => { cleanup(); owners.splice(0).forEach(h => h.dispose()) })
function setup() { const h = harness(); owners.push(h); render(<ParadiseCityMap controller={h.controller} renderPortrait={p => <span role="img" aria-label={`${p.displayName} synthetic portrait`} />} />); return h }
it('implements keyboard local tabs and preserves one map without a mutation', () => {
  const h = setup(), tab = screen.getByRole('tab', { name: 'MAP' }); tab.focus(); fireEvent.keyDown(tab, { key: 'ArrowRight' })
  expect(screen.getByRole('tab', { name: 'FRIENDS' }).getAttribute('aria-selected')).toBe('true'); expect(screen.getAllByRole('img', { name: 'Paradise City illustrated siteplan, uncalibrated' })).toHaveLength(1); expect(h.calls).toEqual([])
})
it('opens current friend details, sends only a contextual open and restores focus on Escape', async () => {
  const h = setup(), trigger = screen.getByRole('button', { name: 'View inkparade on map' }); trigger.focus(); fireEvent.click(trigger)
  const dialog = screen.getByRole('dialog', { name: 'inkparade' }); expect(within(dialog).getByText('Phone location · updated 40s ago')).toBeTruthy()
  await act(async () => { fireEvent.click(within(dialog).getByRole('button', { name: 'MESSAGE' })) }); expect(h.opens[0]?.action).toBe('MESSAGE'); expect(h.calls).toHaveLength(0)
  fireEvent.keyDown(dialog, { key: 'Escape' }); expect(screen.queryByRole('dialog')).toBeNull(); expect(document.activeElement).toBe(trigger)
})
it('scrubs selected private DOM in the same withdrawal notification', () => {
  const h = setup(); fireEvent.click(screen.getByRole('button', { name: 'View inkparade on map' })); act(() => h.setCurrent(false))
  expect(screen.queryByRole('dialog')).toBeNull(); expect(screen.queryByRole('img', { name: 'inkparade synthetic portrait' })).toBeNull(); expect(screen.queryByText('Phone location · updated 40s ago')).toBeNull()
})
it('enables MESSAGE without location or a landmark while MEET HERE stays disabled', async () => {
  const h = setup(); act(() => h.publish({ ...h.snapshot, friends: [{ ...person(), point: null, landmarkId: null, locationSource: 'unavailable' }] }))
  fireEvent.click(screen.getByRole('tab', { name: 'FRIENDS' })); fireEvent.click(screen.getByRole('button', { name: 'inkparade' }))
  const dialog = screen.getByRole('dialog', { name: 'inkparade' }), message = within(dialog).getByRole('button', { name: 'MESSAGE' })
  expect(message).toHaveProperty('disabled', false); expect(within(dialog).getByRole('button', { name: 'MEET HERE' })).toHaveProperty('disabled', true)
  await act(async () => { fireEvent.click(message) }); expect(h.opens[0]?.action).toBe('MESSAGE'); expect(h.opens[0]).not.toHaveProperty('landmark'); expect(h.calls).toEqual([])
})
it('authors a place note with black audience/expiry selectors and sends one explicit intent', async () => {
  const h = setup(); fireEvent.click(screen.getByRole('tab', { name: 'NOTES' })); fireEvent.click(screen.getByRole('button', { name: '+ NEW NOTE' }))
  fireEvent.change(screen.getByRole('textbox', { name: 'Note text' }), { target: { value: 'Controlled note' } }); fireEvent.click(screen.getByRole('button', { name: 'EVERYONE' })); fireEvent.click(screen.getByRole('button', { name: 'END OF DAY' }))
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: '+ POST NOTE' })) })
  expect(h.calls[0]?.input).toEqual({ landmarkId: 'test-main-gate', body: 'Controlled note', audience: 'EVERYONE', expiry: 'END_OF_DAY' })
})
it('never starts phone location from render or an unchecked sharing prompt', () => {
  const h = setup(); fireEvent.click(screen.getByRole('button', { name: 'SHARE LOCATION' })); expect(h.watches).toBe(0); expect(screen.getByRole('button', { name: 'Allow phone location' })).toHaveProperty('disabled', true)
})
it('shows a failed friend gate as unavailable instead of an empty friends list', () => {
  const h = setup()
  act(() => h.setAvailability({ state: 'unavailable', reason: 'friend-gate-unavailable', readAtMs: null, ageMs: null, staleAtMs: null, refreshing: false, canRefresh: true }))
  fireEvent.click(screen.getByRole('tab', { name: 'FRIENDS' }))
  expect(screen.getByLabelText('Friends unavailable')).toBeTruthy()
  expect(screen.getByText(/friendship authority is unavailable \(503\)/)).toBeTruthy()
  expect(screen.queryByText(/No friend location is currently authorized/)).toBeNull()
  expect(screen.queryByText(/0 friends/)).toBeNull()
})
it('labels a stale snapshot as last-known and offers a refetch', async () => {
  const h = setup()
  act(() => h.setAvailability({ state: 'stale', reason: null, readAtMs: h.snapshot.serverNowMs!, ageMs: 20000, staleAtMs: h.snapshot.serverNowMs! + 10000, refreshing: false, canRefresh: true }))
  fireEvent.click(screen.getByRole('tab', { name: 'FRIENDS' }))
  expect(screen.getByText('Snapshot 20s ago')).toBeTruthy(); expect(screen.getByRole('button', { name: 'REFRESH' })).toBeTruthy()
  expect(screen.getByText('Last update · 40s ago')).toBeTruthy()
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'REFRESH' })) })
  expect(h.refreshes).toBe(1); expect(screen.queryByText('Snapshot 20s ago')).toBeNull()
})
it('reports device permission denial without starting another watch and allows a reviewed retry', async () => {
  const h = setup()
  await act(async () => { await h.controller.startPhone(true); h.deny(); await Promise.resolve(); await Promise.resolve() })
  expect(screen.getByLabelText('Phone location unavailable')).toBeTruthy()
  expect(screen.getByText('Phone permission denied')).toBeTruthy(); expect(screen.getByRole('button', { name: 'REVIEW SHARING' })).toBeTruthy()
  fireEvent.click(screen.getByRole('button', { name: 'REVIEW SHARING' })); expect(h.watches).toBe(1)
})
it('requires explicit confirmation before revoke and then removes sharing', async () => {
  const h = setup(); act(() => h.publish({ ...h.snapshot, sharing: 'active' }))
  fireEvent.click(screen.getByRole('button', { name: 'REVOKE' }))
  expect(screen.getByRole('alertdialog', { name: 'Stop sharing location' })).toBeTruthy(); expect(h.calls).toEqual([])
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'STOP SHARING' })) })
  expect(h.calls.map(call => call.name)).toEqual(['revoke']); expect(screen.getByText('Location sharing revoked')).toBeTruthy()
})
it('clusters real checked-in friends and lists exactly those actors', () => {
  const h = setup()
  act(() => h.publish({ ...h.snapshot, friends: [person('a-friend', 'A'), person('b-friend', 'B'), person('c-friend', 'C')] }))
  fireEvent.click(screen.getByRole('button', { name: 'View cluster of 3 friends' }))
  const sheet = screen.getByRole('region', { name: 'Friend cluster' }); expect(within(sheet).getByText('3 friends')).toBeTruthy()
  expect(within(sheet).getAllByRole('button').filter(button => ['A', 'B', 'C'].includes(button.textContent ?? ''))).toHaveLength(3)
})
it('keeps a readable unavailable state and non-writing siteplan controls without backend capability', () => {
  const h = setup(); act(() => h.publish(null)); expect(screen.getByText(/Live location, check-in, meetings and notes are unavailable/)).toBeTruthy()
  fireEvent.click(screen.getByRole('button', { name: 'Select Main Gate' })); expect(screen.getByRole('button', { name: 'CHECK IN' })).toHaveProperty('disabled', true); expect(h.calls).toHaveLength(0)
})
