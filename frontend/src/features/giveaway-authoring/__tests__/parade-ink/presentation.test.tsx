// @vitest-environment jsdom
import { afterEach, expect, it } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { GiveawayAuthoring } from '../../ui/GiveawayAuthoring'
import { harness } from '../harness'
const owners: ReturnType<typeof harness>[] = []
afterEach(() => { cleanup(); owners.splice(0).forEach(h => h.dispose()) })
it('keeps draft fields and client validation without manufacturing upload or publication readiness', async () => {
  const h = harness(); owners.push(h)
  render(<GiveawayAuthoring controller={h.controller} />)
  expect(screen.getByText('Image upload is unavailable. You can save a text-only draft.')).toBeTruthy()
  fireEvent.change(screen.getByLabelText('Item title'), { target: { value: 'Retained draft title' } })
  fireEvent.click(screen.getByRole('button', { name: 'Create giveaway draft' }))
  expect((await screen.findAllByRole('alert')).length).toBeGreaterThan(0)
  expect((screen.getByLabelText('Item title') as HTMLInputElement).value).toBe('Retained draft title')
  expect(h.requests.filter(r => r.method !== 'GET')).toHaveLength(0)
})
