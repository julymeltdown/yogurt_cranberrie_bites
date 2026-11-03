// @vitest-environment jsdom
import { act, cleanup, render, screen, within } from '@testing-library/react'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { MuralSyncPanel } from '../../MuralSyncPanel'
import { delta, fixture, pixel } from '../fixture'

beforeEach(() => {
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(null)
  vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} })
})
afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.unstubAllGlobals() })
it('keeps synchronization and original-result warnings inside the labeled wall while gap recovery blocks editing', async () => {
  const f = fixture(); await f.start()
  f.sync.editor.select(1, 1); await f.sync.editor.confirm()
  render(<MuralSyncPanel sync={f.sync} />)
  const window = within(screen.getByRole('region', { name: /^Wall$/ }))
  expect(window.getByRole('status', { name: 'Original placement status' }).textContent).toContain('no new placement or automatic replay')
  act(() => { f.sync.receiveDelta(f.sync.lease()!, delta(11, [pixel(12)])) })
  expect(window.getByRole('status', { name: 'Wall synchronization status' }).textContent).toContain('out of order')
  expect((window.getByRole('button', { name: 'Confirm pixel' }) as HTMLButtonElement).disabled).toBe(true)
  expect(window.getByLabelText('X').closest('fieldset')?.disabled).toBe(true)
  expect(f.requests.filter(r => r.kind === 'place')).toHaveLength(1)
  act(f.dispose)
})
