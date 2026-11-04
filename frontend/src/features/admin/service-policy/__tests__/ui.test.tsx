// @vitest-environment jsdom
import { afterEach, expect, it } from 'vitest'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { ServicePolicyOperations } from '../ServicePolicyOperations'
import { fixture, validDraft } from './fixture'
const owners: ReturnType<typeof fixture>[] = []
afterEach(() => { cleanup(); owners.splice(0).forEach(owner => owner.dispose()) })
async function mount() {
  const owner = fixture(); owners.push(owner); owner.installUnitIdentity(); await owner.initialize()
  render(<ServicePolicyOperations controller={owner.controller} />)
  await waitFor(() => expect(owner.controller.read().decision).not.toBeNull()); return owner
}
it('labels inputs, focuses risky confirmation and returns focus on cancel; source metadata stays unavailable', async () => {
  const owner = await mount()
  fireEvent.change(screen.getByLabelText('Minimum client version'), { target: { value: validDraft.minimumClientVersion } })
  fireEvent.change(screen.getByLabelText('Publication expiry (UTC)'), { target: { value: validDraft.expiresAt } })
  fireEvent.change(screen.getByLabelText('Reason'), { target: { value: validDraft.reason } })
  const review = screen.getByRole('button', { name: 'Review policy change' }); review.focus(); fireEvent.click(review)
  expect(document.activeElement).toBe(screen.getByRole('heading', { name: 'Confirm service policy change' }))
  expect(screen.getByText(/source and review metadata are unavailable/)).toBeTruthy()
  fireEvent.click(screen.getByRole('button', { name: 'Cancel confirmation' }))
  expect(document.activeElement).toBe(review); expect(owner.requests.filter(r => r.method === 'PUT')).toHaveLength(0)
})
it('retains pending command across child unmount and scrubs controls on authority withdrawal', async () => {
  const owner = await mount()
  await act(async () => { owner.controller.edit(validDraft); owner.controller.review(); owner.mode('pending'); await owner.controller.confirm() })
  cleanup(); render(<ServicePolicyOperations controller={owner.controller} />)
  await waitFor(() => expect(screen.getByRole('button', { name: 'Check policy command status' })).toBeTruthy())
  expect(screen.getByLabelText('Reason').matches(':disabled')).toBe(true)
  act(() => owner.withdraw())
  expect(screen.queryByLabelText('Reason')).toBeNull(); expect(screen.queryByRole('button', { name: 'Confirm policy change' })).toBeNull()
  expect(screen.getByRole('heading', { name: 'Service policy unavailable' })).toBeTruthy()
})
