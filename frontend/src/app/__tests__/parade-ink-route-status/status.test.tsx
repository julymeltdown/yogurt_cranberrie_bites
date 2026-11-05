// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { MemoryRouter, createMemoryRouter, RouterProvider } from 'react-router-dom'
import { ConnectionStatusNotice } from '../../../features/connection-status'
import { RouteState } from '../../errors/RouteState'
import { RouteFailure } from '../../router'
import { connectionCopy, type ConnectionKind } from '../../../features/connection-status/model'

afterEach(cleanup)
it('keeps every non-ready notice actionable without changing its owner predicates', () => {
  for (const kind of ['offline', 'reconnecting', 'resync', 'suspended', 'unavailable', 'maintenance', 'version'] satisfies ConnectionKind[]) {
    const retry = vi.fn(), reload = vi.fn()
    const view = render(<MemoryRouter><ConnectionStatusNotice view={{ kind, stale: true, pending: true, canRetry: true }} retry={retry} reload={reload} /></MemoryRouter>)
    expect(screen.getByRole('heading', { name: connectionCopy[kind].title })).toBeDefined()
    expect(screen.getByText('Previously shown world information may be out of date.')).toBeDefined()
    expect(screen.queryByRole('button', { name: 'Reload page' })).toBeNull()
    fireEvent.click(screen.getByRole('button', { name: 'Reconnect' }))
    expect(retry).toHaveBeenCalledTimes(1)
    expect(reload).not.toHaveBeenCalled()
    expect(screen.getByRole('link', { name: 'Back to world' }).getAttribute('href')).toBe('/world')
    view.unmount()
  }
})
it('does not offer reload while suspended and removes the ready notice', () => {
  const view = render(<MemoryRouter><ConnectionStatusNotice view={{ kind: 'suspended', stale: false, pending: false, canRetry: false }} retry={vi.fn()} reload={vi.fn()} /></MemoryRouter>)
  expect(screen.queryByRole('button')).toBeNull()
  view.rerender(<MemoryRouter><ConnectionStatusNotice view={{ kind: 'ready', stale: false, pending: false, canRetry: false }} retry={vi.fn()} reload={vi.fn()} /></MemoryRouter>)
  expect(screen.queryByRole('region')).toBeNull()
})
it.each([401, 403, 404, 410, 429, 503])('keeps HTTP %s payloads out of the functioning route fallback', async status => {
  const router = createMemoryRouter([{ path: '/', loader() { throw new Response('PRIVATE_PAYLOAD <script>secret()</script>', { status }) }, element: <p>Unreachable</p>, errorElement: <RouteFailure /> }])
  render(<RouterProvider router={router} />)
  await screen.findByRole('heading', { name: 'This page is unavailable' })
  expect(document.body.textContent).not.toContain('PRIVATE_PAYLOAD')
  expect(screen.getByRole('link', { name: 'Back to world' }).getAttribute('href')).toBe('/world')
  router.dispose()
})
it('omits reload for a missing route while preserving safe home and World navigation', () => {
  const reload = vi.fn()
  render(<MemoryRouter><RouteState kind="not-found" reload={reload} /></MemoryRouter>)
  expect(screen.queryByRole('button', { name: 'Reload page' })).toBeNull()
  expect(screen.getByRole('link', { name: 'Back to home' }).getAttribute('href')).toBe('/')
  expect(reload).not.toHaveBeenCalled()
})
