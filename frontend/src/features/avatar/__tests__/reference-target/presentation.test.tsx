// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { AvatarEditor } from '../../AvatarEditor'
import { harness, original } from '../fixtures/harness'
import { randomCandidates, randomizeDraft } from '../../reference-presentation/actions'

const owned: ReturnType<typeof harness>[] = []
afterEach(() => { cleanup(); owned.splice(0).forEach(h => h.dispose()); vi.restoreAllMocks() })
const palette = [{ id: 'tone_fixture', color: '#DDAA88' }, { id: 'tone_unsupported', color: '#887766' }]
const puts = (h: ReturnType<typeof harness>) => h.requests.filter(r => r.method === 'PUT')
async function setup(tones = palette) {
  const h = harness(); owned.push(h); const view = render(<AvatarEditor controller={h.controller} tonePalette={tones} />)
  await waitFor(() => expect(screen.getByRole('button', { name: 'Try hair fixture · full outfit' })).toBeEnabled())
  return { ...h, ...view }
}
describe('reference Customize production presentation', () => {
  it('exposes named icon tabs, keyboard navigation and one active same-owner choice strip', async () => {
    const h = await setup(), main = h.controller.read().preview.canvas
    expect(screen.getByRole('heading', { name: 'CUSTOMIZE' })).toBeVisible()
    expect(screen.getAllByRole('tab')).toHaveLength(7)
    for (const [previous, next] of [['Hair','Eyes'], ['Eyes','Outfit'], ['Outfit','Body'], ['Body','Accessory'], ['Accessory','Tones'], ['Tones','Presets']] as const) {
      fireEvent.keyDown(screen.getByRole('tab', { name: previous }), { key: 'ArrowRight' })
      expect(screen.getByRole('tab', { name: next })).toHaveFocus()
      expect(screen.getAllByRole('group', { name: /full-outfit choices/ })).toHaveLength(1)
    }
    expect(h.controller.read().preview.canvas).toBe(main); expect(h.controller.read().draft).toEqual(original); expect(puts(h)).toHaveLength(0)
    expect(h.previews).toHaveLength(1); expect(h.previews[0]!.leases.size).toBeLessThanOrEqual(4)
  })
  it('uses only supplied catalog palette colors, disables unsupported tones and refuses invented/CSS values', async () => {
    await setup([...palette, { id: 'tone_fixture', color: '#112233' }, { id: 'unknown', color: '#123456' }, { id: 'tone_fixture', color: 'url(https://invalid.test)' }])
    const group = screen.getByRole('group', { name: 'Skin tone colors' })
    expect(group.querySelectorAll('button')).toHaveLength(2)
    const supported = screen.getByRole('button', { name: 'Skin tone tone fixture' })
    expect(supported).toBeEnabled(); expect(supported).toHaveAttribute('aria-pressed', 'true')
    expect(supported.querySelector('span')).toHaveStyle({ backgroundColor: '#DDAA88' })
    expect(screen.getByRole('button', { name: 'Skin tone tone unsupported' })).toBeDisabled()
    expect(screen.queryByRole('button', { name: 'Skin tone unknown' })).not.toBeInTheDocument()
  })
  it('does not invent a palette when its owning reviewed metadata is absent', async () => {
    await setup([]); expect(screen.queryByRole('group', { name: 'Skin tone colors' })).not.toBeInTheDocument()
  })
  it('Random chooses a supported different preset as a draft, Reset restores baseline, and only Save writes', async () => {
    const h = await setup(); vi.spyOn(Math, 'random').mockReturnValue(0)
    expect(randomCandidates(h.controller).map(p => p.id)).toEqual(['fixture_alternate'])
    fireEvent.click(screen.getByRole('button', { name: 'Random outfit' }))
    expect(h.controller.read().draft?.item_ids.hair).toBe('hair_second'); expect(puts(h)).toHaveLength(0)
    fireEvent.click(screen.getByRole('button', { name: 'Reset draft' }))
    expect(h.controller.read().draft).toEqual(original); expect(puts(h)).toHaveLength(0)
    act(() => { expect(randomizeDraft(h.controller)).toBe(true) })
    fireEvent.click(screen.getByRole('button', { name: 'Save outfit' }))
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Outfit saved.')); expect(puts(h)).toHaveLength(1)
  })
  it.each(['conflict','uncertain'] as const)('Reset preserves %s reconciliation until an explicit latest-state decision', async mode => {
    const h = await setup(); fireEvent.click(screen.getByRole('button', { name: 'Random outfit' })); h.setMode(mode)
    fireEvent.click(screen.getByRole('button', { name: 'Save outfit' })); await screen.findByRole('button', { name: 'Check latest saved outfit' })
    fireEvent.click(screen.getByRole('button', { name: 'Reset draft' }))
    expect(h.controller.read().draft).toEqual(original); expect(h.controller.read().needsReconciliation).toBe(true)
    expect(screen.getByRole('button', { name: 'Save outfit' })).toBeDisabled(); expect(puts(h)).toHaveLength(1)
    h.setMode('ok'); h.setServer({ revision: 4, config: original }); fireEvent.click(screen.getByRole('button', { name: 'Check latest saved outfit' }))
    await screen.findByRole('button', { name: 'Keep my draft' })
    const candidate = h.controller.read().candidate
    expect(screen.getByRole('button', { name: 'Reset draft' })).toBeDisabled()
    fireEvent.click(screen.getByRole('button', { name: 'Reset draft' }))
    expect(h.controller.read().candidate).toBe(candidate); expect(h.controller.read().needsReconciliation).toBe(true); expect(puts(h)).toHaveLength(1)
    fireEvent.click(screen.getByRole('button', { name: 'Keep my draft' }))
    expect(h.controller.read().needsReconciliation).toBe(false); expect(puts(h)).toHaveLength(1)
  })
  it('cleans scoped previews and disables actions on privacy teardown and after unmount', async () => {
    const h = await setup(); act(() => h.privacy.changePolicy(h.privacy.capture(), 'suspended'))
    expect(h.container.querySelectorAll('canvas')).toHaveLength(0)
    for (const label of ['Random outfit','Reset draft','Save outfit']) expect(screen.getByRole('button', { name: label })).toBeDisabled()
    h.unmount(); expect(h.previews.every(port => port.disposed)).toBe(true)
  })
})
