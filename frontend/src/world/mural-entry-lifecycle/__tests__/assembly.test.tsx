// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { act, cleanup, render, screen, waitFor } from '@testing-library/react'
import { PortsProvider } from '../../../app/access'
import { createProductionPorts } from '../../../app/ports'
import { WorldPanels } from '../../../app/world-panels'
import { MuralWallPanel, useMuralEntry } from '../../../app/mural-entry'
import { createWorldComposition } from '../../composition'
import { currentScope, project, resolverFor } from '../../composition/__tests__/fixtures/data'
import { inputConfiguration } from '../../../app/world-entry/__tests__/data'
import type { WorldCompositionOptions } from '../../composition'
import type { WorldCamera } from '../../camera/port'
import { memoryScene } from '../../../features/mural/sync/__tests__/fixture'
import type { MuralSyncController } from '../../../features/mural/sync'
import { assemblyData } from './assembly-data'

vi.mock('../../map/resources', async original => {
  const module = await original<typeof import('../../map/resources')>()
  return { ...module, loadMap: (input: Parameters<typeof module.loadMap>[0], signal: AbortSignal) => module.loadMap({ ...input, decode: async () => {
    const image = new Image(); Object.defineProperties(image, { naturalWidth: { value: 96 }, naturalHeight: { value: 32 } })
    return { image, dispose() {} }
  } }, signal) }
})
const stops: (() => void)[] = []
beforeEach(() => {
  vi.stubGlobal('location', { origin: 'https://mcr.example.invalid' })
  vi.stubGlobal('ResizeObserver', class { observe() {} unobserve() {} disconnect() {} })
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(null)
})
afterEach(() => { cleanup(); stops.splice(0).reverse().forEach(stop => stop()); vi.restoreAllMocks(); vi.unstubAllGlobals(); document.body.replaceChildren() })

async function fixture(mode: 'qualified' | 'absent' | 'throw' | 'readonly' = 'qualified') {
  const f = await assemblyData(); stops.push(f.dispose); vi.stubGlobal('WebSocket', f.Socket); await f.login()
  if (mode === 'readonly') f.setWritable(false)
  const scene = memoryScene(), closes = new Set<() => void>(), order: string[] = []
  let engines = 0, sync: MuralSyncController | null = null
  const composition = createWorldComposition({ auth: f.auth, resolveRelease: resolverFor(f.auth, f.data),
    ...(mode === 'absent' ? {} : { mural: mode === 'throw' ? () => { throw Error('fixture supplier fault') } : f.factory }),
    importEngine: async () => host => {
      engines++; const canvas = document.createElement('canvas'); host.append(canvas)
      const camera: WorldCamera = { surface: () => canvas, center() {}, project: p => p, unproject: p => p,
        read: () => ({ zoom: 1, scrollX: 0, scrollY: 0, width: 390, height: 400 }) }
      return { ready: Promise.resolve(), closed: Promise.resolve(), resize() {}, setVisible() {}, camera,
        muralScene() { return { ...scene.port, onClose(fn) { closes.add(fn); return () => { closes.delete(fn) } } } },
        destroy() { order.push(`engine:${scene.read().textures}`); engines--; canvas.remove(); for (const fn of Array.from(closes)) fn() } }
    } })
  stops.push(composition.dispose)
  const ports = createProductionPorts(f.auth, { localInput: inputConfiguration, load: async () => composition })
  stops.push(() => ports.dispose?.())
  const host = document.createElement('div'); document.body.append(host)
  const scope = f.auth.privacy.capture(), lifetime = new AbortController()
  const lease = ports.world!.mount(host, { scope: currentScope(f.auth), privacyScope: scope, bootstrap: await project(f.auth, f.data), signal: lifetime.signal })
  stops.push(lease.dispose)
  await waitFor(() => expect(composition.replication.read()?.phase).toBe('live'))
  function Probe() { sync = useMuralEntry().sync; return null }
  const view = render(<PortsProvider ports={ports}><WorldPanels source={ports.worldAccess!.read()!.presentation!} scope={scope}><Probe /><MuralWallPanel /></WorldPanels></PortsProvider>)
  return { f, composition, ports, lease, lifetime, scene, view, order, sync: () => sync, engines: () => engines }
}

it('actual custom-load production adapter forwards composition mural publication into WorldPanels and stable entry ownership', async () => {
  const h = await fixture(); await screen.findByText('REV 10 · EDITABLE'); h.scene.frame()
  const sync = h.sync()!, release = h.composition.presentation.read()!.mural!.read()!
  await waitFor(() => expect(sync.editor.canUseTool('pencil')).toBe(true))
  expect(h.engines()).toBe(1); expect(h.f.sockets).toHaveLength(1); expect(h.f.stats().factoryCalls).toBe(1)
  expect(Object.keys(h.f.context()!.world).sort()).toEqual(['read', 'subscribe']); expect(Object.isFrozen(h.f.context()!.world)).toBe(true)
  expect(h.scene.read()).toMatchObject({ objects: 1, textures: 1 }); expect(release.scope).toEqual(h.f.auth.privacy.capture())
  h.f.server.mode('lost')
  await act(async () => { sync.editor.select(2, 3); await sync.editor.confirm() })
  const command = sync.editor.read().command
  act(() => h.f.revoke())
  expect(screen.getByText(/Wall unavailable/)).toBeVisible(); expect(h.scene.read().textures).toBe(0); expect(sync.editor.read().command).toBe(command)
  expect(h.f.requests.filter(r => r.method === 'POST' && r.url.endsWith('/pixels'))).toHaveLength(1)
  expect(h.sync()).toBeNull(); expect(h.engines()).toBe(1); expect(h.f.sockets).toHaveLength(1)
  act(() => { h.f.qualify() }); await screen.findByText('REV 11 · EDITABLE')
  expect(h.sync()).toBe(sync); expect(sync.editor.read().command).toBe(command)
  h.f.server.mode('ack'); h.f.server.resultAvailable(true)
  await act(() => sync.editor.check())
  expect(sync.editor.read().phase).toBe('confirmed'); expect(sync.editor.read().command).toBeNull()
  expect(h.f.requests.filter(r => r.method === 'POST' && r.url.endsWith('/pixels'))).toHaveLength(1)
})

it.each(['absent', 'throw'] as const)('%s qualifier keeps the actual caller closed without breaking world presentation', async mode => {
  const h = await fixture(mode)
  expect(await screen.findByText(/Wall unavailable/)).toBeVisible(); expect(h.sync()).toBeNull()
  expect(h.composition.presentation.read()).not.toBeNull(); expect(h.composition.replication.read()?.phase).toBe('live')
  expect(h.f.requests.some(r => r.url.includes('/mural'))).toBe(false); expect(h.engines()).toBe(1)
})

it('preserves supplied readonly eligibility at the actual provider without enabling Confirm', async () => {
  const h = await fixture('readonly'); await screen.findByText('REV 10 · EDITABLE')
  expect(h.sync()!.editor.canEdit()).toBe(false); expect(screen.queryByRole('button', { name: 'Confirm pixel' })).not.toBeInTheDocument()
  expect(h.f.requests.some(r => r.method === 'POST' && r.url.endsWith('/pixels'))).toBe(false)
})

it.each(['abort', 'privacy'] as const)('%s withdraws caller pixels/source before engine destruction', async mode => {
  const h = await fixture(); await screen.findByText('REV 10 · EDITABLE'); h.scene.frame()
  const source = h.composition.presentation.read()!.mural!, release = source.read()!, canvas = document.querySelector('.mcr-mural__surface canvas') as HTMLCanvasElement
  act(() => { if (mode === 'abort') h.lifetime.abort(); else h.f.auth.privacy.changePolicy(h.f.auth.privacy.capture(), 'suspended') })
  expect(release.signal.aborted).toBe(true); expect(source.read()).toBeNull(); expect(canvas.width).toBe(0)
  expect(h.scene.read()).toMatchObject({ objects: 0, textures: 0, listeners: 0 })
  expect(h.order).toEqual(['engine:0']); expect(h.f.stats()).toMatchObject({ sourceStops: 1, sourceListeners: 0 })
  expect(h.engines()).toBe(0); expect(h.f.sockets[0]!.readyState).toBe(3)
})

// The options type remains a narrow additive source seam, not a caller or admission replacement.
const acceptsFactory = (factory: WorldCompositionOptions['mural']) => factory
it('exposes only the optional frozen qualification-source option', () => { expect(acceptsFactory(undefined)).toBeUndefined() })
