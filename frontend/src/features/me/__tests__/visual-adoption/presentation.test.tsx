// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { MemoryRouter } from 'react-router-dom'
import { MeHub, ProfileForm } from '../../ui/MePages'
import { participant, setup } from '../fixtures'

afterEach(cleanup)
it('keeps the qualified optional-fields owner port and clears public fields after privacy expiry', async () => {
  const fixture = setup(vi.fn(async () => Response.json(participant())))
  await fixture.controller.load()
  const renderOptional = vi.fn(({ controller }) => <p>Owner extension: {controller === fixture.controller ? 'same controller' : 'wrong controller'}</p>)
  render(<MemoryRouter><ProfileForm controller={fixture.controller} optionalFields={{ render: renderOptional }} /></MemoryRouter>)
  expect(screen.getByText('Owner extension: same controller')).toBeTruthy()
  expect(screen.queryByText(/Ticket section and visibility controls are unavailable/)).toBeNull()
  fireEvent.change(screen.getByLabelText('Bio (optional)'), { target: { value: 'Private draft' } })
  act(() => fixture.auth.privacy.changePolicy(fixture.auth.privacy.capture(), 'consent-revoked'))
  expect((screen.getByLabelText('Display name') as HTMLInputElement).value).toBe('')
  expect((screen.getByLabelText('Bio (optional)') as HTMLTextAreaElement).value).toBe('')
  expect((screen.getByRole('button', { name: 'Save profile' }) as HTMLButtonElement).disabled).toBe(true)
  expect((screen.getByRole('button', { name: 'Cancel profile changes' }) as HTMLButtonElement).disabled).toBe(true)
  expect(screen.queryByRole('button', { name: /reload saved version/ })).toBeNull()
  expect(screen.getByRole('link', { name: 'Back to world' }).getAttribute('href')).toBe('/world')
  act(fixture.dispose)
})
it('keeps saved identity separate from an unsaved public draft and uses only existing destinations', async () => {
  const fixture = setup(vi.fn(async () => Response.json(participant())))
  await fixture.controller.load()
  fixture.controller.editProfile({ display_name: 'Unsaved name' })
  render(<MemoryRouter><MeHub controller={fixture.controller} auth={fixture.auth} /></MemoryRouter>)
  expect(screen.getByRole('heading', { name: 'Synthetic listener' })).toBeTruthy()
  expect(screen.queryByText('Unsaved name')).toBeNull()
  expect(screen.getByText(/Avatar preview unavailable/)).toBeTruthy()
  expect(screen.queryByRole('img')).toBeNull()
  expect(screen.getAllByRole('link').map(a => a.getAttribute('href'))).toEqual(['/me/profile', '/me/avatar', '/me/settings', '/meet', '/gifts/mine', '/world'])
  expect(screen.queryByRole('tab')).toBeNull()
  act(fixture.dispose)
})
