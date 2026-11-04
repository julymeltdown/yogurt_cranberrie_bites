// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { StrictMode, type ReactNode } from 'react'
import { act, cleanup, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../../router'
import { bootstrapFor, createFixturePorts } from '../../__tests__/fixtures/ports'
import { MuralEntryProvider, MuralWallPanel, useMuralEntry } from '../provider'
import type { MuralSyncController } from '../../../features/mural/sync'
import type { MuralEntryRelease } from '../ports'
import { entryFixture } from './fixtures'

const owned: ReturnType<typeof entryFixture>[] = [], stop: (() => void)[] = []
beforeEach(() => {
  vi.stubGlobal('location', { origin: 'https://mcr.example.invalid' })
  vi.stubGlobal('ResizeObserver', class { observe() {} unobserve() {} disconnect() {} })
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(null)
})
afterEach(() => { cleanup(); stop.splice(0).forEach(f => f()); owned.splice(0).forEach(f => f.dispose()); vi.restoreAllMocks(); vi.unstubAllGlobals() })
function setup() {
  const f = entryFixture(); owned.push(f); const r = f.release(), world = createFixturePorts()
  const router = createMemoryRouter(createAppRoutes({ ...world.ports, auth: f.auth, session: f.auth.session,
    bootstrap: { load: async scope => bootstrapFor(scope) } }, { panels: { '/wall': <MuralWallPanel />, '/feed': <p>Other panel fixture</p> } }), { initialEntries: ['/wall'] })
  stop.push(() => router.dispose())
  let sync: MuralSyncController | null = null
  function Probe() { sync = useMuralEntry().sync; return null }
  function Wrapper({ release = r.value, children }: { release?: MuralEntryRelease | null; children: ReactNode }) {
    return <StrictMode><MuralEntryProvider auth={f.auth} scope={{ ...f.scope }} release={release}><Probe />{children}</MuralEntryProvider></StrictMode>
  }
  const view = render(<Wrapper><RouterProvider router={router} /></Wrapper>)
  return { f, r, world, router, view, Wrapper, sync: () => sync }
}
async function confirmed(h: ReturnType<typeof setup>, revision: number) {
  await waitFor(() => expect(screen.getByRole('status', { name: 'Wall synchronization status' }))
    .toHaveTextContent(`Wall synchronized at revision ${revision}.`))
  expect(h.sync()!.read()).toMatchObject({ phase: 'ready', revision, problem: null })
  const snapshot = h.sync()!.editor.read().snapshot!
  expect(snapshot).toMatchObject({ revision, state: 'EDITABLE', width: 128, height: 128 })
  expect(snapshot.pixels).toEqual(h.f.server.read().pixels)
  expect(screen.getByText(`REV ${revision} · EDITABLE`)).toBeVisible()
  expect(document.querySelector('.mcr-mural__surface canvas')).toHaveAttribute('width', '512')
}
it('keeps the same owner and unknown command across actual Wall/world/other-panel navigation and reconnect', async () => {
  const h = setup(); await confirmed(h, 10); const sync = h.sync()!, mounts = h.world.stats.mounts
  h.f.server.mode('lost')
  await act(async () => { sync.editor.select(2, 3); await sync.editor.confirm() })
  const command = sync.editor.read().command
  expect(command).not.toBeNull(); expect(sync.editor.read().phase).toBe('unknown')
  expect(screen.getByRole('status', { name: 'Original placement status' })).toHaveTextContent('unresolved')
  await act(() => h.router.navigate('/world'))
  expect(screen.queryByRole('status', { name: 'Wall synchronization status' })).not.toBeInTheDocument()
  expect(document.querySelector('.mcr-mural__surface canvas')).toBeNull()
  await act(() => h.router.navigate('/feed')); expect(await screen.findByText('Other panel fixture')).toBeVisible()
  await act(() => h.router.navigate('/wall')); await screen.findByRole('status', { name: 'Original placement status' })
  expect(h.sync()).toBe(sync); expect(sync.editor.read().command).toBe(command)
  act(() => h.r.lifetime.abort())
  expect(screen.queryByRole('status', { name: 'Wall synchronization status' })).not.toBeInTheDocument()
  expect(sync.editor.read().snapshot).toBeNull()
  const next = h.f.release()
  h.view.rerender(<h.Wrapper release={next.value}><RouterProvider router={h.router} /></h.Wrapper>)
  await confirmed(h, 11)
  expect(h.sync()).toBe(sync); expect(sync.editor.read().command).toBe(command)
  expect(screen.getByRole('button', { name: 'Confirm pixel' })).toBeDisabled()
  expect(h.f.requests.filter(r => r.method === 'POST')).toHaveLength(1)
  const replacement = h.f.release(), before = document.querySelector('.mcr-mural__surface canvas') as HTMLCanvasElement
  h.view.rerender(<h.Wrapper release={replacement.value}><RouterProvider router={h.router} /></h.Wrapper>)
  await confirmed(h, 11)
  expect(h.sync()).toBe(sync); expect(sync.editor.read().command).toBe(command)
  expect(before.width).toBe(0)
  expect((document.querySelector('.mcr-mural__surface canvas') as HTMLCanvasElement).width).toBe(512)
  expect(h.world.stats.mounts).toBe(mounts); expect(h.world.stats.peakGames).toBe(1); expect(h.world.stats.peakConnections).toBe(1)
  h.view.unmount(); expect(h.r.authority.count()).toBe(0); expect(next.authority.count()).toBe(0); expect(replacement.authority.count()).toBe(0)
  expect(h.world.stats.games).toBe(0); expect(h.world.stats.connections).toBe(0)
})
it('removes protected DOM and raster on privacy closure and never installs a replacement owner for the expired scope', async () => {
  const h = setup(); await confirmed(h, 10); const sync = h.sync()!
  const canvas = document.querySelector('.mcr-mural__surface canvas') as HTMLCanvasElement
  act(() => h.r.scene.frame())
  const raster = h.r.scene.read().rgba!, server = h.f.server.read()
  expect(h.r.scene.read()).toMatchObject({ textures: 1, objects: 1, uploads: 1 })
  expect(raster).toEqual(new Uint8ClampedArray(server.pixels.flatMap(index => {
    const hex = server.palette[index]!
    return [Number.parseInt(hex.slice(1, 3), 16), Number.parseInt(hex.slice(3, 5), 16), Number.parseInt(hex.slice(5, 7), 16), 255]
  })))
  act(() => h.f.auth.privacy.changePolicy(h.f.scope, 'event-closed'))
  expect(screen.queryByRole('status', { name: 'Wall synchronization status' })).not.toBeInTheDocument()
  expect(document.querySelector('.mcr-mural__surface canvas')).toBeNull(); expect(sync.editor.read().snapshot).toBeNull()
  expect(canvas.width).toBe(0); expect(canvas.height).toBe(0)
  expect(h.r.scene.read()).toMatchObject({ textures: 0, objects: 0, listeners: 0, rgba: null })
  expect(h.r.authority.count()).toBe(0)
  const count = h.f.requests.length
  h.view.rerender(<h.Wrapper release={h.f.release().value}><RouterProvider router={h.router} /></h.Wrapper>)
  await waitFor(() => expect(h.f.requests).toHaveLength(count))
  expect(h.sync()).toBeNull()
})
it('renders a closed panel without a qualified release through the actual composition seam', async () => {
  const h = setup()
  h.view.rerender(<h.Wrapper release={null}><RouterProvider router={h.router} /></h.Wrapper>)
  expect(await screen.findByText(/Wall unavailable. Reconnect to a qualified/)).toBeVisible()
  expect(screen.queryByRole('button', { name: 'Confirm pixel' })).not.toBeInTheDocument()
})
