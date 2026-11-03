// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { StrictMode } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { AvatarEditor } from '../AvatarEditor'
import { configs, optionHarness, unsupported } from './option-previews/harness'
import { original } from './fixtures/harness'
import type { OptionPreview } from '../controller'

const owned: ReturnType<typeof optionHarness>[] = []
afterEach(() => { cleanup(); owned.splice(0).forEach(h => h.dispose()) })
async function ready() { const h = optionHarness(); owned.push(h); h.controller.start(); await vi.waitFor(() => expect(h.controller.read().preview.phase).toBe('ready')); return h }
async function mounted(strict = false) {
  const h = optionHarness(); owned.push(h); const child = <AvatarEditor controller={h.controller} />
  const view = render(strict ? <StrictMode>{child}</StrictMode> : child)
  await screen.findByRole('tab', { name: 'Presets' }); fireEvent.click(screen.getByRole('tab', { name: 'Presets' }))
  await vi.waitFor(() => expect(screen.getByRole('button', { name: 'Try option 1 · full outfit' })).toBeEnabled())
  return { ...h, ...view }
}
const puts = (h: ReturnType<typeof optionHarness>) => h.requests.filter(r => r.method === 'PUT')
describe('controller-owned option preview lifetime', () => {
  it('uses the current owner, three separate option leases and never steals or edits the main preview', async () => {
    const h = await ready(), owner = h.ports[0]!, main = h.controller.read().preview.canvas, results: OptionPreview[] = []
    const release = h.controller.acquireOptionPreviews(configs.slice(0, 3), (i, value) => { results[i] = value })
    await vi.waitFor(() => expect(results.filter(r => r.phase === 'ready')).toHaveLength(3))
    expect(h.ports).toHaveLength(1); expect(owner.renders).toBe(4); expect(owner.peakActive).toBe(1); expect(owner.leases.size).toBe(4)
    expect(results.every(r => r.canvas !== main)).toBe(true); expect(h.controller.read().preview.canvas).toBe(main); expect(h.controller.read().draft).toEqual(original)
    expect(puts(h)).toHaveLength(0); release(); expect(owner.leases.size).toBe(1); expect(main?.width).toBe(64)
    expect(() => h.controller.acquireOptionPreviews(configs.slice(0, 4), () => {})).toThrow(RangeError)
  })
  it('rejects invalid, unsupported and failed candidates without allocating extra renders or changing the draft', async () => {
    const h = await ready(), owner = h.ports[0]!, results: OptionPreview[] = [], before = owner.renders
    h.controller.acquireOptionPreviews([unsupported, { ...original, manifest_version: 999 }, { ...original, item_ids: { ...original.item_ids, hair: 'wrong_id' } }], (i, value) => { results[i] = value })
    expect(results.every(r => r.phase === 'unavailable')).toBe(true); expect(owner.renders).toBe(before)
    owner.fail = true; h.controller.acquireOptionPreviews([original], (i, value) => { results[i] = value })
    await vi.waitFor(() => expect(results[0]?.phase).toBe('unavailable')); expect(h.controller.read().draft).toEqual(original); expect(puts(h)).toHaveLength(0)
  })
  it('bounds ignored-abort churn to one in-flight option and disposes late obsolete results before starting the newest batch', async () => {
    const h = await ready(), owner = h.ports[0]!, received = vi.fn(); owner.held = true; owner.ignoredAbort = true
    h.controller.acquireOptionPreviews(configs.slice(0, 3), received)
    for (let n = 0; n < 40; n++) h.controller.acquireOptionPreviews([configs[n % configs.length]!], received)
    expect(owner.active).toBe(1); expect(owner.renders).toBe(2); expect(owner.pending).toHaveLength(1); expect(owner.signals.at(-1)?.aborted).toBe(true)
    const stale = owner.canvases.at(-1)!, previousReady = received.mock.calls.filter(call => call[1].phase === 'ready').length
    owner.held = false; owner.pending[0]!()
    await vi.waitFor(() => expect(received.mock.calls.filter(call => call[1].phase === 'ready')).toHaveLength(previousReady + 1)); expect(owner.active).toBe(0); expect(stale.width).toBe(0)
    expect(received.mock.calls.filter(call => call[1].phase === 'ready')).toHaveLength(previousReady + 1)
    expect(owner.renders).toBe(3); expect(owner.peakActive).toBe(1); expect(owner.leases.size).toBe(2)
  })
  it.each(['draft', 'privacy', 'identity', 'stop', 'replacement', 'dispose'] as const)('clears current and late option leases on %s', async change => {
    const h = await ready(), owner = h.ports[0]!, received = vi.fn(); owner.held = true; owner.ignoredAbort = true
    h.controller.acquireOptionPreviews([configs[1]!], received); const late = owner.canvases.at(-1)!
    if (change === 'draft') h.controller.selectPreset('option_2')
    if (change === 'privacy') h.privacy.changePolicy(h.privacy.capture(), 'suspended')
    if (change === 'identity') h.session.replace(h.session.read().generation, { userId: '00000000-0000-4000-8000-000000000099', eventId: h.session.read().eventId!, accessToken: 'new-token' })
    if (change === 'stop') h.controller.stop()
    if (change === 'replacement') await h.controller.retryPreview()
    if (change === 'dispose') h.controller.dispose()
    expect(owner.signals[1]?.aborted).toBe(true)
    owner.held = false; owner.pending[0]!(); await vi.waitFor(() => expect(late.width).toBe(0))
    expect(received.mock.calls.some(call => call[1].phase === 'ready')).toBe(false)
    expect(puts(h)).toHaveLength(0)
  })
  it('isolates a throwing recipient and supports reentrant stop during ready delivery', async () => {
    const h = await ready(), owner = h.ports[0]!
    h.controller.acquireOptionPreviews([original], () => { throw new Error('Consumer') })
    expect(owner.renders).toBe(1)
    h.controller.acquireOptionPreviews([original], (_, value) => { if (value.phase === 'ready') h.controller.stop() })
    await vi.waitFor(() => expect(h.controller.read().phase).toBe('idle'))
    expect(owner.leases.size).toBe(0); expect(owner.canvases.every(c => c.width === 0)).toBe(true)
  })
})
describe('actual editor option strip', () => {
  it('keeps ID alternatives, selected state and native keyboard/touch controls without writes until explicit Save', async () => {
    const h = await mounted(true), main = h.controller.read().preview.canvas
    expect(screen.getByRole('button', { name: 'Try option 1 · full outfit' })).toHaveAttribute('aria-pressed', 'true')
    const choice = screen.getByRole('button', { name: 'Try option 2 · full outfit' })
    choice.focus(); expect(choice).toHaveFocus(); fireEvent.click(choice)
    await vi.waitFor(() => expect(screen.getByRole('button', { name: 'Try option 2 · full outfit' })).toBeEnabled())
    expect(h.controller.read().draft?.item_ids.hair).toBe('hair_second'); expect(main?.width).toBe(0); expect(puts(h)).toHaveLength(0)
    expect(screen.getByLabelText('hair')).toHaveValue('hair_second')
    fireEvent.click(screen.getByRole('button', { name: 'Save outfit' })); await screen.findByText('Outfit saved.')
    expect(puts(h)).toHaveLength(1); expect(h.controller.read().persisted?.revision).toBe(2)
  })
  it('disposes pages/categories synchronously, limits visible canvases and never changes the draft merely by browsing', async () => {
    const h = await mounted(), owner = h.ports[0]!, draft = h.controller.read().draft, main = h.controller.read().preview.canvas
    await vi.waitFor(() => expect(h.container.querySelectorAll('.mcr-avatar-option canvas')).toHaveLength(3))
    const previous = [...h.container.querySelectorAll<HTMLCanvasElement>('.mcr-avatar-option canvas')]
    fireEvent.click(screen.getByRole('button', { name: 'Next outfit choices' }))
    expect(previous.every(c => c.width === 0 && !c.isConnected)).toBe(true)
    await vi.waitFor(() => expect(screen.getByRole('button', { name: 'Try option 4 · full outfit' })).toBeEnabled())
    fireEvent.click(screen.getByRole('tab', { name: 'Tones' }))
    await vi.waitFor(() => expect(screen.getByRole('button', { name: 'Try tone fixture · full outfit' })).toBeEnabled())
    expect(screen.getByRole('button', { name: 'Try tone unsupported · full outfit' })).toBeDisabled()
    expect(h.controller.read().draft).toBe(draft); expect(h.controller.read().preview.canvas).toBe(main); expect(owner.leases.size).toBeLessThanOrEqual(4); expect(puts(h)).toHaveLength(0)
    h.unmount(); expect(owner.leases.size).toBe(0); expect(h.container.querySelectorAll('canvas')).toHaveLength(0)
  })
  it.each(['conflict', 'uncertain'] as const)('retains the %s draft across option navigation and requires explicit reconciliation/save', async mode => {
    const h = await mounted(); fireEvent.click(screen.getByRole('button', { name: 'option 2' })); h.setMode(mode)
    fireEvent.click(screen.getByRole('button', { name: 'Save outfit' })); await screen.findByRole('button', { name: 'Check latest saved outfit' })
    fireEvent.click(screen.getByRole('tab', { name: 'Hair' }))
    expect(h.controller.read().draft?.item_ids.hair).toBe('hair_second'); expect(screen.getByRole('button', { name: 'Save outfit' })).toBeDisabled(); expect(puts(h)).toHaveLength(1)
    h.setMode('ok'); h.setServer({ revision: 4, config: original }); fireEvent.click(screen.getByRole('button', { name: 'Check latest saved outfit' }))
    await screen.findByRole('button', { name: 'Keep my draft' }); fireEvent.click(screen.getByRole('button', { name: 'Keep my draft' }))
    expect(puts(h)).toHaveLength(1); fireEvent.click(screen.getByRole('button', { name: 'Save outfit' })); await screen.findByText('Outfit saved.')
    expect(puts(h)[1]!.body).toMatchObject({ expected_revision: 4, config: { item_ids: { hair: 'hair_second' } } })
  })
})
