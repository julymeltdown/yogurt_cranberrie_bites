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
  const h = setup(); fireEvent.click(screen.getByRole('button', { name: 'Share location' })); expect(h.watches).toBe(0); expect(screen.getByRole('button', { name: 'Allow phone location' })).toHaveProperty('disabled', true)
})
it('keeps a readable unavailable state and non-writing siteplan controls without backend capability', () => {
  const h = setup(); act(() => h.publish(null)); expect(screen.getByText(/Live location, check-in, meetings and notes are unavailable/)).toBeTruthy()
  fireEvent.click(screen.getByRole('button', { name: 'Select Main Gate' })); expect(screen.getByRole('button', { name: 'CHECK IN' })).toHaveProperty('disabled', true); expect(h.calls).toHaveLength(0)
})
