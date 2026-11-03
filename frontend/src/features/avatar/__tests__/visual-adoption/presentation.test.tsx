// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { afterEach, describe, expect, it } from 'vitest'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { AvatarEditor } from '../../AvatarEditor'
import { harness, original } from '../fixtures/harness'

const owned: ReturnType<typeof harness>[] = []
afterEach(() => { cleanup(); owned.splice(0).forEach(f => f.dispose()) })
async function setup() {
  const f = harness(); owned.push(f); render(<AvatarEditor controller={f.controller} />)
  await screen.findByRole('img', { name: 'Draft outfit preview' }); return f
}
const tab = (name: string) => screen.getByRole('tab', { name })
const writes = (f: ReturnType<typeof harness>) => f.requests.filter(r => r.method === 'PUT')

describe('UIV04 avatar category presentation', () => {
  it('keeps every layer and tone available through keyboard categories with bounded option previews and no draft change or save', async () => {
    const f = await setup(), main = f.controller.read().preview.canvas
    for (const [index, name] of ['Presets', 'Body', 'Eyes', 'Hair', 'Outfit', 'Accessory', 'Tones'].entries()) {
      if (index) fireEvent.keyDown(tab(['Presets', 'Body', 'Eyes', 'Hair', 'Outfit', 'Accessory', 'Tones'][index - 1]!), { key: 'ArrowDown' })
      expect(tab(name)).toHaveAttribute('aria-selected', 'true')
      if (index) expect(screen.getByRole('combobox', { name: name === 'Tones' ? 'Skin tone' : name.toLowerCase() })).toBeVisible()
    }
    expect(screen.getByRole('option', { name: 'tone unsupported' })).toBeDisabled()
    fireEvent.keyDown(tab('Tones'), { key: 'Home' })
    expect(tab('Presets')).toHaveFocus()
    expect(screen.getByRole('button', { name: 'fixture unsupported' })).toBeDisabled()
    expect(f.controller.read().preview.canvas).toBe(main)
    expect(main?.isConnected).toBe(true)
    expect(f.previews[0]!.leases.size).toBeLessThanOrEqual(4)
    expect(document.querySelectorAll('.mcr-avatar-option canvas').length).toBeLessThanOrEqual(3)
    expect(screen.getByRole('button', { name: 'fixture original' })).toHaveAttribute('aria-pressed', 'true')
    expect(f.controller.read().draft).toEqual(original); expect(writes(f)).toHaveLength(0)
    fireEvent.click(tab('Accessory'))
    expect(screen.getByRole('option', { name: 'None' })).toBeEnabled()
    fireEvent.change(screen.getByRole('combobox', { name: 'accessory' }), { target: { value: '' } })
    await waitFor(() => expect(f.controller.read().preview.phase).toBe('ready'))
    expect(f.controller.read().draft?.item_ids.accessory).toBeNull()
    fireEvent.click(screen.getByRole('button', { name: 'Cancel changes' }))
    expect(f.controller.read().draft).toEqual(original); expect(writes(f)).toHaveLength(0)
  })

  it('preserves an uncertain draft across categories and reconciliation until an explicit revision-bound save', async () => {
    const f = await setup(); f.setMode('uncertain')
    fireEvent.click(screen.getByRole('button', { name: 'fixture alternate' }))
    fireEvent.click(screen.getByRole('button', { name: 'Save outfit' }))
    await screen.findByRole('button', { name: 'Check latest saved outfit' })
    expect(writes(f)).toHaveLength(1)
    fireEvent.click(tab('Hair'))
    expect(screen.getByRole('combobox', { name: 'hair' })).toHaveValue('hair_second')
    expect(screen.getByRole('button', { name: 'Save outfit' })).toBeDisabled()
    f.setMode('ok'); f.setServer({ revision: 4, config: original })
    fireEvent.click(screen.getByRole('button', { name: 'Check latest saved outfit' }))
    await screen.findByRole('region', { name: 'Saved outfit reconciliation' })
    fireEvent.click(tab('Tones')); fireEvent.click(tab('Hair'))
    expect(screen.getByRole('combobox', { name: 'hair' })).toHaveValue('hair_second')
    fireEvent.click(screen.getByRole('button', { name: 'Keep my draft' }))
    expect(writes(f)).toHaveLength(1)
    await waitFor(() => expect(screen.getByRole('button', { name: 'Save outfit' })).toBeEnabled())
    fireEvent.click(screen.getByRole('button', { name: 'Save outfit' }))
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Outfit saved.'))
    expect(writes(f)).toHaveLength(2)
    expect(writes(f)[1]!.body).toEqual({ expected_revision: 4, config: f.controller.read().persisted?.config })
    expect(f.controller.read().persisted?.config.item_ids.hair).toBe('hair_second')
  })

  it('exposes pending Save and prevents Cancel while a write is unresolved', async () => {
    const f = await setup(); f.setMode('slow')
    fireEvent.click(screen.getByRole('button', { name: 'fixture alternate' }))
    fireEvent.click(screen.getByRole('button', { name: 'Save outfit' }))
    await waitFor(() => expect(writes(f)).toHaveLength(1))
    expect(screen.getByRole('button', { name: 'Save outfit' })).toHaveAttribute('aria-busy', 'true')
    expect(screen.getByRole('button', { name: 'Cancel changes' })).toBeDisabled()
    act(() => f.pendingSave.resolve(Response.json({ revision: 2, config: f.controller.read().draft })))
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Outfit saved.'))
  })
})
