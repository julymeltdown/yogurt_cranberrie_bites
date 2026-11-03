// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { ConnectionStatus, ConnectionStatusNotice } from '../index'
import { RouteState } from '../../../app/errors/RouteState'
import { harness } from '../../../app/errors/__tests__/harness'
afterEach(cleanup)
for (const kind of ['offline', 'reconnecting', 'resync', 'suspended', 'unavailable', 'maintenance', 'version'] as const) it(`renders safe ${kind} copy with fallback navigation`, () => {
  render(<MemoryRouter><ConnectionStatusNotice view={{ kind, stale: true, pending: ['resync', 'reconnecting'].includes(kind), canRetry: kind === 'unavailable' }} retry={() => {}} reload={() => {}} /></MemoryRouter>)
  expect(screen.getByRole('status').textContent).toContain('Previously shown world')
  expect(screen.getByRole('link', { name: 'Back to world' }).getAttribute('href')).toBe('/world')
  expect(screen.getByRole('link', { name: 'Back to home' }).getAttribute('href')).toBe('/')
  if (kind === 'version') expect(screen.queryByRole('button', { name: 'Reconnect' })).toBeNull()
})
it('does not interrupt ready world with a redundant status panel', () => {
  const { container } = render(<MemoryRouter><ConnectionStatusNotice view={{ kind: 'ready', stale: false, pending: false, canRetry: false }} retry={() => {}} reload={() => {}} /></MemoryRouter>)
  expect(container.textContent).toBe('')
})
it('uses real keyboard-focusable buttons for explicit reconnect/reload and real links for fallback', () => {
  const retry = vi.fn(), reload = vi.fn()
  render(<MemoryRouter><ConnectionStatusNotice view={{ kind: 'unavailable', stale: true, pending: false, canRetry: true }} retry={retry} reload={reload} /></MemoryRouter>)
  const primary = screen.getByRole('button', { name: 'Reconnect' }); primary.focus(); expect(document.activeElement).toBe(primary)
  fireEvent.click(primary); fireEvent.click(screen.getByRole('button', { name: 'Reload page' })); expect(retry).toHaveBeenCalledOnce(); expect(reload).toHaveBeenCalledOnce()
})
for (const kind of ['not-found', 'unavailable', 'maintenance', 'version'] as const) it(`supplies a fixed ${kind} route with safe fallback`, () => {
  render(<MemoryRouter><RouteState kind={kind} reload={() => {}} /></MemoryRouter>)
  expect(screen.getByRole('heading', { level: 1 })).toBeTruthy(); expect(screen.getByRole('link', { name: 'Back to home' })).toBeTruthy()
  if (kind === 'not-found') expect(screen.queryByRole('button')).toBeNull()
})
it('actual external-store component clears stale copy only at fresh authority and unsubscribes', () => {
  const h = harness()
  try {
    const result = render(<MemoryRouter><ConnectionStatus controller={h.controller} reload={() => {}} /></MemoryRouter>)
    expect(screen.queryByRole('status')).toBeNull()
    const old = h.view.read()!.baseline!
    act(() => h.patch({ phase: 'resync' }))
    expect(screen.getByRole('status').textContent).toContain('Previously shown world')
    act(() => h.patch({ phase: 'live' }))
    expect(screen.getByRole('status').textContent).toContain('Previously shown world')
    act(() => h.patch({ baseline: { ...old, streamId: 'actual-replacement' } }))
    expect(screen.queryByRole('status')).toBeNull()
    result.unmount(); expect(h.view.listeners.size).toBe(1)
  } finally { h.dispose() }
})
