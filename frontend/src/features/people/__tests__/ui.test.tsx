// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { StrictMode } from 'react'
import { MemoryRouter } from 'react-router-dom'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, expect, it } from 'vitest'
import { People } from '../People'
import { PublicProfile } from '../../profile/PublicProfile'
import { IntroEditor } from '../../intro/IntroEditor'
import { createIntroController } from '../../intro/controller'
import { AgeStatus } from '../../age-status/AgeStatus'
import { createAgeStatusController } from '../../age-status/controller'
import { eventId, harness, introId, publicIntro, userId } from './fixtures/harness'
const owned: (() => void)[] = []
afterEach(() => { cleanup(); owned.splice(0).forEach(fn => fn()) })
function setup() { const h = harness(); owned.push(h.dispose); return h }
const wrapper = (children: React.ReactNode) => <StrictMode><MemoryRouter>{children}</MemoryRouter></StrictMode>
it('supports discovery without X, public profile navigation and a clear artwork-unavailable state', async () => {
  const h = setup(), view = render(wrapper(<People controller={h.controller} />)); const author = publicIntro().author
  expect(await screen.findByRole('link', { name: author.display_name })).toHaveAttribute('href', `/people/${author.user_id}`)
  expect(screen.getByRole('img', { name: `${author.display_name}'s avatar: avatar unavailable` })).toBeInTheDocument()
  expect(view.container.querySelector('canvas,img')).toBeNull(); expect(view.container.textContent).not.toContain('Minimum age')
  view.rerender(wrapper(<PublicProfile controller={h.controller} userId={author.user_id} />)); await screen.findByRole('heading', { name: author.display_name })
  expect(screen.getByRole('link', { name: 'Report profile' })).toHaveAttribute('href', `/report?targetType=USER&targetId=${author.user_id}`); expect(screen.getByRole('button', { name: 'Request sharing' })).toBeDisabled()
})
it('keeps owner-only conditions editable in read-only and clears mounted text on policy invalidation', async () => {
  const h = setup(), controller = createIntroController(h.options); owned.push(controller.dispose)
  h.respond(async () => Response.json({ intro_id: introId, event_id: eventId, owner_id: userId, text: 'Owner private draft', age_bounds: { minimum_age: 20, maximum_age_exclusive: 30 }, expires_at: null, created_at: '2026-11-07T08:00:00Z', state: 'DRAFT', revision: 1 }))
  render(wrapper(<IntroEditor controller={controller} />)); await waitFor(() => expect(screen.getByLabelText('Introduction')).toHaveValue('Owner private draft'))
  act(() => h.access({ readOnly: true })); fireEvent.change(screen.getByLabelText('Introduction'), { target: { value: 'Read-only local draft' } }); expect(screen.getByRole('button', { name: 'Save introduction' })).toBeDisabled(); expect(screen.getByRole('button', { name: 'Close introduction' })).toBeEnabled()
  act(() => h.privacy.changePolicy(h.privacy.capture(), 'suspended')); expect(screen.getByLabelText('Introduction')).toHaveValue(''); expect(screen.getByRole('button', { name: 'Save introduction' })).toBeDisabled()
})
it('shows provider unavailable without a verified badge or a self-declaration input', async () => {
  const h = setup(), controller = createAgeStatusController(h.options); owned.push(controller.dispose)
  h.respond(async () => Response.json({ user_id: userId, event_id: eventId, status: 'PROVIDER_UNAVAILABLE', verified_at: null, expires_at: null, verification_available: false }))
  const view = render(wrapper(<AgeStatus controller={controller} />)); await screen.findByRole('heading', { name: 'Verification provider unavailable' }); expect(screen.getByRole('button', { name: 'Start verification' })).toBeDisabled(); expect(view.container.querySelector('input')).toBeNull()
})
