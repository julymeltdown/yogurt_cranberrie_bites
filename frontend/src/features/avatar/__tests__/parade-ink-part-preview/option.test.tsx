// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeAll, expect, it, vi } from 'vitest'
import type { MotionArt } from '../../../../entities/avatar/runtime/motion-adoption/art'
import { artwork, fixture, part } from '../../../../entities/avatar/runtime/motion-adoption/__tests__/parade-ink-part-preview/fixture'
import { MotionOption } from '../../motion-adoption/MotionOption'
import { MotionPartPreview } from '../../motion-adoption/part-preview/MotionPartPreview'

let art: MotionArt
const stops: (() => void)[] = []
beforeAll(async () => { art = await artwork() })
afterEach(() => { cleanup(); stops.splice(0).forEach(fn => fn()); vi.restoreAllMocks() })
const settle = () => act(async () => { for (let i = 0; i < 30; i++) await Promise.resolve() })
async function setup() {
  const f = fixture(art); stops.push(f.runtime.dispose); const catalog = await f.ready()
  const draft = catalog.presets[0]!.config, selection = f.authority.selection(draft), onChoose = vi.fn()
  return { ...f, props: { runtime: f.runtime, draft, selection, label: 'Hair 1', pressed: true, disabled: false, onChoose } }
}
it('renders the lease canvas directly and preserves button semantics and selection action', async () => {
  const f = await setup(), choose = vi.spyOn(f.runtime, 'choose'), acquire = vi.spyOn(f.runtime, 'acquire')
  const view = render(<MotionOption {...f.props} part={part} />); await settle()
  const button = screen.getByRole('button', { name: 'Choose Hair 1' }) as HTMLButtonElement
  expect(button.getAttribute('aria-pressed')).toBe('true'); expect(button.disabled).toBe(false)
  expect(screen.getByRole('img', { name: 'Hair 1 preview' }).querySelector('canvas')).toBe(f.canvases[0])
  expect(view.container.querySelector('canvas')?.style.width).toBe('56px')
  fireEvent.click(button); expect(f.props.onChoose).toHaveBeenCalledOnce()
  expect(choose).not.toHaveBeenCalled(); expect(acquire).not.toHaveBeenCalled()
  expect(f.runtime.diagnostics()).toMatchObject({ partPreviews: 1, leases: 0, clockListeners: 0 })
})
it('keeps disabled choices disabled', async () => {
  const f = await setup(); render(<MotionOption {...f.props} part={part} disabled />); await settle()
  const button = screen.getByRole('button') as HTMLButtonElement
  expect(button.disabled).toBe(true); fireEvent.click(button); expect(f.props.onChoose).not.toHaveBeenCalled()
})
it('preserves the absent-part full-outfit choose and MotionView acquisition path', async () => {
  const f = await setup(), choose = vi.spyOn(f.runtime, 'choose'), acquire = vi.spyOn(f.runtime, 'acquire'), isolated = vi.spyOn(f.runtime, 'acquirePartPreview')
  render(<MotionOption {...f.props} />); await settle()
  expect(choose).toHaveBeenCalledOnce(); expect(acquire).toHaveBeenCalledOnce(); expect(isolated).not.toHaveBeenCalled()
  expect(f.runtime.diagnostics()).toMatchObject({ leases: 1, partPreviews: 0, composition: { entries: 1, created: 1 } })
  expect(screen.getByRole('img', { name: 'Hair 1 preview' })).toBeTruthy()
})
it('clears the previous identity synchronously on category/key changes and disposes on unmount', async () => {
  const f = await setup(), view = render(<MotionOption {...f.props} part={part} />); await settle()
  const old = f.canvases[0]!
  view.rerender(<MotionOption {...f.props} part={{ kind: 'eyes', key: 'eyes-1' }} />)
  expect(old.width).toBe(0); expect(view.container.querySelector('canvas')).toBeNull()
  await settle(); expect(f.runtime.diagnostics().partPreviews).toBe(1)
  const next = view.container.querySelector('canvas')!; view.unmount()
  expect(next.width).toBe(0); expect(f.runtime.diagnostics().partPreviews).toBe(0)
})
it('releases the old runtime before mounting a replacement', async () => {
  const f = await setup(), next = await setup(), view = render(<MotionOption {...f.props} part={part} />); await settle()
  view.rerender(<MotionOption {...next.props} part={part} />)
  expect(f.runtime.diagnostics().partPreviews).toBe(0)
  await settle(); expect(next.runtime.diagnostics().partPreviews).toBe(1)
})
it('removes all displayed pixels synchronously on owner withdrawal', async () => {
  const f = await setup(), view = render(<MotionOption {...f.props} part={part} />); await settle()
  act(() => f.revoke())
  expect(view.container.querySelector('canvas')).toBeNull(); expect(screen.getByText('Part unavailable')).toBeTruthy()
})
it('does not reacquire for equivalent part objects or unrelated draft changes', async () => {
  const f = await setup(), acquire = vi.spyOn(f.runtime, 'acquirePartPreview'), view = render(<MotionOption {...f.props} part={part} />); await settle()
  view.rerender(<MotionOption {...f.props} part={{ ...part }} />); await settle()
  expect(acquire).toHaveBeenCalledOnce()
})
it('disposes an abort-resistant late lease without attaching it after unmount', async () => {
  const f = await setup(), lease = await f.runtime.acquirePartPreview(part, { signal: new AbortController().signal })
  const canvas = lease.read()!.canvas
  let finish!: (value: typeof lease) => void
  vi.spyOn(f.runtime, 'acquirePartPreview').mockImplementation(() => new Promise(resolve => { finish = resolve }))
  const view = render(<MotionPartPreview runtime={f.runtime} part={part} label="Hair preview" />)
  view.unmount(); await act(async () => { finish(lease) })
  expect(lease.read()).toBeNull(); expect(canvas.width).toBe(0)
})
