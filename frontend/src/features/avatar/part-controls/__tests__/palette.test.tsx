// @vitest-environment jsdom
import { act } from 'react'
import { createRoot } from 'react-dom/client'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { MotionBodyPalette } from '../MotionBodyPalette'
import type { MotionEditorController } from '../../motion-adoption/controller'
import type { PublicReviewedConfig } from '../../../../entities/avatar/runtime/motion-adoption/reviewed-production'
import { motionBodyOptions } from '../../wardrobe-options/body-options'
import { paletteFixture } from './fixture'

const cleanup: (() => void)[] = []
beforeEach(() => {
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true)
  vi.stubGlobal('Image', class { src = ''; async decode() {} })
  vi.stubGlobal('URL', class extends URL { static createObjectURL() { return 'blob:palette-test'; } static revokeObjectURL() {} })
})
afterEach(async () => { await act(async () => cleanup.splice(0).reverse().forEach(fn => fn())); vi.restoreAllMocks(); vi.unstubAllGlobals() })
async function fixture(wardrobe = true, canSave = true) {
  const f = await paletteFixture(canSave); cleanup.push(f.dispose)
  if (wardrobe) expect(await f.wardrobe()).toBe(true)
  return f
}
function mount(controller: MotionEditorController<PublicReviewedConfig>) {
  const host = document.createElement('div'); document.body.append(host); const root = createRoot(host)
  act(() => root.render(<MotionBodyPalette controller={controller} />))
  let closed = false
  const unmount = () => { if (closed) return; closed = true; act(() => root.unmount()); host.remove() }
  cleanup.push(unmount); return { host, unmount }
}
const buttons = (host: HTMLElement) => [...host.querySelectorAll<HTMLButtonElement>('button[data-body-key]')]
it('renders every exact qualified chip and selects all8 body keys without changing any other coordinate or saving', async () => {
  const f = await fixture(), view = mount(f.owner.controller), options = motionBodyOptions(f.owner.controller.read().catalog!.items)
  expect(buttons(view.host)).toHaveLength(8)
  for (const [index, button] of buttons(view.host).entries()) {
    expect(button.getAttribute('aria-label')).toBe(`Skin tone ${index + 1}`)
    expect(button.querySelector('span')!.style.backgroundColor).toBe(options[index]!.color)
    const before = f.authority.selection(f.owner.controller.read().draft!)
    await act(async () => button.click())
    expect(f.authority.selection(f.owner.controller.read().draft!)).toEqual({ ...before, body: options[index]!.key })
    expect(button.getAttribute('aria-pressed')).toBe('true')
  }
  expect(f.calls.filter(call => call.method === 'PUT')).toHaveLength(0)
  await act(async () => { expect(await f.owner.controller.save()).toBe(true) })
  expect(f.calls.filter(call => call.method === 'PUT')).toHaveLength(1)
  expect(f.owner.controller.read().persisted).toEqual(f.server())
})
it('does not own start/stop/dispose, Save, Reset or Random, and view cleanup keeps Account draft and runtime', async () => {
  const f = await fixture(), original = f.owner.controller
  const controller = { ...original, start: vi.fn(original.start), stop: vi.fn(original.stop), dispose: vi.fn(original.dispose),
    save: vi.fn(original.save), reset: vi.fn(original.reset), random: vi.fn(original.random) }
  const view = mount(controller), draft = original.read().draft
  view.unmount()
  for (const method of [controller.start, controller.stop, controller.dispose, controller.save, controller.reset, controller.random]) expect(method).not.toHaveBeenCalled()
  expect(original.read().draft).toBe(draft); expect(f.runtime.isCurrent()).toBe(true)
})
it('does not convert a complete preset into a fabricated wardrobe', async () => {
  const f = await fixture(false), view = mount(f.owner.controller), draft = f.owner.controller.read().draft
  expect(view.host.textContent).toContain('Choose Dress up')
  expect(buttons(view.host).every(button => button.matches(':disabled'))).toBe(true)
  await act(async () => buttons(view.host)[7]!.click())
  expect(f.owner.controller.read().draft).toBe(draft)
})
it('shows missing metadata as unavailable without inventing colors or hiding supported body keys', async () => {
  const f = await fixture(), original = f.owner.controller.read(), catalog = original.catalog!
  const items = catalog.items.map(item => item.key === 'body-3' ? { ...item, skinPalette: { colors: [[999, 0, 0]] } } : item)
  const snapshot = { ...original, catalog: { ...catalog, items } }
  const controller = { ...f.owner.controller, read: () => snapshot }
  const view = mount(controller), missing = buttons(view.host)[3]!
  expect(buttons(view.host)).toHaveLength(8); expect(missing.matches(':disabled')).toBe(true)
  expect(missing.querySelector('span')!.style.backgroundColor).toBe('')
  expect(view.host.textContent).toContain('Some skin tones are unavailable')
  expect(f.calls.filter(call => call.method === 'PUT')).toHaveLength(0)
})
it('rejects a stale DOM click when the fresh controller state is saving', async () => {
  const f = await fixture(), ready = f.owner.controller.read(); let state = ready
  const choose = vi.fn(f.owner.controller.choose), controller = { ...f.owner.controller, choose, read: () => state }
  const view = mount(controller); state = { ...ready, phase: 'saving' }
  await act(async () => buttons(view.host)[7]!.click())
  expect(choose).not.toHaveBeenCalled(); expect(f.owner.controller.read().draft).toBe(ready.draft)
})
it('follows existing capability policy: local palette draft remains editable without a server mutation', async () => {
  const f = await fixture(true, false), view = mount(f.owner.controller)
  expect(f.authority.canSave).toBe(false)
  await act(async () => buttons(view.host)[7]!.click())
  expect(f.authority.selection(f.owner.controller.read().draft!)).toMatchObject({ body: 'body-7' })
  expect(f.calls.filter(call => call.method === 'PUT')).toHaveLength(0)
})
it('retains conflict reconciliation while changing a local tone and leaves mutation gating to the owner', async () => {
  const f = await fixture(); f.mode('conflict'); expect(await f.owner.controller.save()).toBe(false)
  const view = mount(f.owner.controller), attempts = f.calls.filter(call => call.method === 'PUT').length
  await act(async () => buttons(view.host)[7]!.click())
  expect(f.owner.controller.read()).toMatchObject({ needsReconciliation: true, notice: 'conflict' })
  expect(await f.owner.controller.save()).toBe(false)
  expect(f.calls.filter(call => call.method === 'PUT')).toHaveLength(attempts)
})
it('withdraws chips immediately after canonical privacy revocation and cannot resurrect a private draft', async () => {
  const f = await fixture(), view = mount(f.owner.controller), stale = buttons(view.host)[7]!
  await act(async () => { f.auth.privacy.changePolicy(f.scope, 'consent-revoked') })
  expect(buttons(view.host)).toHaveLength(0); expect(view.host.textContent).toContain('Skin tones are unavailable')
  await act(async () => stale.click())
  expect(f.owner.controller.read().draft).toBeNull(); expect(f.calls.filter(call => call.method === 'PUT')).toHaveLength(0)
})
