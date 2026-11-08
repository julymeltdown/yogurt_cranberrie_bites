// @vitest-environment jsdom
// WP-09 T-906: the identity settings screen.
//
// The acceptance statements are:
//
// * the screen is reachable from My Profile;
// * disconnect is confirmed and reversible by reconnecting.
//
// The screen composes WP-04b's `IdentityStatus`, which owns the read, the start,
// the disconnect and the confirmation dialog. This test proves the composition:
// the screen is reachable from the profile page, it states the purpose and the
// non-requirement, and disconnect goes through a confirmation and can be undone
// by reconnecting.
import '@testing-library/jest-dom/vitest'
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createIdentityController } from '../controller'
import { IdentitySettings } from '../IdentitySettings'
import { ProfilePage } from '../../profile/ProfilePage'
import { TransportError } from '../../../shared/api/errors'

const userId = '00000000-0000-4000-8000-000000000003'
const disposals: (() => void)[] = []
const routers: ReturnType<typeof createMemoryRouter>[] = []

afterEach(() => {
  cleanup()
  disposals.splice(0).forEach(dispose => dispose())
  routers.splice(0).forEach(router => router.dispose())
})

interface Harness {
  controller: ReturnType<typeof createIdentityController>
  requests: { method: string; url: string }[]
  respond: (handler: (request: { method: string; url: string }) => Promise<Response>) => void
  navigated: string[]
}

function harness(initial: unknown): Harness {
  const requests: Harness['requests'] = []
  const navigated: string[] = []
  let handler: (request: { method: string; url: string }) => Promise<Response> = async () => Response.json(initial)
  const controller = createIdentityController({
    now: () => Date.parse('2026-11-09T00:00:00Z'),
    scope: () => ({ generation: 1, userId, eventId: null }),
    navigate: url => navigated.push(url),
    request: async request => {
      requests.push({ method: request.method, url: request.path })
      const response = await handler({ method: request.method, url: request.path })
      if (!response.ok) {
        const envelope = await response.json()
        throw new TransportError('http', {
          status: response.status,
          server: { envelope, core: envelope.details ?? null, reason: envelope.details?.reason ?? null },
        })
      }
      const data = await response.json()
      return { data: request.decode(data, { scope: { generation: 1, userId, eventId: null }, status: 200, path: request.path }) }
    },
  })
  disposals.push(() => controller.dispose())
  return { controller, requests, navigated, respond(next) { handler = next } }
}

const verified = {
  provider: 'naver',
  status: 'VERIFIED',
  verified_at: '2026-11-01T00:00:00Z',
  expires_at: '2027-01-30T00:00:00Z',
  claims: { age: 26, gender_bucket: 'female' },
  display_on_profile: false,
}
const notVerified = {
  provider: 'naver',
  status: 'NOT_VERIFIED',
  verified_at: null,
  expires_at: null,
  claims: null,
  display_on_profile: false,
}
const revoked = {
  provider: 'naver',
  status: 'REVOKED',
  verified_at: '2026-08-01T00:00:00Z',
  expires_at: '2026-08-02T00:00:00Z',
  claims: null,
  display_on_profile: false,
}

function renderSettings(initial: unknown) {
  const h = harness(initial)
  render(<IdentitySettings controller={h.controller} />)
  return h
}

describe('the identity settings screen', () => {
  it('states the purpose and the non-requirement', () => {
    renderSettings(notVerified)
    const text = document.body.textContent ?? ''
    // Purpose.
    expect(text).toMatch(/confirms your age and gender/i)
    expect(text).toMatch(/age or gender restriction/i)
    // Non-requirement.
    expect(text).toMatch(/not required for the rest of the product/i)
    expect(text).toMatch(/work without it/i)
  })

  it('renders the identity status inside the screen', async () => {
    const h = renderSettings(verified)
    await vi.waitFor(() => expect(h.controller.read().phase).toBe('ready'))
    expect(screen.getByRole('heading', { level: 1, name: 'Identity verification' })).toBeVisible()
    // The owner-only claim view is present.
    expect(screen.getByText(/Verified/)).toBeVisible()
  })

  it('shows the owner-only age on this screen and nowhere else', async () => {
    const h = renderSettings(verified)
    await vi.waitFor(() => expect(h.controller.read().phase).toBe('ready'))
    // This is the one screen that may show the derived age, and it is owner-only.
    expect(screen.getByText('Age')).toBeVisible()
    expect(screen.getByText('26')).toBeVisible()
  })

  it('confirms disconnect before disconnecting', async () => {
    const h = renderSettings(verified)
    await vi.waitFor(() => expect(h.controller.read().phase).toBe('ready'))
    fireEvent.click(screen.getByRole('button', { name: 'Disconnect' }))
    // A confirmation dialog appears; nothing has been sent yet.
    const dialog = await screen.findByRole('dialog')
    expect(within(dialog).getByText(/Disconnect NAVER\?/)).toBeVisible()
    expect(h.requests.filter(request => request.method === 'DELETE')).toHaveLength(0)
    // Cancelling sends nothing.
    fireEvent.click(within(dialog).getByRole('button', { name: 'Keep connected' }))
    expect(h.requests.filter(request => request.method === 'DELETE')).toHaveLength(0)
  })

  it('disconnects only after the confirmation', async () => {
    const h = renderSettings(verified)
    await vi.waitFor(() => expect(h.controller.read().phase).toBe('ready'))
    h.respond(async request => request.method === 'DELETE' ? Response.json(revoked) : Response.json(verified))
    fireEvent.click(screen.getByRole('button', { name: 'Disconnect' }))
    const dialog = await screen.findByRole('dialog')
    fireEvent.click(within(dialog).getByRole('button', { name: 'Disconnect NAVER' }))
    await vi.waitFor(() => expect(h.requests.filter(request => request.method === 'DELETE')).toHaveLength(1))
    expect(h.requests.find(request => request.method === 'DELETE')?.url).toBe('/api/me/identity/naver')
  })

  it('is reversible by reconnecting after a disconnect', async () => {
    const h = renderSettings(revoked)
    await vi.waitFor(() => expect(h.controller.read().phase).toBe('ready'))
    // A revoked claim offers a reconnect action. The handler is installed before
    // the click so the start request sees it.
    const reconnect = await screen.findByRole('button', { name: /Reconnect/ })
    // The start answer must expire within the ten-minute window the client
    // issued; the harness clock is fixed at 2026-11-09T00:00:00Z.
    h.respond(async () => Response.json({
      // The provider URL must be a plain https authorization request: the client
      // follows it, so it checks the protocol, the absence of credentials and a
      // `response_type=code` with a state.
      authorization_url: 'https://nid.naver.com/oauth2.0/authorize?response_type=code&state=fixture',
      expires_at: '2026-11-09T00:05:00Z',
    }))
    expect((reconnect as HTMLButtonElement).disabled).toBe(false)
    fireEvent.click(reconnect)
    await vi.waitFor(() => expect(h.requests.some(request => request.method === 'POST')).toBe(true))
    // Reconnecting navigates to the provider: the flow is reversible.
    await vi.waitFor(() => expect(h.navigated).toHaveLength(1))
    expect(h.navigated[0]).toContain('nid.naver.com')
  })

  it('never renders a numeric age for an unverified owner', async () => {
    const h = renderSettings(notVerified)
    await vi.waitFor(() => expect(h.controller.read().phase).toBe('ready'))
    const text = document.body.textContent ?? ''
    expect(text).not.toMatch(/\b\d{1,3}\s*(?:years?|yo)\b/i)
  })
})

describe('the screen is reachable from My Profile', () => {
  it('links to the identity settings from the owner profile page', () => {
    const router = createMemoryRouter([{ path: '*', element: <ProfilePage isOwner view={{
      profileId: userId, nickname: 'Owner', bio: null, favorites: [], roomAvailable: true,
    }} /> }], { initialEntries: ['/me'] })
    routers.push(router)
    render(<RouterProvider router={router} />)
    const link = screen.getByRole('link', { name: 'Identity verification' })
    expect(link.getAttribute('href')).toBe('/me/identity')
  })

  it('is not offered to a visitor', () => {
    const router = createMemoryRouter([{ path: '*', element: <ProfilePage isOwner={false} view={{
      profileId: userId, nickname: 'Someone', bio: null, favorites: [], roomAvailable: true,
    }} /> }], { initialEntries: [`/discover/people/${userId}`] })
    routers.push(router)
    render(<RouterProvider router={router} />)
    expect(screen.queryByRole('link', { name: 'Identity verification' })).toBeNull()
  })
})
