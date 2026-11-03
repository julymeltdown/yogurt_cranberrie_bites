// @vitest-environment jsdom
import { act, cleanup, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { MuralSyncPanel } from '../../MuralSyncPanel'
import { delta, fixture, pixel } from '../fixture'

beforeEach(() => {
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(null)
  vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} })
})
afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.unstubAllGlobals() })
it('places truthful ready status below the canvas and keeps gap/original-result warnings expanded', async () => {
  const f = fixture(); await f.start()
  const { container } = render(<MuralSyncPanel sync={f.sync} />)
  const ready = screen.getByRole('status', { name: 'Wall synchronization status' })
  expect(ready.textContent).toBe('Wall synchronized at revision 10.')
  expect(ready.getAttribute('data-phase')).toBe('ready')
  expect(container.querySelector('canvas')!.compareDocumentPosition(ready) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
  act(() => { f.sync.editor.select(1, 1) }); await act(async () => f.sync.editor.confirm())
  act(() => f.sync.receiveDelta(f.sync.lease()!, delta(11, [pixel(12)])))
  expect(ready.textContent).toContain('out of order'); expect(ready.closest('details')).toBeNull()
  expect(screen.getByRole('status', { name: 'Original placement status' }).closest('details')).toBeNull()
  expect((screen.getByRole('button', { name: 'Confirm pixel' }) as HTMLButtonElement).disabled).toBe(true)
  expect(f.requests.filter(r => r.kind === 'place')).toHaveLength(1)
  act(f.dispose)
})
