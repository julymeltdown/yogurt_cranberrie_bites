// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { StrictMode } from 'react'
import { afterEach, describe, expect, it } from 'vitest'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { AvatarEditor } from '../AvatarEditor'
import { harness, original } from './fixtures/harness'
import { optionHarness } from './option-previews/harness'

const owned: { dispose(): void }[] = []
afterEach(() => { cleanup(); owned.splice(0).forEach(h => h.dispose()) })
const puts = (h: { requests: { method: string; body: unknown }[] }) => h.requests.filter(r => r.method === 'PUT')
const disclosure = () => screen.getByRole('tabpanel').querySelector('summary')!
async function setup() {
  const h = harness(); owned.push(h); const view = render(<StrictMode><AvatarEditor controller={h.controller} /></StrictMode>)
  await screen.findByRole('tab', { name: 'Presets' }); fireEvent.click(screen.getByRole('tab', { name: 'Presets' }))
  await waitFor(() => expect(screen.getByRole('button', { name: 'Try fixture alternate · full outfit' })).toBeEnabled())
  return { ...h, ...view }
}
describe('compact Customize presentation', () => {
  it('shows one visual selection surface and keeps ID controls in an explicit native disclosure', async () => {
    const h = await setup()
    expect(screen.getAllByRole('group', { name: /full-outfit choices/ })).toHaveLength(1)
    expect(screen.getByRole('button', { name: 'fixture alternate' })).not.toBeVisible()
    const summary = disclosure(); expect(summary.parentElement).not.toHaveAttribute('open')
    fireEvent.click(summary); expect(summary.parentElement).toHaveAttribute('open')
    expect(screen.getByRole('button', { name: 'fixture unsupported' })).toBeDisabled()
    fireEvent.click(screen.getByRole('button', { name: 'fixture alternate' }))
    expect(h.controller.read().draft?.item_ids.hair).toBe('hair_second'); expect(puts(h)).toHaveLength(0)
    fireEvent.click(summary); expect(summary.parentElement).not.toHaveAttribute('open')
    fireEvent.click(screen.getByRole('button', { name: 'Reset draft' })); expect(h.controller.read().draft).toEqual(original)
  })
  it('retains category keyboard navigation and disclosed native selections without changing the main preview merely by browsing', async () => {
    const h = await setup(), main = h.controller.read().preview.canvas
    fireEvent.click(screen.getByRole('tab', { name: 'Hair' })); expect(screen.getByRole('combobox')).not.toBeVisible()
    for (const [previous, next] of [['Hair','Eyes'],['Eyes','Outfit'],['Outfit','Body'],['Body','Accessory'],['Accessory','Tones'],['Tones','Presets']] as const) {
      fireEvent.keyDown(screen.getByRole('tab', { name: previous }), { key: 'ArrowRight' })
      expect(screen.getByRole('tab', { name: next })).toHaveAttribute('aria-selected', 'true')
      if (next !== 'Presets') expect(screen.getByRole('combobox')).not.toBeVisible()
    }
    expect(h.controller.read().preview.canvas).toBe(main); expect(h.controller.read().draft).toEqual(original)
    fireEvent.click(screen.getByRole('tab', { name: 'Tones' })); fireEvent.click(disclosure()); expect(screen.getByRole('combobox', { name: 'Skin tone' })).toHaveValue('tone_fixture')
    expect(screen.getByRole('option', { name: 'tone unsupported' })).toBeDisabled()
    fireEvent.click(screen.getByRole('tab', { name: 'Accessory' })); fireEvent.click(disclosure())
    fireEvent.change(screen.getByRole('combobox', { name: 'accessory' }), { target: { value: '' } })
    expect(h.controller.read().draft?.item_ids.accessory).toBeNull(); expect(puts(h)).toHaveLength(0)
  })
  it('retains paged option leases, keyboard focus and synchronous privacy/unmount cleanup', async () => {
    const h = optionHarness(); owned.push(h); const view = render(<AvatarEditor controller={h.controller} />)
    await screen.findByRole('tab', { name: 'Presets' }); fireEvent.click(screen.getByRole('tab', { name: 'Presets' }))
    await waitFor(() => expect(view.container.querySelectorAll('.mcr-avatar-option canvas')).toHaveLength(3))
    const main = h.controller.read().preview.canvas, stale = [...view.container.querySelectorAll<HTMLCanvasElement>('.mcr-avatar-option canvas')]
    const next = screen.getByRole('button', { name: 'Next outfit choices' }); next.focus(); fireEvent.click(next)
    expect(next).toHaveFocus(); expect(stale.every(canvas => canvas.width === 0 && !canvas.isConnected)).toBe(true)
    await waitFor(() => expect(screen.getByRole('button', { name: 'Try option 4 · full outfit' })).toBeEnabled())
    expect(h.controller.read().preview.canvas).toBe(main); expect(h.ports[0]!.leases.size).toBeLessThanOrEqual(4); expect(puts(h)).toHaveLength(0)
    act(() => h.privacy.changePolicy(h.privacy.capture(), 'suspended'))
    expect(view.container.querySelectorAll('canvas')).toHaveLength(0); expect(h.ports[0]!.leases.size).toBe(0)
    view.unmount(); expect(h.ports.every(port => port.disposed)).toBe(true)
  })
  it.each(['denied', 'conflict', 'uncertain'] as const)('keeps the %s draft and recovery controls reachable without replaying a write', async mode => {
    const h = await setup(); fireEvent.click(screen.getByRole('button', { name: 'Try fixture alternate · full outfit' })); h.setMode(mode)
    fireEvent.click(screen.getByRole('button', { name: 'Save outfit' }))
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent(mode === 'denied' ? 'Your outfit was not saved.' : mode === 'conflict' ? 'changed elsewhere' : 'save result is unknown'))
    expect(h.controller.read().draft?.item_ids.hair).toBe('hair_second'); expect(puts(h)).toHaveLength(1)
    if (mode !== 'denied') {
      expect(screen.getByRole('button', { name: 'Save outfit' })).toBeDisabled()
      h.setMode('ok'); h.setServer({ revision: 4, config: original }); fireEvent.click(screen.getByRole('button', { name: 'Check latest saved outfit' }))
      await screen.findByRole('button', { name: 'Keep my draft' }); fireEvent.click(screen.getByRole('button', { name: 'Keep my draft' }))
      expect(puts(h)).toHaveLength(1); fireEvent.click(screen.getByRole('button', { name: 'Save outfit' }))
      await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Outfit saved.'))
      expect(puts(h)[1]!.body).toMatchObject({ expected_revision: 4 })
    }
  })
})
