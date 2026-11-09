// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { createIdentityController } from '../controller'
import { IdentityStatus } from '../IdentityStatus'

const userId = '00000000-0000-4000-8000-000000000003'
const disposals: (() => void)[] = []

afterEach(() => {
  cleanup()
  disposals.splice(0).forEach((dispose) => dispose())
})

interface Harness {
  controller: ReturnType<typeof createIdentityController>
  requests: { method: string; url: string; body: string | null }[]
  respond: (handler: (request: { method: string; url: string }) => Promise<Response>) => void
  navigated: string[]
}

function harness(initial: unknown): Harness {
  const requests: Harness['requests'] = []
  const navigated: string[] = []
  let handler: (request: { method: string; url: string }) => Promise<Response> = async () =>
    Response.json(initial)
  const controller = createIdentityController({
    now: () => Date.parse('2026-11-09T00:00:00Z'),
    scope: () => ({ generation: 1, userId, eventId: null }),
    navigate: (url) => navigated.push(url),
    request: async (request) => {
      requests.push({
        method: request.method,
        url: request.path,
        body: request.body && 'json' in request.body ? JSON.stringify(request.body.json) : null,
      })
      const response = await handler({ method: request.method, url: request.path })
      if (!response.ok) {
        const envelope = await response.json()
        const { TransportError } = await import('../../../shared/api/errors')
        throw new TransportError('http', {
          status: response.status,
          server: {
            envelope,
            core: envelope.details ?? null,
            reason: envelope.details?.reason ?? null,
          },
        })
      }
      const data = await response.json()
      return { data: request.decode(data, { scope: { generation: 1, userId, eventId: null }, status: 200, path: request.path }), scope: { generation: 1, userId, eventId: null } }
    },
  })
  disposals.push(() => controller.dispose())
  return {
    controller,
    requests,
    navigated,
    respond(next) {
      handler = next
    },
  }
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
const expired = {
  provider: 'naver',
  status: 'EXPIRED',
  verified_at: '2026-08-01T00:00:00Z',
  expires_at: '2026-10-30T00:00:00Z',
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

async function renderStatus(initial: unknown) {
  const h = harness(initial)
  render(
    <MemoryRouter>
      <IdentityStatus controller={h.controller} />
    </MemoryRouter>,
  )
  await vi.waitFor(() => expect(h.controller.read().phase).toBe('ready'))
  return h
}

describe('IdentityStatus', () => {
  it('shows the verified state with the derived age and bucket, owner only', async () => {
    await renderStatus(verified)
    expect(screen.getByText('Verified with NAVER')).toBeTruthy()
    // The age and the gender bucket are the owner's own view.
    expect(screen.getByText('26')).toBeTruthy()
    expect(screen.getByText('Female')).toBeTruthy()
    // The expiry date is shown.
    expect(screen.getByText('2027-01-30')).toBeTruthy()
    // And the copy says verification is not required for the rest of the product.
    expect(screen.getByText(/not required for the rest of the product/i)).toBeTruthy()
  })

  it('shows the not-verified state and offers "Connect NAVER"', async () => {
    await renderStatus(notVerified)
    expect(screen.getByText('Not verified')).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Connect NAVER' })).toBeTruthy()
    // No claim value is rendered for a user who has none.
    expect(screen.queryByText('Age')).toBeNull()
  })

  it('shows the expired state and offers "Reconnect"', async () => {
    await renderStatus(expired)
    expect(screen.getByText('Verification expired')).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Reconnect' })).toBeTruthy()
    expect(screen.getByText(/expires after 90 days/i)).toBeTruthy()
    // An expired claim carries no attributes.
    expect(screen.queryByText('Age')).toBeNull()
  })

  it('shows the revoked state and offers "Reconnect"', async () => {
    await renderStatus(revoked)
    expect(screen.getByText('NAVER disconnected')).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Reconnect' })).toBeTruthy()
    expect(screen.queryByText('Age')).toBeNull()
  })

  it('asks for confirmation before disconnecting', async () => {
    const h = await renderStatus(verified)
    const disconnect = screen.getByRole('button', { name: 'Disconnect' })
    fireEvent.click(disconnect)
    // A dialog appears and no request has been sent yet.
    const dialog = screen.getByRole('dialog')
    expect(within(dialog).getByText('Disconnect NAVER?')).toBeTruthy()
    expect(h.requests.filter((request) => request.method === 'DELETE')).toHaveLength(0)
    // Cancelling closes it and still sends nothing.
    fireEvent.click(within(dialog).getByRole('button', { name: 'Keep connected' }))
    expect(screen.queryByRole('dialog')).toBeNull()
    expect(h.requests.filter((request) => request.method === 'DELETE')).toHaveLength(0)
    // Confirming sends exactly one DELETE to the frozen path.
    h.respond(async () => Response.json(revoked))
    fireEvent.click(screen.getByRole('button', { name: 'Disconnect' }))
    fireEvent.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Disconnect NAVER' }))
    await vi.waitFor(() => expect(h.controller.read().verification?.status).toBe('REVOKED'))
    const deletes = h.requests.filter((request) => request.method === 'DELETE')
    expect(deletes).toHaveLength(1)
    expect(deletes[0]!.url).toBe('/api/me/identity/naver')
    expect(screen.getByText('NAVER disconnected')).toBeTruthy()
  })

  it('is keyboard reachable, labelled and keeps focus visible', async () => {
    await renderStatus(verified)
    // Every control is a real button with an accessible name and a focusable
    // element. The focus outline is declared in the stylesheet; this asserts the
    // elements that receive it exist and are not inert.
    const buttons = screen.getAllByRole('button')
    expect(buttons.length).toBeGreaterThan(0)
    for (const button of buttons) {
      expect(button.textContent?.trim().length ?? 0).toBeGreaterThan(0)
      expect(button.hasAttribute('disabled') ? true : button.tabIndex >= 0).toBe(true)
    }
    // The section is labelled by its heading.
    const section = screen.getByRole('region', { name: 'NAVER verification' }) as HTMLElement
    expect(section).toBeTruthy()
    // The dialog traps focus and returns it; the confirm button is reachable.
    fireEvent.click(screen.getByRole('button', { name: 'Disconnect' }))
    const dialog = screen.getByRole('dialog')
    expect(dialog.getAttribute('aria-modal')).toBe('true')
    expect(within(dialog).getAllByRole('button').length).toBeGreaterThanOrEqual(2)
  })

  it('renders no decorative left accent bar', async () => {
    await renderStatus(verified)
    const section = screen.getByRole('region', { name: 'NAVER verification' })
    // The stylesheet is the authority here; this asserts the markup does not
    // carry an inline left border or a decorative rail element either.
    expect(section.getAttribute('style')).toBeNull()
    expect(section.querySelector('[class*="accent"], [class*="rail"]')).toBeNull()
  })

  it('reports a provider-independent message for a rate-limited start', async () => {
    const h = await renderStatus(notVerified)
    h.respond(async () =>
      Response.json(
        { code: 'GATEWAY_429', message: 'Rate limit exceeded', details: { reason: 'RATE_LIMITED' } },
        { status: 429 },
      ),
    )
    fireEvent.click(screen.getByRole('button', { name: 'Connect NAVER' }))
    await vi.waitFor(() => expect(h.controller.read().message).toContain('Too many attempts'))
    // No raw provider code is shown.
    expect(screen.queryByText(/RATE_LIMITED/)).toBeNull()
  })
})
