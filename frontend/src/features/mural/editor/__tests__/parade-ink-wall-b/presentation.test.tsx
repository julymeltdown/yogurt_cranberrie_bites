// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { MuralEditor } from '../../MuralEditor'
import { toolsFixture, snapshot } from '../wall-tools-b/fixture'

let f: ReturnType<typeof toolsFixture>
beforeEach(async () => {
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(null)
  vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} })
  f = toolsFixture(); await f.ready()
})
afterEach(() => { cleanup(); f.dispose(); vi.restoreAllMocks(); vi.unstubAllGlobals() })

it('retains every server color and real preview while secondary viewing controls remain independently usable', () => {
  const { container } = render(<MuralEditor controller={f.controller} />)
  expect(container.querySelectorAll('canvas')).toHaveLength(1)
  expect(screen.getAllByRole('button', { name: /^Color \d+ #/ })).toHaveLength(16)
  fireEvent.click(screen.getByRole('button', { name: 'Zoom in' }))
  expect(screen.getByLabelText('Zoom level').textContent).toBe('2×')
  fireEvent.click(screen.getByRole('button', { name: 'Fit wall' }))
  expect(screen.getByLabelText('Zoom level').textContent).toBe('1×')
  fireEvent.click(screen.getByRole('button', { name: 'Pan wall' }))
  fireEvent.keyDown(screen.getByRole('group', { name: 'Mural pixel preview' }), { key: 'ArrowRight' })
  expect(f.controller.read().selection).toBeNull()
  expect(f.actions).toHaveLength(0); expect(f.pixels).toHaveLength(0)
})

it('moving the fill action keeps keyboard preview separate from the explicit one-command confirmation', async () => {
  render(<MuralEditor controller={f.controller} />)
  fireEvent.click(screen.getByRole('button', { name: 'Fill preview' }))
  fireEvent.click(screen.getByRole('button', { name: 'Color 2 #A22635' }))
  act(() => { f.controller.select(10, 10) })
  const surface = screen.getByRole('group', { name: 'Mural pixel preview' })
  fireEvent.keyDown(surface, { key: 'Enter' })
  expect(f.actions).toHaveLength(0)
  expect(screen.getByLabelText('Selection preview').textContent).toContain('16 changed cells')
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Confirm fill' })) })
  expect(f.actions).toHaveLength(1)
})

it('unknown fill recovery stays outside help and never replays the original action', async () => {
  f.dispose(); f = toolsFixture({ port: { submit: async () => ({ kind: 'unknown' }), check: async () => ({ kind: 'unknown' }) } }); await f.ready()
  render(<MuralEditor controller={f.controller} />)
  fireEvent.click(screen.getByRole('button', { name: 'Fill preview' }))
  fireEvent.click(screen.getByRole('button', { name: 'Color 2 #A22635' }))
  act(() => { f.controller.select(10, 10) })
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Confirm fill' })) })
  const check = screen.getByRole('button', { name: 'Check original command' })
  expect(check.closest('details')).toBeNull()
  expect((screen.getByRole('button', { name: 'Confirm fill' }) as HTMLButtonElement).disabled).toBe(true)
  await act(async () => { fireEvent.click(check) })
  expect(f.actions).toHaveLength(1); expect(f.checks).toHaveLength(1)
})

it('frozen and privacy transitions disable writing and remove the actual canvas without hiding viewing tools prematurely', () => {
  f.controller.acceptSnapshot(f.controller.scope, snapshot({ state: 'FROZEN' }))
  const { container } = render(<MuralEditor controller={f.controller} />)
  expect((screen.getByRole('button', { name: 'Fill preview' }) as HTMLButtonElement).disabled).toBe(true)
  expect((screen.getByRole('button', { name: 'Pan wall' }) as HTMLButtonElement).disabled).toBe(false)
  expect((screen.getByRole('button', { name: 'Fit wall' }) as HTMLButtonElement).disabled).toBe(false)
  act(() => { f.session.replace(f.session.read().generation, null) })
  expect(container.querySelector('canvas')).toBeNull()
  expect(screen.queryByRole('button', { name: /^Color 0 / })).toBeNull()
})
