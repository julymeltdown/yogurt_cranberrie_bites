// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { createIdentityController } from '../controller'
import { VerificationCallback } from '../VerificationCallback'
import { VerificationStart } from '../VerificationStart'
import { readResultMarker, resultMessage, RESULT_MARKERS } from '../api/identity'

const userId = '00000000-0000-4000-8000-000000000003'
const disposals: (() => void)[] = []

afterEach(() => {
  cleanup()
  disposals.splice(0).forEach((dispose) => dispose())
})

const notVerified = {
  provider: 'naver',
  status: 'NOT_VERIFIED',
  verified_at: null,
  expires_at: null,
  claims: null,
  display_on_profile: false,
}

const authorizationUrl =
  'https://nid.naver.com/oauth2.0/authorize?response_type=code&client_id=abc&redirect_uri=https%3A%2F%2Fmcr.example%2Fapi%2Fidentity%2Fnaver%2Fcallback&state=state-token'

function harness(read: unknown, start: unknown = { authorization_url: authorizationUrl, expires_at: '2026-11-09T00:10:00Z' }) {
  const requests: { method: string; url: string; body: string | null }[] = []
  const navigated: string[] = []
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
      const payload = request.method === 'POST' ? start : read
      return {
        data: request.decode(payload, {
          scope: { generation: 1, userId, eventId: null },
          status: 200,
          path: request.path,
        }),
        scope: { generation: 1, userId, eventId: null },
      }
    },
  })
  disposals.push(() => controller.dispose())
  return { controller, requests, navigated }
}

describe('VerificationStart', () => {
  it('asks the server for a URL and hands the browser to the provider', async () => {
    const h = harness(notVerified)
    render(
      <MemoryRouter>
        <VerificationStart controller={h.controller} />
      </MemoryRouter>,
    )
    await vi.waitFor(() => expect(h.controller.read().phase).toBe('ready'))
    fireEvent.click(screen.getByRole('button', { name: 'Continue to NAVER' }))
    await vi.waitFor(() => expect(h.navigated).toEqual([authorizationUrl]))
    // The start request is the frozen operation: POST to the frozen path with
    // exactly one field, a return path from the closed allowlist.
    const posts = h.requests.filter((request) => request.method === 'POST')
    expect(posts).toHaveLength(1)
    expect(posts[0]!.url).toBe('/api/me/identity/naver/verifications')
    expect(JSON.parse(posts[0]!.body!)).toEqual({ return_path: '/me/identity' })
    // No claim value is ever sent: the server derives them from the provider.
    expect(posts[0]!.body).not.toMatch(/age|gender|birth/)
  })

  it('refuses an authorization URL that is not a plain https authorization request', async () => {
    for (const url of [
      'http://nid.naver.com/oauth2.0/authorize?response_type=code&state=x',
      'https://user:pass@nid.naver.com/oauth2.0/authorize?response_type=code&state=x',
      'https://nid.naver.com/oauth2.0/authorize?response_type=token&state=x',
      'https://nid.naver.com/oauth2.0/authorize?response_type=code',
      'https://nid.naver.com/oauth2.0/authorize?response_type=code&state=x#fragment',
    ]) {
      const h = harness(notVerified, { authorization_url: url, expires_at: '2026-11-09T00:10:00Z' })
      render(
        <MemoryRouter>
          <VerificationStart controller={h.controller} />
        </MemoryRouter>,
      )
      await vi.waitFor(() => expect(h.controller.read().phase).toBe('ready'))
      fireEvent.click(screen.getByRole('button', { name: 'Continue to NAVER' }))
      await vi.waitFor(() => expect(h.controller.read().busy).toBe(false))
      expect(h.navigated, url).toEqual([])
      expect(h.controller.read().message, url).toContain('unavailable')
      cleanup()
    }
  })

  it('refuses a start whose expiry is outside the state lifetime', async () => {
    const h = harness(notVerified, { authorization_url: authorizationUrl, expires_at: '2026-11-09T01:00:00Z' })
    render(
      <MemoryRouter>
        <VerificationStart controller={h.controller} />
      </MemoryRouter>,
    )
    await vi.waitFor(() => expect(h.controller.read().phase).toBe('ready'))
    fireEvent.click(screen.getByRole('button', { name: 'Continue to NAVER' }))
    await vi.waitFor(() => expect(h.controller.read().busy).toBe(false))
    expect(h.navigated).toEqual([])
  })
})

describe('VerificationCallback', () => {
  it('reports success and re-reads the status without resuming anything', async () => {
    const h = harness({
      provider: 'naver',
      status: 'VERIFIED',
      verified_at: '2026-11-09T00:00:00Z',
      expires_at: '2027-02-07T00:00:00Z',
      claims: { age: 26, gender_bucket: 'female' },
      display_on_profile: false,
    })
    render(
      <MemoryRouter>
        <VerificationCallback controller={h.controller} marker="success" />
      </MemoryRouter>,
    )
    await vi.waitFor(() => expect(h.controller.read().phase).toBe('ready'))
    expect(screen.getByRole('status').textContent).toContain('verified')
    // Exactly one read and no write of any kind: the landing page does not
    // re-trigger the work the user was doing before verification.
    expect(h.requests.filter((request) => request.method !== 'GET')).toHaveLength(0)
    expect(h.requests.filter((request) => request.method === 'GET')).toHaveLength(1)
    expect(screen.getByText(/Nothing you were doing before verification was submitted or repeated/)).toBeTruthy()
  })

  it('reports a cancelled consent with a cancellable message', async () => {
    const h = harness(notVerified)
    render(
      <MemoryRouter>
        <VerificationCallback controller={h.controller} marker="cancelled" />
      </MemoryRouter>,
    )
    await vi.waitFor(() => expect(h.controller.read().phase).toBe('ready'))
    expect(screen.getByRole('alert').textContent).toContain('cancelled')
    expect(screen.getByRole('button', { name: 'Try again' })).toBeTruthy()
    expect(screen.getByText('Back to where you were')).toBeTruthy()
    // Still no write.
    expect(h.requests.filter((request) => request.method !== 'GET')).toHaveLength(0)
  })

  it('shows a provider-independent message for every failure marker', async () => {
    for (const marker of RESULT_MARKERS.filter((value) => value !== 'success')) {
      const h = harness(notVerified)
      render(
        <MemoryRouter>
          <VerificationCallback controller={h.controller} marker={marker} />
        </MemoryRouter>,
      )
      await vi.waitFor(() => expect(h.controller.read().phase).toBe('ready'))
      const message = resultMessage(marker)
      expect(message.length).toBeGreaterThan(0)
      // The message never echoes a raw provider code.
      expect(message).not.toMatch(/invalid_request|access_denied|server_error|profile_incomplete/)
      cleanup()
    }
  })

  it('shows a neutral message when the marker is absent or unknown', async () => {
    const h = harness(notVerified)
    render(
      <MemoryRouter>
        <VerificationCallback controller={h.controller} marker={null} />
      </MemoryRouter>,
    )
    await vi.waitFor(() => expect(h.controller.read().phase).toBe('ready'))
    expect(screen.getByRole('alert').textContent).toContain('did not finish')
  })
})

describe('readResultMarker', () => {
  it('reads only the closed marker set', () => {
    for (const marker of RESULT_MARKERS) {
      expect(readResultMarker(`?identity=${marker}`)).toBe(marker)
    }
    for (const search of ['', '?identity=', '?identity=nope', '?identity=SUCCESS', '?other=success']) {
      expect(readResultMarker(search), search).toBeNull()
    }
  })
})
