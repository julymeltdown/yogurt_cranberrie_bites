// @vitest-environment jsdom
import { StrictMode, useEffect } from 'react'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { afterEach, expect, it, vi } from 'vitest'
import { PolicyNotice } from '../PolicyNotice'
import { usePolicyAttempt } from '../usePolicyAttempt'
import type { PolicyAttempt, PreWorldPolicyPort } from '../model'

afterEach(cleanup)
const scope = { userId: '11111111-1111-4111-8111-111111111111', eventId: '22222222-2222-4222-8222-222222222222', generation: 1 }
const privacyScope = { ...scope, policyGeneration: 0, gate: 'open' as const }
const release = { version: 1 as const, applicationVersion: '0.1.0', buildId: 'a'.repeat(64), sourceRevision: 'b'.repeat(40) }
const source = Object.freeze({})
const decision = { kind: 'maintenance' as const, eventRevision: 1, policyRevision: 2, minimumClientVersion: '0.1.0',
  serverTimeMs: 1_000_000, validUntilMs: 1_005_000 }

it('uses ordinary keyboard controls for a trusted version notice and retry', () => {
  const retry = vi.fn(), reload = vi.fn()
  render(<MemoryRouter><PolicyNotice view={{ kind: 'version', minimumClientVersion: '0.2.0' }} retry={retry} reload={reload} /></MemoryRouter>)
  expect(screen.getByRole('heading', { name: 'An app update is required' })).toBeTruthy()
  expect(screen.getByText(/requires app version 0.2.0 or later/)).toBeTruthy()
  const reloadButton = screen.getByRole('button', { name: 'Reload app' })
  reloadButton.focus(); expect(document.activeElement).toBe(reloadButton)
  fireEvent.click(reloadButton); fireEvent.click(screen.getByRole('button', { name: 'Check again' }))
  expect(reload).toHaveBeenCalledTimes(1); expect(retry).toHaveBeenCalledTimes(1)
  expect(screen.getByRole('link', { name: 'Back to home' }).getAttribute('href')).toBe('/')
})

it('does not dispatch a discarded StrictMode attempt or restamp a mismatched privacy capture', async () => {
  const claims: PolicyAttempt[] = []
  const port: PreWorldPolicyPort = { release: vi.fn(() => release), read: vi.fn(async captured => { claims.push(captured); return decision }) }
  function Harness({ mismatch = false }: { mismatch?: boolean }) {
    const policy = usePolicyAttempt({ source, port, scope, privacyScope: mismatch ? { ...privacyScope, eventId: '33333333-3333-4333-8333-333333333333' } : privacyScope,
      attempt: mismatch ? 1 : 0, isCurrent: () => true })
    useEffect(() => {
      const lifetime = new AbortController(), owner = policy.start(lifetime.signal)
      void owner.prepare(lifetime.signal)
      return () => lifetime.abort()
      // Match useScopedLoad: only captured attempt identity restarts the read.
      // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [mismatch])
    return <PolicyNotice view={policy.view} retry={() => {}} reload={() => {}} />
  }
  const view = render(<StrictMode><MemoryRouter><Harness /></MemoryRouter></StrictMode>)
  await waitFor(() => expect(screen.getByRole('heading', { name: 'World entry is paused' })).toBeTruthy())
  expect(port.read).toHaveBeenCalledTimes(1)
  expect(claims[0]?.scope).toEqual(privacyScope)
  view.rerender(<StrictMode><MemoryRouter><Harness mismatch /></MemoryRouter></StrictMode>)
  await waitFor(() => expect(screen.getByRole('heading', { name: 'Event availability could not be confirmed' })).toBeTruthy())
  expect(port.read).toHaveBeenCalledTimes(1)
  view.unmount()
})

it('clears a displayed trusted notice on expiry without automatic retry', async () => {
  vi.useFakeTimers()
  try {
    const port: PreWorldPolicyPort = { release: () => release, read: vi.fn(async () => ({ ...decision, validUntilMs: decision.serverTimeMs + 100 })) }
    function Harness() {
      const policy = usePolicyAttempt({ source, port, scope, privacyScope, attempt: 0, isCurrent: () => true })
      useEffect(() => {
        const lifetime = new AbortController(), owner = policy.start(lifetime.signal)
        void owner.prepare(lifetime.signal)
        return () => lifetime.abort()
        // eslint-disable-next-line react-hooks/exhaustive-deps
      }, [])
      return <PolicyNotice view={policy.view} retry={() => {}} reload={() => {}} />
    }
    render(<MemoryRouter><Harness /></MemoryRouter>)
    await act(async () => { await vi.advanceTimersByTimeAsync(0) })
    expect(screen.getByRole('heading', { name: 'World entry is paused' })).toBeTruthy()
    await act(async () => { await vi.advanceTimersByTimeAsync(100) })
    expect(screen.getByRole('heading', { name: 'Event availability could not be confirmed' })).toBeTruthy()
    expect(port.read).toHaveBeenCalledTimes(1)
    cleanup()
    expect(vi.getTimerCount()).toBe(0)
  } finally { vi.useRealTimers() }
})
