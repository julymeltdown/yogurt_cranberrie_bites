// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { StrictMode, useLayoutEffect } from 'react'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../router'
import { ShellLink } from '../navigation'
import { createProductionPorts, ShellLoadError } from '../ports'
import { PortsProvider, useScopedLoad } from '../access'
import type { ActiveScope } from '../ports'
import { bootstrapFor, createFixturePorts, giftPath, ids } from './fixtures/ports'

afterEach(cleanup)
const routers: ReturnType<typeof createMemoryRouter>[] = []
afterEach(() => { routers.splice(0).forEach(router => router.dispose()) })
function setup(path = '/world', fixture = createFixturePorts(), options: { strict?: boolean } = {}) {
  const router = createMemoryRouter(createAppRoutes(fixture.ports, { panels: {
    '/gifts': <><ShellLink id="fixture-gift" to={giftPath}>Open test giveaway</ShellLink><p>Fixture list</p></>,
    '/gifts/:giveawayId': <><ShellLink id="fixture-manage" to={`${giftPath}/manage`}>Manage test giveaway</ShellLink><p>Fixture detail</p></>,
    '/gifts/:giveawayId/manage': <input aria-label="Fixture draft" />,
  } }), { initialEntries: [path] })
  routers.push(router)
  const ui = <RouterProvider router={router} />
  const result = render(options.strict ? <StrictMode>{ui}</StrictMode> : ui)
  return { ...result, ...fixture, router }
}
const open = async () => { await screen.findByRole('navigation', { name: 'Sections' }) }
/** WP-09: the concert screens are reached from the secondary menu disclosure. */
const openConcertMenu = () => {
  const summary = document.querySelector('.mcr-secondary-menu > summary')
  if (summary instanceof HTMLElement && !(summary.parentElement as HTMLDetailsElement).open) fireEvent.click(summary)
}
const click = (name: string) => fireEvent.click(screen.getByRole('link', { name }))
const back = () => fireEvent.click(screen.getByRole('button', { name: 'Back to previous panel' }))

describe('persistent world ownership', () => {
  it('keeps one identical host and lease through the three sections, the concert menu, details and history', async () => {
    const { stats, router } = setup()
    await open()
    const host = document.querySelector('[data-world-host]')
    // WP-09: the tab bar is three sections; the concert screens are behind the
    // secondary menu. The world host and its lease must survive both.
    for (const section of ['Discover', 'Activity', 'My Profile']) {
      click(section)
      expect(document.querySelector('[data-world-host]')).toBe(host)
      expect(stats.mounts).toBe(1)
    }
    for (const concert of ['Feed', 'Meet', 'Gifts', 'Wall', 'World']) {
      openConcertMenu()
      click(concert)
      expect(document.querySelector('[data-world-host]')).toBe(host)
      expect(stats.mounts).toBe(1)
    }
    openConcertMenu(); click('Gifts'); click('Open test giveaway'); click('Manage test giveaway')
    expect(router.state.location.pathname).toBe(`${giftPath}/manage`)
    back(); expect(router.state.location.pathname).toBe(giftPath)
    back(); expect(router.state.location.pathname).toBe('/gifts')
    back(); expect(router.state.location.pathname).toBe('/world')
    expect(stats).toMatchObject({ games: 1, connections: 1, peakGames: 1, peakConnections: 1, mounts: 1 })
  })
  it('survives StrictMode setup-cleanup and fully releases on unmount', async () => {
    const { stats, unmount } = setup(giftPath, createFixturePorts(), { strict: true })
    await open()
    expect(screen.getByRole('heading', { level: 1, name: 'Giveaway' })).toHaveFocus()
    expect(stats.peakGames).toBe(1); expect(stats.peakConnections).toBe(1)
    expect(stats.games).toBe(1); expect(stats.connections).toBe(1)
    unmount()
    expect(stats.games).toBe(0); expect(stats.connections).toBe(0)
    expect(stats.mounts).toBe(stats.disposals)
    expect(stats.signals.every(signal => signal.aborted)).toBe(true)
  })
  it('closes a direct gift deep link through list to world without foreign history', async () => {
    const { stats, router } = setup(`${giftPath}/manage`)
    await open()
    back(); expect(router.state.location.pathname).toBe(giftPath)
    back(); expect(router.state.location.pathname).toBe('/gifts')
    back(); expect(router.state.location.pathname).toBe('/world')
    expect(stats.mounts).toBe(1)
  })
  it.each(['/legal/privacy', '/admin/reports'])('releases resources when exiting to %s', async destination => {
    const { stats, router } = setup()
    await open()
    await act(() => router.navigate(destination))
    await waitFor(() => expect(stats.games).toBe(0))
    expect(stats.connections).toBe(0)
    expect(document.querySelector('[data-world-host]')).toBeNull()
    if (destination.startsWith('/admin')) expect(await screen.findByRole('heading', { name: 'Access denied' })).toBeVisible()
  })
  it('replaces signed-in home with world and keeps a single current owner', async () => {
    const { stats, router } = setup()
    await open()
    await act(() => router.navigate('/'))
    await waitFor(() => expect(router.state.location.pathname).toBe('/world'))
    await open()
    expect(router.state.historyAction).toBe('REPLACE')
    expect(stats.games).toBe(1); expect(stats.connections).toBe(1)
    expect(stats.peakGames).toBe(1); expect(stats.peakConnections).toBe(1)
    expect(stats.mounts - stats.disposals).toBe(1)
  })
  it('releases old identity synchronously, then allocates one owner for account/event replacement', async () => {
    const { ports, stats, replace } = setup()
    await open()
    act(() => {
      replace()
      expect(stats.games).toBe(0)
      expect(stats.connections).toBe(0)
    })
    await open()
    expect(stats.scopes.at(-1)).toBe(`${ids.other}/${ids.event}`)
    act(() => replace(ids.other, ids.nextEvent))
    await open()
    expect(stats.scopes.at(-1)).toBe(`${ids.other}/${ids.nextEvent}`)
    expect(stats.peakGames).toBe(1); expect(stats.peakConnections).toBe(1)
    act(() => { ports.session.replace(ports.session.read().generation, null) })
    expect(stats.games).toBe(0); expect(stats.connections).toBe(0)
    expect(screen.getByRole('heading', { name: 'Sign in to continue' })).toBeVisible()
  })
  it('does not restart the host on a same-identity memory bearer refresh', async () => {
    const { ports, stats } = setup()
    await open()
    act(() => { ports.session.commitRefresh(ports.session.read(), { userId: ids.user, accessToken: 'refreshed-fixture-token' }) })
    expect(stats.mounts).toBe(1)
  })
  it('keeps panel navigation available after a world mount failure and retries explicitly', async () => {
    const fixture = createFixturePorts()
    const mount = vi.fn().mockImplementationOnce(() => { throw new Error('private adapter diagnostic') })
      .mockImplementation(fixture.ports.world!.mount)
    setup('/gifts', { ...fixture, ports: { ...fixture.ports, world: { mount } } })
    await open()
    expect(screen.getByRole('alert')).toHaveTextContent('The world could not open')
    expect(screen.queryByText('private adapter diagnostic')).not.toBeInTheDocument()
    expect(fixture.stats.games).toBe(0)
    fireEvent.click(screen.getByRole('button', { name: 'Try again' }))
    expect(fixture.stats.games).toBe(1)
    expect(fixture.stats.connections).toBe(1)
    expect(screen.getByRole('heading', { name: 'Gifts', level: 1 })).toBeVisible()
  })
  it('disposes a lease returned after synchronous session invalidation inside mount', async () => {
    const fixture = createFixturePorts()
    const mount = fixture.ports.world!.mount
    const ports = { ...fixture.ports, world: { mount: (...args: Parameters<typeof mount>) => {
      const lease = mount(...args)
      fixture.ports.session.replace(fixture.ports.session.read().generation, null)
      return lease
    } } }
    setup('/world', { ...fixture, ports })
    await screen.findByRole('heading', { name: 'Sign in to continue' })
    expect(fixture.stats.games).toBe(0); expect(fixture.stats.connections).toBe(0)
  })
})

describe('honest access, bootstrap and failure boundaries', () => {
  it.each(['bootstrap', 'admin'] as const)('never dispatches stale %s after identity changes between render and passive setup', async kind => {
    const fixture = createFixturePorts()
    const snapshot = fixture.ports.session.read()
    const scope: ActiveScope = { generation: snapshot.generation, userId: ids.user, eventId: ids.event }
    const bootstrap = vi.fn(fixture.ports.bootstrap.load)
    const authorize = vi.fn(fixture.ports.admin.authorize)
    const ports = { ...fixture.ports, bootstrap: { load: bootstrap }, admin: { authorize } }
    // Isolate the captured-scope hook so no parent subscription can accidentally mask
    // the race by rerendering/cleaning it up before the queued invocation executes.
    function Probe() {
      const result = useScopedLoad<unknown>(scope, signal => kind === 'bootstrap'
        ? ports.bootstrap.load(scope, signal) : ports.admin.authorize(scope, signal), 0)
      return <output>{result.status}</output>
    }
    function ReplaceBeforePassiveEffects() {
      useLayoutEffect(() => { fixture.replace() }, [])
      return null
    }
    render(<PortsProvider ports={ports}><Probe /><ReplaceBeforePassiveEffects /></PortsProvider>)
    await act(async () => { await Promise.resolve() })
    expect(snapshot.signal.aborted).toBe(true)
    expect(ports.session.isCurrent(scope)).toBe(false)
    expect(bootstrap).not.toHaveBeenCalled()
    expect(authorize).not.toHaveBeenCalled()
  })
  it('keeps production credentials empty and deep links unavailable without a bypass', async () => {
    const ports = createProductionPorts()
    const router = createMemoryRouter(createAppRoutes(ports), { initialEntries: [giftPath] }); routers.push(router)
    render(<RouterProvider router={router} />)
    expect(ports.session.read().accessToken).toBeNull()
    expect(await screen.findByRole('heading', { name: 'World entry is not open yet' })).toBeVisible()
    expect(document.querySelector('[data-world-host]')).toBeNull()
    expect(router.state.location.pathname).toBe(giftPath)
  })
  it.each(['unknown', 'checking', 'anonymous', 'unavailable', 'error'] as const)('never starts the world during %s auth', async state => {
    const fixture = createFixturePorts(state)
    const { stats, router } = setup(giftPath, fixture)
    expect(stats.mounts).toBe(0)
    expect(router.state.location.pathname).toBe(giftPath)
    act(() => fixture.setReadiness('ready'))
    await open()
    expect(screen.getByRole('heading', { name: 'Giveaway' })).toBeVisible()
    expect(stats.mounts).toBe(1)
  })
  it('preserves the direct target through checking, anonymous login and legitimate readiness', async () => {
    const fixture = createFixturePorts('checking')
    const { router } = setup(giftPath, fixture)
    act(() => fixture.setReadiness('anonymous'))
    click('Sign in')
    expect(router.state.location.pathname).toBe('/login')
    expect(new URLSearchParams(router.state.location.search).get('returnTo')).toBe(giftPath)
    await screen.findByRole('heading', { name: 'Sign in' })
    click('Back to your destination')
    expect(router.state.location.pathname).toBe(giftPath)
    act(() => fixture.setReadiness('ready'))
    await open()
    expect(screen.getByRole('heading', { name: 'Giveaway' })).toBeVisible()
  })
  it('discards a late bootstrap from the former identity even when it ignores abort', async () => {
    const fixture = createFixturePorts()
    const requests: { scope: Parameters<typeof bootstrapFor>[0]; resolve: (value: ReturnType<typeof bootstrapFor>) => void; signal: AbortSignal }[] = []
    const ports = { ...fixture.ports, bootstrap: { load: (scope: Parameters<typeof bootstrapFor>[0], signal: AbortSignal) => new Promise<ReturnType<typeof bootstrapFor>>(resolve => requests.push({ scope, signal, resolve })) } }
    setup(giftPath, { ...fixture, ports })
    await waitFor(() => expect(requests).toHaveLength(1))
    act(() => fixture.replace())
    await waitFor(() => expect(requests).toHaveLength(2))
    await act(() => { requests[0]!.resolve(bootstrapFor(requests[0]!.scope)) })
    expect(requests[0]!.signal.aborted).toBe(true)
    expect(fixture.stats.mounts).toBe(0)
    await act(() => { requests[1]!.resolve(bootstrapFor(requests[1]!.scope)) })
    await open()
    expect(fixture.stats.scopes).toEqual([`${ids.other}/${ids.event}`])
  })
  it('fails closed for wrong bootstrap identity, then retries without losing the deep link', async () => {
    const fixture = createFixturePorts()
    const load = vi.fn().mockResolvedValueOnce(bootstrapFor({ generation: 1, userId: ids.other, eventId: ids.event }))
      .mockImplementation(async scope => bootstrapFor(scope))
    const { stats, router } = setup(giftPath, { ...fixture, ports: { ...fixture.ports, bootstrap: { load } } })
    expect(await screen.findByRole('heading', { name: 'Your event is unavailable' })).toBeVisible()
    expect(stats.mounts).toBe(0)
    fireEvent.click(screen.getByRole('button', { name: 'Try again' }))
    await open()
    expect(router.state.location.pathname).toBe(giftPath)
  })
  it('never treats a ready participant as an admin role', async () => {
    const fixture = createFixturePorts()
    const authorize = vi.fn(async () => false)
    setup('/admin', { ...fixture, ports: { ...fixture.ports, admin: { authorize } } })
    expect(await screen.findByRole('heading', { name: 'Access denied' })).toBeVisible()
    expect(authorize).toHaveBeenCalledOnce()
    expect(fixture.stats.mounts).toBe(0)
  })
  it('keeps anonymous public routes lazy and entirely outside bootstrap/world allocation', async () => {
    const fixture = createFixturePorts('anonymous')
    const load = vi.fn(fixture.ports.bootstrap.load)
    setup('/', { ...fixture, ports: { ...fixture.ports, bootstrap: { load } } })
    await screen.findByRole('heading', { name: /One world.*Every era/ })
    expect(load).not.toHaveBeenCalled()
    expect(fixture.stats.mounts).toBe(0)
  })
  it('shows unavailable state for invalid IDs without exposing the target in product text', async () => {
    setup('/gifts/private-handle')
    await open()
    expect(screen.getByRole('alert', { name: 'This link is unavailable' })).toBeVisible()
    expect(screen.queryByText('private-handle')).not.toBeInTheDocument()
  })
  it('retains the shell across a nested render error and does not display thrown secrets', async () => {
    const fixture = createFixturePorts()
    function Broken(): never { throw new Error('secret fixture payload') }
    const router = createMemoryRouter(createAppRoutes(fixture.ports, { panels: { '/feed': <Broken /> } }), { initialEntries: ['/world'] }); routers.push(router)
    render(<RouterProvider router={router} />)
    await open()
    const log = vi.spyOn(console, 'error').mockImplementation(() => undefined)
    try {
      click('Feed')
      expect(await screen.findByRole('heading', { name: 'This page is unavailable', level: 1 })).toBeVisible()
      expect(screen.queryByText('secret fixture payload')).not.toBeInTheDocument()
      expect(fixture.stats.mounts).toBe(1)
    } finally { log.mockRestore() }
  })
  it.each(['unauthenticated', 'denied', 'not-found', 'gone', 'rate-limited', 'unavailable'] as const)(
    'keeps a safe shell entry failure for %s', async reason => {
      const fixture = createFixturePorts()
      const { stats, router } = setup(giftPath, { ...fixture, ports: { ...fixture.ports,
        bootstrap: { load: async () => { throw new ShellLoadError(reason) } } } })
      await screen.findByRole('alert')
      expect(stats.mounts).toBe(0)
      expect(router.state.location.pathname).toBe(giftPath)
      expect(screen.getByRole('link', { name: 'Back to home' })).toBeVisible()
    })
  it('bounds a hung bootstrap and aborts it without mounting the world', async () => {
    const fixture = createFixturePorts()
    let signal: AbortSignal | undefined
    vi.useFakeTimers()
    try {
      setup(giftPath, { ...fixture, ports: { ...fixture.ports, bootstrap: { load: async (_, received) => {
        signal = received; return new Promise(() => undefined)
      } } } })
      await act(async () => { await Promise.resolve() })
      expect(signal).toBeDefined()
      await act(() => vi.advanceTimersByTime(12_000))
      expect(screen.getByRole('heading', { name: 'Event availability could not be confirmed' })).toBeVisible()
      expect(signal!.aborted).toBe(true)
      expect(fixture.stats.mounts).toBe(0)
    } finally { vi.useRealTimers() }
  })
  it('keeps navigation usable while the world capability is unavailable', async () => {
    const fixture = createFixturePorts()
    setup('/gifts', { ...fixture, ports: { ...fixture.ports,
      bootstrap: { load: async scope => ({ ...bootstrapFor(scope), worldAvailable: false,
        capabilities: { ...bootstrapFor(scope).capabilities, world: { available: false, reason: 'FEATURE_DISABLED' } } }) } } })
    await open()
    expect(screen.getByRole('heading', { name: 'Gifts', level: 1 })).toBeVisible()
    expect(fixture.stats.mounts).toBe(0)
  })
})

describe('panel input, focus and scrolling', () => {
  it('bounds scroll memory across more than 100 detail routes and clears it for a new identity', async () => {
    const { stats, router, replace } = setup(giftPath)
    await open()
    document.querySelector('.shell-panel-body')!.scrollTop = 600
    let last = giftPath
    for (let index = 0; index < 101; index++) {
      last = `/gifts/00000000-0000-4000-8000-${(1000 + index).toString(16).padStart(12, '0')}`
      await act(() => router.navigate(last))
      document.querySelector('.shell-panel-body')!.scrollTop = index + 1
    }
    click('World')
    await act(() => router.navigate(giftPath))
    expect(document.querySelector('.shell-panel-body')!.scrollTop).toBe(0)
    await act(() => router.navigate(last))
    expect(document.querySelector('.shell-panel-body')!.scrollTop).toBe(101)
    expect(stats.mounts).toBe(1)
    act(() => replace())
    await open()
    expect(document.querySelector('.shell-panel-body')!.scrollTop).toBe(0)
    expect(stats.peakGames).toBe(1)
    expect(stats.peakConnections).toBe(1)
  })
  it('blocks input and pauses presentation while hidden, without assuming persistent networking', async () => {
    const { stats } = setup()
    await open()
    try {
      Object.defineProperty(document, 'hidden', { configurable: true, value: true })
      fireEvent(document, new Event('visibilitychange'))
      expect(stats.inputBlocked).toBe(true)
      expect(stats.visible).toBe(false)
      Object.defineProperty(document, 'hidden', { configurable: true, value: false })
      fireEvent(document, new Event('visibilitychange'))
      expect(stats.inputBlocked).toBe(false)
      expect(stats.visible).toBe(true)
      expect(stats.mounts).toBe(1)
    } finally { Reflect.deleteProperty(document, 'hidden') }
  })
  it('focuses new headings and restores the known opener on browser back', async () => {
    const { router } = setup('/gifts')
    await open()
    const opener = screen.getByRole('link', { name: 'Open test giveaway' })
    opener.focus(); fireEvent.click(opener)
    expect(screen.getByRole('heading', { name: 'Giveaway', level: 1 })).toHaveFocus()
    await act(() => router.navigate(-1))
    expect(document.getElementById(opener.id)).toHaveFocus()
  })
  it('pauses input for panels and leaves Escape to editable fields', async () => {
    const { stats, router } = setup()
    await open()
    expect(stats.inputBlocked).toBe(false)
    click('Gifts'); expect(stats.inputBlocked).toBe(true)
    expect(document.querySelector('[data-world-host]')).toHaveAttribute('inert')
    click('Open test giveaway'); click('Manage test giveaway')
    const draft = screen.getByRole('textbox', { name: 'Fixture draft' })
    draft.focus(); fireEvent.keyDown(draft, { key: 'Escape' })
    expect(router.state.location.pathname).toBe(`${giftPath}/manage`)
    fireEvent.keyDown(screen.getByRole('heading', { name: 'Manage giveaway', level: 1 }), { key: 'Escape' })
    expect(router.state.location.pathname).toBe(giftPath)
    click('World'); expect(stats.inputBlocked).toBe(false)
    expect(document.querySelector('[data-world-host]')).not.toHaveAttribute('inert')
  })
  it('supports peek/half/full without remount and restores scroll after world navigation', async () => {
    const { stats } = setup('/gifts')
    await open()
    const body = document.querySelector('.shell-panel-body')!
    body.scrollTop = 220
    fireEvent.click(screen.getByRole('button', { name: 'Expand panel' }))
    expect(stats.visible).toBe(false)
    fireEvent.click(screen.getByRole('button', { name: 'Reduce panel' }))
    expect(stats.visible).toBe(true)
    fireEvent.click(screen.getByRole('button', { name: 'Minimize panel' }))
    expect(body).not.toBeVisible()
    fireEvent.click(screen.getByRole('button', { name: 'Show panel content' }))
    expect(body).toBeVisible()
    click('World'); click('Gifts')
    expect(document.querySelector('.shell-panel-body')!.scrollTop).toBe(220)
    expect(stats.mounts).toBe(1)
  })
  it('reveals a minimized panel when a new query target opens on the same path', async () => {
    const first = `/report?targetType=USER&targetId=${ids.user}`
    const second = `/report?targetType=USER&targetId=${ids.other}`
    const { stats, router } = setup(first)
    await open()
    fireEvent.click(screen.getByRole('button', { name: 'Minimize panel' }))
    expect(document.querySelector('.shell-panel-body')).not.toBeVisible()
    await act(() => router.navigate(second))
    expect(document.querySelector('.shell-panel-body')).toBeVisible()
    expect(screen.getByRole('heading', { name: 'Report', level: 1 })).toHaveFocus()
    expect(stats.mounts).toBe(1)
  })
})
