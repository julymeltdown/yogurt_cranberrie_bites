// @vitest-environment jsdom
import { act } from 'react'
import { MemoryRouter } from 'react-router-dom'
import { afterEach, expect, it } from 'vitest'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { SettingsPage } from '../../SettingsPage'
import { fixture } from './fixture'

let f: ReturnType<typeof fixture>
afterEach(() => { cleanup(); f?.dispose() })
async function open() {
  f = fixture({ matches: true, addEventListener() {}, removeEventListener() {} }); await f.controller.load()
  return render(<MemoryRouter><SettingsPage controller={f.controller} auth={f.auth} runtime={f.adapter} theme={f.theme} /></MemoryRouter>)
}
const apply = () => screen.getByRole('button', { name: 'Apply saved preferences in this session' })
it('actual settings saving never applies effects/audio; explicit apply and unmount use the same adapter/controllers', async () => {
  const view = await open()
  fireEvent.click(screen.getByLabelText('Enable sound')); fireEvent.click(screen.getByLabelText('Low power'))
  expect((apply() as HTMLButtonElement).disabled).toBe(true)
  fireEvent.click(screen.getByRole('button', { name: 'Save preferences' }))
  await waitFor(() => expect(f.read().writes).toBe(1))
  expect(f.read()).toMatchObject({ effectCalls: 0, soundCalls: 0, activeSound: 0 })
  await waitFor(() => expect((apply() as HTMLButtonElement).disabled).toBe(false))
  fireEvent.click(apply())
  await waitFor(() => expect(f.adapter.read().phase).toBe('applied'))
  expect(f.read()).toMatchObject({ effectCalls: 1, soundCalls: 1, activeSound: 1, activeEffects: 1,
    adoption: { reducedMotion: true } })
  expect(screen.getByText('Saved preferences applied in this session.')).toBeTruthy()
  expect(screen.getByRole('link', { name: 'Back to world' }).getAttribute('href')).toBe('/world')
  view.unmount()
  expect(f.read()).toMatchObject({ activeSound: 0, activeEffects: 0 })
  expect(f.theme.read().phase).toBe('ready') // The borrower never disposes its shell controller.
})
it('actual settings reports failed theme application, preserves saved values and clears on privacy loss', async () => {
  await open()
  fireEvent.change(screen.getByLabelText('Preferred era'), { target: { value: 'PARADE' } })
  fireEvent.click(screen.getByRole('button', { name: 'Save preferences' }))
  await waitFor(() => expect(f.controller.read().participant?.preferences.theme).toBe('PARADE'))
  await waitFor(() => expect((apply() as HTMLButtonElement).disabled).toBe(false))
  fireEvent.click(apply())
  await waitFor(() => expect(screen.getByText('Could not apply preferences. Saved values are unchanged. Try again.')).toBeTruthy())
  expect(f.controller.read().participant?.preferences.theme).toBe('PARADE')
  expect(f.read()).toMatchObject({ activeEffects: 0, activeSound: 0 })
  act(() => f.revoke())
  expect(f.controller.read().participant).toBeNull(); expect(f.adapter.read().phase).toBe('disposed')
})
