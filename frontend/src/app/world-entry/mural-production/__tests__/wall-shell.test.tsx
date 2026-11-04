// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { act, cleanup, render, screen, waitFor } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createProductionPorts } from '../../../ports'
import { createAppRoutes } from '../../../router'
import { authHarness, inputConfiguration, replicationPort, requestFor } from '../../__tests__/data'
import type { WorldComposition } from '../../../../world/composition'

const stops: (() => void)[] = []
afterEach(() => { cleanup(); stops.splice(0).reverse().forEach(stop => stop()); vi.restoreAllMocks() })

it('keeps the full Wall attached to the same World while blocking input, and still suspends when the page hides', async () => {
  const h = authHarness(), request = await requestFor(h.auth), replication = replicationPort(request)
  const lease = { dispose: vi.fn(), setInputBlocked: vi.fn(), setVisible: vi.fn() }
  const composition: WorldComposition = { mount: vi.fn(() => lease), replication: { ...replication, reconnect: async () => false },
    subscribe: replication.subscribe, read: () => ({ phase: 'ready', runtime: null, renderer: 'unavailable', problem: null }),
    dispose: vi.fn(), settled: async () => {} }
  const ports = createProductionPorts(h.auth, { localInput: inputConfiguration, load: async () => composition })
  const router = createMemoryRouter(createAppRoutes({ ...ports, readiness: { read: () => 'ready', subscribe: () => () => {} },
    bootstrap: { load: async () => request.bootstrap } }), { initialEntries: ['/wall'] })
  stops.push(() => h.auth.dispose(), () => ports.dispose?.(), () => router.dispose())
  vi.spyOn(document, 'hidden', 'get').mockReturnValue(false)
  render(<RouterProvider router={router} />)
  await screen.findByRole('navigation', { name: 'Main navigation' })
  await waitFor(() => expect(composition.mount).toHaveBeenCalledOnce())
  const host = document.querySelector('[data-world-host]')
  // Force the full panel through the actual shell control, independent of jsdom width.
  await act(async () => screen.getByRole('button', { name: 'Expand panel' }).click())
  expect(document.querySelector('.shell-stage')).toHaveAttribute('data-panel-size', 'full')
  expect(lease.setInputBlocked).toHaveBeenLastCalledWith(true)
  expect(lease.setVisible).toHaveBeenLastCalledWith(true)
  vi.spyOn(document, 'hidden', 'get').mockReturnValue(true)
  act(() => document.dispatchEvent(new Event('visibilitychange')))
  expect(lease.setVisible).toHaveBeenLastCalledWith(false)
  vi.spyOn(document, 'hidden', 'get').mockReturnValue(false)
  act(() => document.dispatchEvent(new Event('visibilitychange')))
  expect(lease.setVisible).toHaveBeenLastCalledWith(true)
  await act(() => router.navigate('/feed'))
  expect(lease.setVisible).toHaveBeenLastCalledWith(false)
  expect(document.querySelector('[data-world-host]')).toBe(host)
  expect(composition.mount).toHaveBeenCalledOnce()
})
