// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { MuralEditor } from '../../MuralEditor'
import { fixture } from '../fixture'
import type { CommandResult } from '../../contract'

beforeEach(() => {
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(null)
  vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} })
})
afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.unstubAllGlobals() })
it('keeps keyboard selection local and exposes pending confirmation without enabling a second write', async () => {
  let resolve!: (value: CommandResult) => void
  const f = fixture({ place: () => new Promise(r => { resolve = r }) })
  render(<MuralEditor controller={f.controller} />)
  const canvas = screen.getByRole('group', { name: 'Mural pixel preview' })
  fireEvent.keyDown(canvas, { key: 'ArrowRight' }); fireEvent.keyDown(canvas, { key: 'Enter' })
  expect(f.places).toHaveLength(0)
  expect(screen.getByLabelText('X').getAttribute('type')).toBe('number')
  const confirm = screen.getByRole('button', { name: 'Confirm pixel' }) as HTMLButtonElement
  fireEvent.click(confirm)
  expect(confirm.disabled).toBe(true); expect(confirm.getAttribute('aria-busy')).toBe('true')
  expect((screen.getByRole('button', { name: 'Check original command' }) as HTMLButtonElement).disabled).toBe(true)
  fireEvent.click(confirm); expect(f.places).toHaveLength(1)
  await act(async () => resolve({ kind: 'unknown' }))
  expect(screen.getByRole('status', { name: 'Wall command status' }).textContent).toContain('Outcome unknown')
  expect(confirm.disabled).toBe(true)
  expect(screen.getAllByRole('button', { name: /^Color \d+ #/ })).toHaveLength(16)
  expect(f.controller.read().snapshot?.revision).toBe(10)
  act(f.dispose)
})
