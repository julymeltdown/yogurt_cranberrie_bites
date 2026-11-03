// @vitest-environment jsdom
import { StrictMode } from 'react'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { MemoryRouter } from 'react-router-dom'
import { ProfileForm, MeHub } from '../ui/MePages'
import { SettingsPage } from '../../settings'
import { participant, setup } from './fixtures'
afterEach(cleanup)
const response = () => Response.json(participant())
describe('FE34 accessible route-ready UI', () => {
  it('keeps optional fields independent of world navigation and cancels a public draft', async () => {
    const { controller, auth, dispose } = setup(vi.fn(async () => response()))
    await controller.load()
    const view = render(<MemoryRouter><MeHub controller={controller} auth={auth} /></MemoryRouter>)
    expect(screen.getByRole('link', { name: 'Back to world' }).getAttribute('href')).toBe('/world')
    expect(screen.getByRole('link', { name: 'My gifts' }).getAttribute('href')).toBe('/gifts/mine')
    expect(screen.queryByRole('img')).toBeNull()
    view.rerender(<MemoryRouter><ProfileForm controller={controller} /></MemoryRouter>)
    fireEvent.change(screen.getByLabelText('Bio (optional)'), { target: { value: 'Unsaved' } })
    fireEvent.click(screen.getByRole('button', { name: 'Cancel profile changes' }))
    expect((screen.getByLabelText('Bio (optional)') as HTMLTextAreaElement).value).toBe('')
    expect(screen.getByText(/Ticket section and visibility controls are unavailable/)).toBeTruthy()
    act(dispose)
  })
  it('starts sound off and never invents live application without an adapter', async () => {
    const { controller, auth, dispose } = setup(vi.fn(async () => response())); await controller.load()
    render(<MemoryRouter><SettingsPage controller={controller} auth={auth} /></MemoryRouter>)
    expect((screen.getByLabelText('Enable sound') as HTMLInputElement).checked).toBe(false)
    expect(screen.queryByRole('button', { name: 'Apply saved preferences in this session' })).toBeNull()
    expect(screen.getByText(/Live preference controls are unavailable/)).toBeTruthy(); act(dispose)
  })
  it('aborts a pending runtime adapter on privacy invalidation and drops its late success in StrictMode', async () => {
    const { controller, auth, dispose } = setup(vi.fn(async () => response())); await controller.load()
    let resolve!: () => void, signal!: AbortSignal
    const apply = vi.fn(async (_p, _scope, s) => { signal=s; await new Promise<void>(r=>{resolve=r}) })
    render(<StrictMode><MemoryRouter><SettingsPage controller={controller} auth={auth} runtime={{ apply }} /></MemoryRouter></StrictMode>)
    fireEvent.click(screen.getByRole('button', { name: 'Apply saved preferences in this session' }))
    await waitFor(()=>expect(apply).toHaveBeenCalledTimes(1))
    act(()=>{auth.privacy.changePolicy(auth.privacy.capture(),'block')}); expect(signal.aborted).toBe(true)
    await act(async()=>resolve()); expect(screen.queryByText('Saved preferences applied in this session.')).toBeNull()
    expect((screen.getByLabelText('Enable sound') as HTMLInputElement).checked).toBe(false); act(dispose)
  })
})
