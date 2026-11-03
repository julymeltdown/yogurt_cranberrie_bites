// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { StrictMode } from 'react'
import { afterEach, describe, expect, it } from 'vitest'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { AvatarEditor } from '../AvatarEditor'
import { harness, original } from './fixtures/harness'
const owned: ReturnType<typeof harness>[] = []
afterEach(() => { cleanup(); owned.splice(0).forEach(f => f.dispose()) })
function setup(strict = false) { const f = harness(); owned.push(f); const view = <AvatarEditor controller={f.controller} />; return { ...f, ...render(strict ? <StrictMode>{view}</StrictMode> : view) } }
describe('FE13 accessible editor', () => {
  it('mounts through strict effects, previews selections and cancels without persisting them', async () => {
    const f = setup(true)
    await screen.findByRole('img', { name: 'Draft outfit preview' })
    expect(screen.getByRole('heading', { name: 'DRESS UP' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Save outfit' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'fixture unsupported' })).toBeDisabled()
    fireEvent.change(screen.getByLabelText('hair'), { target: { value: 'hair_second' } })
    expect(screen.getByRole('button', { name: 'Save outfit' })).toBeEnabled()
    fireEvent.click(screen.getByRole('button', { name: 'Cancel changes' }))
    expect(screen.getByLabelText('hair')).toHaveValue('hair_fixture')
    expect(f.requests.filter(r => r.method === 'PUT')).toHaveLength(0)
    fireEvent.click(screen.getByRole('button', { name: 'fixture alternate' }))
    fireEvent.click(screen.getByRole('button', { name: 'Save outfit' }))
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Outfit saved.'))
    expect(f.controller.read().persisted?.revision).toBe(2)
  })
  it('keeps draft visible through conflict and allows explicit adoption of the saved outfit', async () => {
    const f = setup(); await screen.findByRole('img', { name: 'Draft outfit preview' })
    f.setMode('conflict'); fireEvent.click(screen.getByRole('button', { name: 'fixture alternate' })); fireEvent.click(screen.getByRole('button', { name: 'Save outfit' }))
    await screen.findByRole('button', { name: 'Check latest saved outfit' })
    expect(screen.getByRole('button', { name: 'Save outfit' })).toBeDisabled(); expect(screen.getByLabelText('hair')).toHaveValue('hair_second')
    f.setMode('ok'); f.setServer({ revision: 4, config: original })
    fireEvent.click(screen.getByRole('button', { name: 'Check latest saved outfit' }))
    await screen.findByRole('heading', { name: 'Saved outfit · revision 4' })
    fireEvent.click(screen.getByRole('button', { name: 'Use saved outfit' }))
    expect(screen.getByLabelText('hair')).toHaveValue('hair_fixture')
    expect(f.requests.filter(r => r.method === 'PUT')).toHaveLength(1)
  })
  it('erases mounted pixels synchronously on policy cleanup and on unmount', async () => {
    const f = setup(); await screen.findByRole('img', { name: 'Draft outfit preview' }); const canvas = f.container.querySelector('canvas')!
    act(() => { f.privacy.changePolicy(f.privacy.capture(), 'suspended') })
    expect(canvas.width).toBe(0); expect(f.container.querySelector('canvas')).toBeNull(); expect(screen.getByRole('button', { name: 'Save outfit' })).toBeDisabled()
    f.unmount(); expect(f.controller.read().draft).toBeNull(); expect(f.previews.every(p => p.disposed)).toBe(true)
  })
})
