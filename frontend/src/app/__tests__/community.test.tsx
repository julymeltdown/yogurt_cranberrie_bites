// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { StrictMode } from 'react'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../router'
import type { ActiveScope, ShellBootstrap, ShellPorts } from '../ports'
import { bootstrapFor, createFixturePorts } from './fixtures/ports'
import { deferred, fixture, giveaway, uid } from '../../features/gift-discovery/__tests__/fixtures/data'
import { createCommunityData } from './fixtures/community-data'

const disposals: (() => void)[] = []
afterEach(() => { cleanup(); disposals.splice(0).forEach(dispose => dispose()) })
function setup(options: { path?: string; strict?: boolean; handler?: (request: Request) => Response | Promise<Response>;
  bootstrap?: (scope: ActiveScope, signal: AbortSignal) => Promise<ShellBootstrap> } = {}) {
  const f = fixture(options.handler ? { handler: options.handler } : {}), world = createFixturePorts()
  const load = vi.fn(options.bootstrap ?? (async (scope: ActiveScope) => bootstrapFor(scope)))
  const ports: ShellPorts = { ...world.ports, auth: f.auth, session: f.auth.session, bootstrap: { load } }
  const router = createMemoryRouter(createAppRoutes(ports), { initialEntries: [options.path ?? '/gifts'] })
  disposals.push(() => { router.dispose(); f.dispose() })
  const element = <RouterProvider router={router} />
  return { ...f, ...render(options.strict ? <StrictMode>{element}</StrictMode> : element), router, load, stats: world.stats }
}
const clickLink = (name: string) => fireEvent.click(screen.getByRole('link', { name }))
const listReady = () => screen.findByRole('link', { name: 'Fan badges' })

it('composes real feature routes with one controller and world owner across list/detail/own/world navigation', async () => {
  const f = setup()
  await listReady()
  const host = document.querySelector('[data-world-host]')
  clickLink('Fan badges')
  expect(await screen.findByRole('heading', { name: 'Fan badges' })).toBeVisible()
  expect(f.router.state.location.pathname).toBe(`/gifts/${giveaway().giveaway_id}`)
  clickLink('Check my gifts →')
  expect(await screen.findByText('Only you can see this draft.')).toBeVisible()
  fireEvent.click(screen.getByRole('button', { name: 'RECEIVING' }))
  expect(await screen.findByRole('link', { name: 'View giveaway' })).toBeVisible()
  clickLink('World'); clickLink('Gifts')
  await listReady()
  expect(f.requests.filter(request => new URL(request.url).pathname.endsWith('/giveaways')
    && !new URL(request.url).pathname.endsWith('/me/giveaways'))).toHaveLength(1)
  expect(document.querySelector('[data-world-host]')).toBe(host)
  expect(f.stats).toMatchObject({ mounts: 1, games: 1, connections: 1, peakGames: 1, peakConnections: 1 })
  expect(f.load).toHaveBeenCalledTimes(1)
  f.unmount()
  expect(f.stats).toMatchObject({ games: 0, connections: 0 })
})

it('does not fetch gift data merely by entering world and survives StrictMode deep-link setup', async () => {
  const f = setup({ path: '/world', strict: true })
  await screen.findByRole('navigation', { name: 'Main navigation' })
  expect(f.requests).toHaveLength(0)
  await act(() => f.router.navigate(`/gifts/${giveaway().giveaway_id}`))
  expect(await screen.findByRole('heading', { name: 'Fan badges' })).toBeVisible()
  expect(f.stats).toMatchObject({ games: 1, connections: 1, peakGames: 1, peakConnections: 1 })
  const mounts = f.stats.mounts
  act(() => { f.auth.session.commitRefresh(f.auth.session.read(), { userId: f.auth.session.read().userId!, accessToken: 'updated-test-bearer' }) })
  expect(f.stats.mounts).toBe(mounts)
  f.unmount()
  expect(f.stats.mounts).toBe(f.stats.disposals)
})

it('withdraws current UI and world synchronously on policy change, then uses freshly loaded capabilities', async () => {
  const refreshed = deferred<ShellBootstrap>()
  let reads = 0
  const f = setup({ bootstrap: async scope => ++reads === 1 ? bootstrapFor(scope) : refreshed.promise })
  await listReady()
  const before = f.requests.length
  act(() => {
    f.auth.privacy.changePolicy(f.auth.privacy.capture(), 'block')
    expect(f.stats.games).toBe(0)
  })
  expect(screen.queryByRole('link', { name: 'Fan badges' })).not.toBeInTheDocument()
  await waitFor(() => expect(f.load).toHaveBeenCalledTimes(2))
  expect(f.requests).toHaveLength(before)
  const bootstrap = bootstrapFor(f.load.mock.calls[1]![0])
  refreshed.resolve(Object.freeze({ ...bootstrap, capabilities: Object.freeze({ ...bootstrap.capabilities, gifts: { available: false as const, reason: 'FEATURE_DISABLED' as const } }) }))
  expect(await screen.findByRole('heading', { name: 'Gifts are unavailable' })).toBeVisible()
  expect(f.requests).toHaveLength(before)
  expect(f.stats.peakGames).toBe(1)
})

it('rejects bootstrap completed under an obsolete policy generation even when its port ignores abort', async () => {
  const old = deferred<ShellBootstrap>(), next = deferred<ShellBootstrap>()
  let reads = 0
  const f = setup({ bootstrap: () => ++reads === 1 ? old.promise : next.promise })
  await waitFor(() => expect(f.load).toHaveBeenCalledTimes(1))
  act(() => { f.auth.privacy.changePolicy(f.auth.privacy.capture(), 'consent-revoked') })
  await waitFor(() => expect(f.load).toHaveBeenCalledTimes(2))
  expect(f.load.mock.calls[0]![1].aborted).toBe(true)
  await act(async () => { old.resolve(bootstrapFor(f.load.mock.calls[0]![0])); await old.promise })
  expect(f.stats.mounts).toBe(0)
  expect(f.requests).toHaveLength(0)
  next.resolve(bootstrapFor(f.load.mock.calls[1]![0]))
  await listReady()
  expect(f.stats.games).toBe(1)
})

it('keeps suspended access closed on retry without dispatching or inventing revalidation', async () => {
  const f = setup()
  await listReady()
  act(() => { f.auth.privacy.changePolicy(f.auth.privacy.capture(), 'suspended') })
  expect(await screen.findByRole('heading', { name: 'Event availability could not be confirmed' })).toBeVisible()
  fireEvent.click(screen.getByRole('button', { name: 'Check again' }))
  await screen.findByRole('heading', { name: 'Event availability could not be confirmed' })
  expect(f.load).toHaveBeenCalledTimes(1)
  expect(f.requests).toHaveLength(1)
  expect(f.auth.privacy.capture().gate).toBe('suspended')
  expect(f.stats.games).toBe(0)
})

it('aborts pending feature data on account replacement and never renders its late payload', async () => {
  const pending = deferred<Response>()
  let requests = 0
  const f = setup({ handler: async request => {
    if (++requests === 1) return pending.promise
    const event = new URL(request.url).pathname.split('/')[4]!
    return Response.json({ items: [giveaway({ event_id: event, title: 'Replacement account gifts' })], next_cursor: null })
  } })
  await waitFor(() => expect(f.requests).toHaveLength(1))
  act(() => { f.auth.session.replace(f.auth.session.read().generation, { userId: uid(99), eventId: uid(98), accessToken: 'replacement-token' }) })
  expect(f.requests[0]!.signal.aborted).toBe(true)
  expect(await screen.findByRole('link', { name: 'Replacement account gifts' })).toBeVisible()
  await act(async () => { pending.resolve(Response.json({ items: [giveaway()], next_cursor: null })); await pending.promise })
  expect(screen.queryByRole('link', { name: 'Fan badges' })).not.toBeInTheDocument()
  expect(f.stats.peakGames).toBe(1)
  expect(f.load).toHaveBeenCalledTimes(2)
})

it('retains pending party commands across tabs and permits verified READ_ONLY safety cancellation', async () => {
  const data = createCommunityData()
  let readOnly = false
  const f = setup({ path: '/meet', handler: data.handle, bootstrap: async scope => {
    const value = bootstrapFor(scope)
    return readOnly ? Object.freeze({ ...value, event: Object.freeze({ ...value.event, phase: 'READ_ONLY' as const }), capabilities: Object.freeze({
      ...value.capabilities, mutations: { available: false, reason: 'EVENT_READ_ONLY' } as const,
    }) }) : value
  } })
  clickLink((await screen.findByRole('link', { name: 'Before the encore' })).textContent!)
  const apply = await screen.findByRole('button', { name: 'Apply to party' })
  await waitFor(() => expect(apply).toBeEnabled())
  fireEvent.click(apply); fireEvent.click(apply)
  expect(await screen.findByText('The server is still processing this command.')).toBeVisible()
  clickLink('Gifts'); await listReady(); clickLink('Meet')
  expect(await screen.findByText('The server is still processing this command.')).toBeVisible()
  fireEvent.click(screen.getByRole('button', { name: 'Check original command status' }))
  expect(await screen.findByText('The command was confirmed.')).toBeVisible()
  const commands = f.requests.filter(request => request.method === 'POST')
  expect(commands).toHaveLength(1)
  const status = f.requests.find(request => new URL(request.url).pathname.endsWith('/commands'))!
  expect(new URL(status.url).searchParams.get('idempotency_key')).toBe(commands[0]!.headers.get('Idempotency-Key'))
  expect(f.stats.mounts).toBe(1)
  act(() => { readOnly = true; f.auth.privacy.changePolicy(f.auth.privacy.capture(), 'consent-revoked') })
  await screen.findByRole('link', { name: 'Before the encore' })
  fireEvent.click(screen.getByRole('button', { name: 'MY PLAN' }))
  const cancel = await screen.findByRole('button', { name: 'Cancel application' })
  await waitFor(() => expect(cancel).toBeEnabled())
  fireEvent.click(cancel)
  expect(await screen.findByText('The command was confirmed.')).toBeVisible()
  const deletion = f.requests.find(request => request.method === 'DELETE')!
  expect(new URL(deletion.url).searchParams.get('expected_revision')).toBe('1')
  expect(await deletion.clone().text()).toBe('')
  clickLink('View party application')
  expect(await screen.findByRole('button', { name: 'Apply to party' })).toBeDisabled()
  expect(f.stats.peakGames).toBe(1)
})
