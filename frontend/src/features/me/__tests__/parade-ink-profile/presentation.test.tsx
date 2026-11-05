// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { afterEach, expect, it, vi } from 'vitest'
import { MeHub, ProfileForm } from '../../ui/MePages'
import { SettingsPage } from '../../../settings/SettingsPage'
import { PublicProfile } from '../../../profile/PublicProfile'
import { participant, setup } from '../fixtures'
import { harness, publicIntro } from '../../../people/__tests__/fixtures/harness'
const disposers: (() => void)[] = []
afterEach(() => { cleanup(); disposers.splice(0).forEach(dispose => dispose()) })
async function own() {
  const fetcher = vi.fn(async () => Response.json(participant())), h = setup(fetcher)
  disposers.push(h.dispose); await h.controller.load(); return { ...h, fetcher }
}
it('keeps relocated edit/customize and private account navigation functional without extra profile reads', async () => {
  const h = await own(), avatar = vi.fn(() => <output>Approved renderer slot</output>)
  const view = render(<MemoryRouter><MeHub controller={h.controller} auth={h.auth} renderAvatar={avatar} /></MemoryRouter>)
  expect(screen.getByRole('link', { name: 'Edit profile' })).toHaveAttribute('href', '/me/profile')
  expect(screen.getByRole('link', { name: 'Dress up' })).toHaveAttribute('href', '/me/avatar')
  expect(screen.getByRole('link', { name: 'My gifts' })).toHaveAttribute('href', '/gifts/mine')
  fireEvent.click(screen.getByText('Profile options'))
  expect(screen.getByRole('link', { name: 'Private social links' })).toHaveAttribute('href', '/me/social')
  expect(screen.getByRole('link', { name: 'Sharing requests' })).toHaveAttribute('href', '/me/requests')
  expect(avatar).toHaveBeenCalledWith(h.controller.read().participant!.profile.avatar, `${participant().profile.display_name}'s avatar`)
  expect(h.fetcher).toHaveBeenCalledTimes(1)
  expect(view.container.textContent).not.toMatch(/Guestbook|Lv\.|Say hi|XP/)
})
it('retains real draft cancellation and the unavailable U03 boundary inside the new form frame', async () => {
  const h = await own()
  render(<MemoryRouter><ProfileForm controller={h.controller} /></MemoryRouter>)
  expect(screen.getByRole('button', { name: 'Save profile' })).toBeDisabled()
  fireEvent.change(screen.getByLabelText('Bio (optional)'), { target: { value: 'A new unsaved bio' } })
  expect(screen.getByRole('button', { name: 'Save profile' })).toBeEnabled()
  fireEvent.click(screen.getByRole('button', { name: 'Cancel profile changes' }))
  expect(screen.getByLabelText('Bio (optional)')).toHaveValue('')
  expect(screen.getByRole('button', { name: 'Save profile' })).toBeDisabled()
  expect(screen.getByText(/Ticket section and visibility controls are unavailable/)).toBeVisible()
  expect(h.fetcher).toHaveBeenCalledTimes(1)
})
it('keeps preference era independent and sound off; cancel restores the saved draft without an apply adapter', async () => {
  const h = await own()
  render(<MemoryRouter><SettingsPage controller={h.controller} auth={h.auth} /></MemoryRouter>)
  expect(screen.getByLabelText('Enable sound')).not.toBeChecked()
  const era = screen.getByRole('combobox', { name: /^Preferred era/ })
  fireEvent.change(era, { target: { value: 'DANGER' } })
  fireEvent.click(screen.getByLabelText('Low power'))
  expect(screen.getByRole('button', { name: 'Save preferences' })).toBeEnabled()
  fireEvent.click(screen.getByRole('button', { name: 'Cancel preference changes' }))
  expect(era).toHaveValue('REVENGE'); expect(screen.getByLabelText('Low power')).not.toBeChecked()
  expect(screen.getByLabelText('Enable sound')).not.toBeChecked()
  expect(screen.queryByRole('button', { name: 'Apply saved preferences in this session' })).toBeNull()
  expect(screen.getByRole('link', { name: 'Back to world' })).toHaveAttribute('href', '/world')
  expect(h.fetcher).toHaveBeenCalledTimes(1)
})
it('keeps public actions capability-dependent and removes the framed identity when privacy changes', async () => {
  const h = harness(); disposers.push(h.dispose); const author = publicIntro().author
  render(<MemoryRouter><PublicProfile controller={h.controller} userId={author.user_id} renderAvatar={() => <output>Public approved portrait</output>} /></MemoryRouter>)
  await screen.findByRole('heading', { name: author.display_name })
  expect(screen.getByRole('button', { name: 'Request sharing' })).toBeDisabled()
  expect(screen.getByRole('button', { name: 'Block user' })).toBeDisabled()
  expect(screen.getByRole('button', { name: 'Parties' })).toBeDisabled()
  expect(screen.getByRole('button', { name: 'Gifts' })).toBeDisabled()
  expect(screen.getByRole('link', { name: 'Report profile' })).toHaveAttribute('href', `/report?targetType=USER&targetId=${author.user_id}`)
  act(() => h.privacy.changePolicy(h.privacy.capture(), 'block'))
  expect(screen.queryByRole('heading', { name: author.display_name })).toBeNull()
  expect(screen.queryByText('Public approved portrait')).toBeNull()
})
