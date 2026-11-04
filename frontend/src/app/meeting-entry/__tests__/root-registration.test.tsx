// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { StrictMode } from 'react'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../../router'
import type { ActiveScope, ShellPorts } from '../../ports'
import { bootstrapFor, createFixturePorts } from '../../__tests__/fixtures/ports'
import { fixture, uid } from '../../../features/gift-discovery/__tests__/fixtures/data'

// Count actual bridge lifetimes while preserving its real fail-closed behavior.
// No mock injects a meeting release, source, policy grant, place revision, or clock.
const lifecycle = vi.hoisted(() => ({ creates: 0, disposals: 0 }))
vi.mock('../bridge', async original => {
  const actual = await original<typeof import('../bridge')>()
  return { ...actual, createMeetingEntry: (...args: Parameters<typeof actual.createMeetingEntry>) => {
    lifecycle.creates++
    const entry = actual.createMeetingEntry(...args); let disposed = false
    return { ...entry, dispose() { if (!disposed) { disposed = true; lifecycle.disposals++ }; entry.dispose() } }
  } }
})
const resources: (() => void)[] = []
beforeEach(() => { lifecycle.creates = 0; lifecycle.disposals = 0; vi.stubGlobal('location', { origin: 'https://mcr.example.invalid' }) })
afterEach(() => { cleanup(); resources.splice(0).forEach(stop => stop()); vi.unstubAllGlobals() })
const beaconId = '00000000-0000-4000-8000-000000000015'
const unavailable = 'Real-world meeting guidance is unavailable until current place, audience, and expiry information is verified.'
function setup(path: string, strict = false) {
  const auth = fixture(), world = createFixturePorts(); auth.controller.dispose()
  const bootstrap = vi.fn(async (scope: ActiveScope) => bootstrapFor(scope))
  const ports: ShellPorts = { ...world.ports, auth: auth.auth, session: auth.auth.session, bootstrap: { load: bootstrap } }
  const router = createMemoryRouter(createAppRoutes(ports), { initialEntries: [path] })
  resources.push(() => { router.dispose(); auth.dispose() })
  const node = <RouterProvider router={router} />
  return { ...auth, ...render(strict ? <StrictMode>{node}</StrictMode> : node), router, stats: world.stats, bootstrap }
}
const meetingRequests = (requests: readonly Request[]) => requests.filter(r => new URL(r.url).pathname.includes('/beacons'))

it.each([['W07', '/world/check-in'], ['M06', `/meet/beacons/${beaconId}`]])('registers %s with a real unavailable meeting panel rather than the generic placeholder', async (_task, path) => {
  const f = setup(path!)
  expect(await screen.findByText(unavailable)).toBeVisible()
  expect(screen.queryByText(/Still taking shape/)).not.toBeInTheDocument()
  expect(screen.queryByRole('button', { name: 'Share check-in' })).not.toBeInTheDocument()
  expect(screen.queryByText('SYNTHETIC red scarf')).not.toBeInTheDocument()
  expect(meetingRequests(f.requests)).toHaveLength(0)
  expect(f.stats).toMatchObject({ games: 1, connections: 1, peakGames: 1, peakConnections: 1 })
})
it('mounts the real meeting-entry lifetime once outside panel navigation, including StrictMode cleanup', async () => {
  const f = setup('/world', true)
  await screen.findByRole('navigation', { name: 'Main navigation' })
  await waitFor(() => expect(lifecycle.creates - lifecycle.disposals).toBe(1))
  const creates = lifecycle.creates, mounts = f.stats.mounts, host = document.querySelector('[data-world-host]')
  for (const path of ['/world/check-in', `/meet/beacons/${beaconId}`, '/world', '/world/check-in']) {
    await act(() => f.router.navigate(path))
    if (path !== '/world') expect(await screen.findByText(unavailable)).toBeVisible()
    expect(lifecycle.creates).toBe(creates)
    expect(document.querySelector('[data-world-host]')).toBe(host)
    expect(f.stats.mounts).toBe(mounts)
  }
  expect(meetingRequests(f.requests)).toHaveLength(0)
  f.unmount(); expect(lifecycle.disposals).toBe(lifecycle.creates)
  expect(f.stats).toMatchObject({ games: 0, connections: 0 })
})
it('preserves direct-beacon back navigation through Meet to World without creating a second world lease', async () => {
  const f = setup(`/meet/beacons/${beaconId}`)
  await screen.findByText(unavailable)
  const host = document.querySelector('[data-world-host]')
  fireEvent.click(screen.getByRole('button', { name: 'Back to previous panel' }))
  await waitFor(() => expect(f.router.state.location.pathname).toBe('/meet'))
  expect(await screen.findByRole('heading', { name: 'Meet', level: 1 })).toBeVisible()
  fireEvent.click(screen.getByRole('button', { name: 'Back to previous panel' }))
  await waitFor(() => expect(f.router.state.location.pathname).toBe('/world'))
  expect(document.querySelector('[data-world-host]')).toBe(host); expect(f.stats.mounts).toBe(1)
})
it('withdraws the shell owner on canonical policy changes and never promotes broad capabilities into a meeting grant', async () => {
  const f = setup('/world/check-in')
  await screen.findByText(unavailable)
  await waitFor(() => expect(lifecycle.creates - lifecycle.disposals).toBe(1))
  const before = lifecycle.disposals
  act(() => { f.auth.privacy.changePolicy(f.auth.privacy.capture(), 'block'); expect(f.stats.games).toBe(0) })
  expect(await screen.findByText(unavailable)).toBeVisible()
  expect(lifecycle.disposals).toBeGreaterThan(before)
  expect(meetingRequests(f.requests)).toHaveLength(0)
  expect(f.stats.peakGames).toBe(1)
})
it('releases the prior meeting lifetime on identity/event replacement while keeping current authority absent', async () => {
  const f = setup(`/meet/beacons/${beaconId}`)
  await screen.findByText(unavailable)
  await waitFor(() => expect(lifecycle.creates - lifecycle.disposals).toBe(1))
  const before = lifecycle.disposals
  act(() => { f.auth.session.replace(f.auth.session.read().generation, { userId: uid(90), eventId: uid(91), accessToken: 'replacement-memory-token' }) })
  expect(await screen.findByText(unavailable)).toBeVisible()
  expect(lifecycle.disposals).toBeGreaterThan(before)
  expect(f.stats).toMatchObject({ games: 1, connections: 1, peakGames: 1, peakConnections: 1 })
  expect(meetingRequests(f.requests)).toHaveLength(0)
})
it('keeps malformed beacon links in the existing safe invalid-link state without meeting requests', async () => {
  const f = setup('/meet/beacons/not-a-uuid')
  expect(await screen.findByText('This link is unavailable')).toBeVisible()
  expect(meetingRequests(f.requests)).toHaveLength(0)
})
