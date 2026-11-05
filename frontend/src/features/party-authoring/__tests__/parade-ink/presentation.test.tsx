// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, expect, it } from 'vitest'
import { PartyAuthoring } from '../../ui/PartyAuthoring'
import { harness, draft } from './harness'

const owned: ReturnType<typeof harness>[] = []
afterEach(() => { cleanup(); owned.splice(0).forEach(h => h.dispose()) })
it('retains the editable session draft and clears private fields on logout', async () => {
  const h = harness(); owned.push(h); await h.auth.checkAdmission()
  render(<PartyAuthoring controller={h.controller} />)
  const title = screen.getByRole('textbox', { name: 'Party title' })
  fireEvent.change(title, { target: { value: 'Private local concert plan' } })
  fireEvent.click(screen.getByRole('button', { name: 'Save session draft' }))
  fireEvent.change(title, { target: { value: 'Replacement' } })
  fireEvent.click(screen.getByRole('button', { name: 'Restore draft' }))
  expect((screen.getByRole('textbox', { name: 'Party title' }) as HTMLInputElement).value).toBe('Private local concert plan')
  act(() => h.auth.session.replace(h.auth.session.read().generation, null))
  await waitFor(() => expect(screen.queryByDisplayValue('Private local concert plan')).toBeNull())
  expect(screen.getByRole('heading', { name: 'Party authoring unavailable' })).toBeTruthy()
})

it('locks publishing after a lost response and preserves the original command check', async () => {
  const h = harness(() => Promise.reject(new Error('Synthetic lost response'))); owned.push(h)
  await h.auth.checkAdmission(); await h.controller.submit(draft())
  render(<PartyAuthoring controller={h.controller} />)
  expect(h.controller.read().phase).toBe('uncertain')
  expect(screen.getByRole('button', { name: 'Create party' }).hasAttribute('disabled')).toBe(true)
  expect(screen.getByRole('button', { name: 'Check original command' })).toBeTruthy()
  expect(h.requests.filter(request => request.method === 'POST')).toHaveLength(1)
})
