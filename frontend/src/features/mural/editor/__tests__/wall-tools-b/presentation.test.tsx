// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { MuralEditor } from '../../MuralEditor'
import { toolContext, toolsFixture, ids } from './fixture'

let f: ReturnType<typeof toolsFixture>
beforeEach(async () => {
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(null)
  vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} })
  f = toolsFixture(); await f.ready()
})
afterEach(() => { cleanup(); f.dispose(); vi.restoreAllMocks(); vi.unstubAllGlobals() })

it('exposes qualified tools, a static caption and server shared budget with explicit fill confirmation', async () => {
  render(<MuralEditor controller={f.controller} />)
  expect(screen.getByRole('heading', { name: 'THE BLACK PARADE' })).toBeTruthy()
  fireEvent.click(screen.getByRole('button', { name: 'Fill preview' }))
  fireEvent.click(screen.getByRole('button', { name: 'Color 2 #A22635' }))
  fireEvent.click(screen.getByText('Coordinates & help'))
  fireEvent.change(screen.getByRole('spinbutton', { name: 'X' }), { target: { value: '10' } })
  fireEvent.change(screen.getByRole('spinbutton', { name: 'Y' }), { target: { value: '10' } })
  fireEvent.submit(screen.getByRole('button', { name: 'Preview pixel' }).closest('form')!)
  expect(screen.getByLabelText('Selection preview').textContent).toContain('FILL · 16 changed cells · revision 10')
  expect(f.actions).toHaveLength(0)
  await act(async () => fireEvent.click(screen.getByRole('button', { name: 'Confirm fill' })))
  expect(f.actions).toHaveLength(1)
})

it('keyboard uses the active drawing tool, Enter never implicitly sends, and Escape clears only a preview', () => {
  render(<MuralEditor controller={f.controller} />)
  fireEvent.click(screen.getByRole('button', { name: 'Erase preview' }))
  act(() => f.controller.select(10, 10))
  const surface = screen.getByRole('group', { name: 'Mural pixel preview' })
  fireEvent.keyDown(surface, { key: 'ArrowRight' }); fireEvent.keyDown(surface, { key: 'Enter' })
  expect(f.controller.read().draft?.operation).toBe('erase')
  expect(f.controller.read().selection?.x).toBe(11); expect(f.actions).toHaveLength(0)
  fireEvent.keyDown(surface, { key: 'Escape' })
  expect(f.controller.read().draft).toBeNull(); expect(f.controller.read().selection).toBeNull()
})

it('undo is a distinct explicit confirmation and never exposes another actor target', async () => {
  f.context(toolContext({ undoTarget: { actionId: ids.operation, revision: 10, changedCells: 16, expiresAt: '2026-11-04T00:10:00Z' } }))
  await f.controller.refreshTools()
  render(<MuralEditor controller={f.controller} />)
  fireEvent.click(screen.getByRole('button', { name: 'Preview undo of my last action' }))
  expect(screen.getByRole('button', { name: 'Confirm undo' })).toBeTruthy()
  expect(screen.getByLabelText('Selection preview').textContent).toContain('UNDO · 16 changed cells')
  expect(f.actions).toHaveLength(0)
  fireEvent.click(screen.getByRole('button', { name: 'Cancel preview' }))
  expect(screen.queryByRole('button', { name: 'Confirm undo' })).toBeNull()
})
